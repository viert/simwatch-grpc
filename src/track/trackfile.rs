use chrono::{DateTime, Utc};
use log::debug;
use std::{
  error::Error,
  fmt::Display,
  fs::{File, OpenOptions},
  io::{Seek, SeekFrom, Write},
  mem::size_of,
  os::unix::prelude::FileExt,
  ptr::slice_from_raw_parts,
};

use crate::{moving::pilot::Pilot, service::camden};

const TRACK_VERSION: u16 = 1;

type Result<T> = std::result::Result<T, TrackFileError>;

#[derive(Debug)]
pub struct TrackFileError(String);
impl Display for TrackFileError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "TrackFileError: {}", self.0)
  }
}
impl Error for TrackFileError {}

impl From<std::io::Error> for TrackFileError {
  fn from(value: std::io::Error) -> Self {
    TrackFileError(format!("{value}"))
  }
}

#[derive(Debug, Clone)]
#[repr(C)]
struct TrackFileHeader {
  version: u16,
  ts: u64,
  count: u64,
}

impl Default for TrackFileHeader {
  fn default() -> Self {
    Self {
      version: TRACK_VERSION,
      ts: Utc::now().timestamp_millis() as u64,
      count: Default::default(),
    }
  }
}

impl TrackFileHeader {
  pub fn inc(&mut self) {
    self.ts = Utc::now().timestamp_millis() as u64;
    self.count += 1;
  }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct TrackPoint {
  pub lat: f64,
  pub lng: f64,
  pub alt: i32,
  pub hdg: i16,
  pub gs: i32,
  pub ts: i64,
}

impl PartialEq for TrackPoint {
  fn eq(&self, other: &Self) -> bool {
    self.lat == other.lat
      && self.lng == other.lng
      && self.alt == other.alt
      && self.hdg == other.hdg
      && self.gs == other.gs
  }
}

impl From<TrackPoint> for camden::TrackPoint {
  fn from(value: TrackPoint) -> Self {
    Self {
      lat: value.lat,
      lng: value.lng,
      alt: value.alt,
      hdg: value.hdg as i32,
      gs: value.gs,
      ts: value.ts,
    }
  }
}

impl From<&Pilot> for TrackPoint {
  fn from(value: &Pilot) -> Self {
    Self {
      lat: value.position.lat,
      lng: value.position.lng,
      alt: value.altitude,
      hdg: value.heading,
      gs: value.groundspeed,
      ts: value.last_updated.timestamp_millis(),
    }
  }
}

fn to_raw<T>(obj: &T) -> Vec<u8> {
  let slice = slice_from_raw_parts(obj, size_of::<T>()) as *const [u8];
  let slice = unsafe { &*slice };
  slice.into()
}

fn from_raw<T: Clone>(data: &[u8]) -> std::result::Result<T, TrackFileError> {
  if data.len() < size_of::<T>() {
    Err(TrackFileError("insufficient data length".into()))
  } else {
    let slice = data as *const [u8] as *const T;
    let tp = unsafe { &*slice };
    Ok(tp.clone())
  }
}

pub struct TrackFile {
  file: File,
  name: String,
}

impl TrackFile {
  pub fn new(filename: &str) -> Result<Self> {
    let res = OpenOptions::new().write(true).read(true).open(&filename);

    match res {
      Ok(file) => Ok(Self {
        file,
        name: filename.to_owned(),
      }),
      Err(err) => match err.kind() {
        std::io::ErrorKind::NotFound => {
          let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&filename)?;
          let header = TrackFileHeader::default();
          let raw_header = to_raw(&header);
          file.write_all(&raw_header)?;
          let tf = Self {
            file,
            name: filename.to_owned(),
          };
          Ok(tf)
        }
        _ => Err(err.into()),
      },
    }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn mtime(&self) -> Result<DateTime<Utc>> {
    let header = self.read_file_header()?;
    let secs = header.ts / 1000;
    let nsecs = (header.ts % 1000) * 1000;
    let dt = DateTime::from_timestamp(secs as i64, nsecs as u32).unwrap_or(Utc::now());
    Ok(dt)
  }

  fn read_file_header(&self) -> Result<TrackFileHeader> {
    let mut buf = [0; size_of::<TrackFileHeader>()];
    self.file.read_at(&mut buf, 0)?;
    Ok(from_raw(&buf)?)
  }

  fn write_file_header(&mut self, header: &TrackFileHeader) -> Result<()> {
    let buf = to_raw(header);
    debug!(
      "writing file header {}: {:?} buf={:?}",
      self.name, header, buf
    );
    self.file.write_at(&buf, 0)?;
    Ok(())
  }

  fn inc(&mut self) -> Result<()> {
    let mut header = self.read_file_header()?;
    header.inc();
    self.write_file_header(&header)?;
    Ok(())
  }

  pub fn count(&self) -> Result<u64> {
    let header = self.read_file_header()?;
    Ok(header.count)
  }

  pub fn destroy(self) -> Result<()> {
    std::fs::remove_file(&self.name)?;
    Ok(())
  }

  pub fn append(&mut self, tp: &TrackPoint) -> Result<()> {
    let header = self.read_file_header()?;
    let count = header.count as usize;
    let offset = if count < 2 {
      // if less than 2 points exist, append only
      0
    } else {
      let mut last_two = self.read_multiple_at(count - 2, 2)?;
      let last = last_two.pop().unwrap();
      let prev = last_two.pop().unwrap();
      if last == prev && prev == *tp {
        // if the last two points are equal and the new one equals to them
        // replace the last one, overwriting only timestamp
        -(size_of::<TrackPoint>() as i64)
      } else {
        // otherwise, append
        0
      }
    };

    let data = to_raw(tp);
    self.file.seek(SeekFrom::End(offset))?;
    self.file.write_all(&data)?;
    self.inc()?;
    Ok(())
  }

  pub fn read_multiple_at(&self, pos: usize, len: usize) -> Result<Vec<TrackPoint>> {
    let header = self.read_file_header()?;
    let count = header.count as usize;
    let mut len = len;

    if pos + len > count {
      len = count - pos;
    }

    if len < 1 {
      return Ok(Vec::new());
    }

    let mut buf = vec![];
    let tplen = size_of::<TrackPoint>();
    buf.resize(len * tplen, 0);

    let offset = size_of::<TrackFileHeader>() + pos * tplen;
    self.file.read_at(&mut buf, offset as u64)?;

    let mut points = vec![];
    for idx in 0..len {
      let start = idx * tplen;
      let end = (idx + 1) * tplen;
      let tp = from_raw(&buf[start..end])?;
      points.push(tp);
    }

    Ok(points)
  }

  pub fn read_at(&self, pos: usize) -> Result<TrackPoint> {
    let header = self.read_file_header()?;
    if pos as u64 >= header.count {
      Err(TrackFileError(format!("index {pos} out of bounds")))
    } else {
      let mut buf = [0; size_of::<TrackPoint>()];
      let offset = size_of::<TrackFileHeader>() + pos * size_of::<TrackPoint>();
      self.file.read_at(&mut buf, offset as u64)?;
      let tp = from_raw(&buf)?;
      Ok(tp)
    }
  }

  pub fn read_all(&self) -> Result<Vec<TrackPoint>> {
    let header = self.read_file_header()?;
    let mut buf = [0; size_of::<TrackPoint>()];
    let mut res = vec![];
    for idx in 0..header.count {
      let idx = idx as usize;
      let offset = size_of::<TrackFileHeader>() + idx * size_of::<TrackPoint>();
      self.file.read_at(&mut buf, offset as u64)?;
      let tp = from_raw(&buf)?;
      res.push(tp);
    }
    Ok(res)
  }
}
