use chrono::{DateTime, Utc};
use std::{
  error::Error,
  fmt::{Debug, Display},
  fs::{File, OpenOptions},
  io::{Seek, SeekFrom, Write},
  marker::PhantomData,
  mem::size_of,
  os::unix::prelude::FileExt,
  ptr::slice_from_raw_parts,
};
use tonic::Status;

pub type Result<T> = std::result::Result<T, TrackFileError>;

#[derive(Debug)]
pub enum TrackFileError {
  IOError(std::io::Error),
  InvalidMagicNumber,
  InvalidFileLength(usize, usize),
  InsufficientDataLength(usize),
  IndexError(usize),
}

impl Display for TrackFileError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      TrackFileError::IOError(err) => write!(f, "TrackFileError: {err}"),
      TrackFileError::InvalidMagicNumber => write!(f, "Track file corrupted, invalid magic number"),
      TrackFileError::InvalidFileLength(expected, got) => write!(
        f,
        "Invalid track file length: expected {expected}, got {got}"
      ),
      TrackFileError::InsufficientDataLength(size) => write!(
        f,
        "Insufficient data length while parsing track file entry: {size}"
      ),
      TrackFileError::IndexError(idx) => {
        write!(f, "Invalid index {idx} while reading track file data")
      }
    }
  }
}

impl Error for TrackFileError {}

impl From<std::io::Error> for TrackFileError {
  fn from(value: std::io::Error) -> Self {
    Self::IOError(value)
  }
}

impl From<TrackFileError> for Status {
  fn from(value: TrackFileError) -> Self {
    Status::internal(format!("{value}"))
  }
}

pub trait TrackFileHeader: Sized + Clone + Default {
  fn check_magic(&self) -> bool;
  fn version(&self) -> u64;
  fn timestamp(&self) -> u64;
  fn count(&self) -> u64;
  fn inc(&mut self);
}

fn to_raw<T: Sized>(obj: &T) -> Vec<u8> {
  let slice = slice_from_raw_parts(obj, size_of::<T>()) as *const [u8];
  let slice = unsafe { &*slice };
  slice.into()
}

fn from_raw<T: Sized + Clone>(data: &[u8]) -> std::result::Result<T, TrackFileError> {
  if data.len() < size_of::<T>() {
    Err(TrackFileError::InsufficientDataLength(data.len()))
  } else {
    let slice = data as *const [u8] as *const T;
    let tp = unsafe { &*slice };
    Ok(tp.clone())
  }
}

pub struct TrackFile<E: Clone + Sized + PartialEq, H: TrackFileHeader> {
  file: File,
  name: String,
  phantom_e: PhantomData<E>,
  phantom_h: PhantomData<H>,
}

impl<E: Clone + Sized + PartialEq, H: TrackFileHeader> TrackFile<E, H> {
  pub fn new(filename: &str) -> Result<Self> {
    let res = OpenOptions::new().write(true).read(true).open(&filename);

    let tf = match res {
      Ok(file) => Self {
        file,
        name: filename.to_owned(),
        phantom_e: PhantomData,
        phantom_h: PhantomData,
      },
      Err(err) => match err.kind() {
        std::io::ErrorKind::NotFound => {
          let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&filename)?;
          let header = H::default();
          let raw_header = to_raw(&header);
          file.write_all(&raw_header)?;
          Self {
            file,
            name: filename.to_owned(),
            phantom_e: PhantomData,
            phantom_h: PhantomData,
          }
        }
        _ => return Err(err.into()),
      },
    };
    tf.check()?;
    Ok(tf)
  }

  fn check(&self) -> Result<()> {
    let header = self.read_file_header()?;
    if !header.check_magic() {
      Err(TrackFileError::InvalidMagicNumber)
    } else {
      let meta = std::fs::metadata(&self.name)?;
      let expected_len = (header.count() as usize) * Self::entry_size() + Self::header_size();
      let real_len = meta.len() as usize;
      if real_len != expected_len {
        Err(TrackFileError::InvalidFileLength(expected_len, real_len))
      } else {
        Ok(())
      }
    }
  }

  fn make_entry_buf() -> Vec<u8> {
    let mut buf = vec![];
    buf.resize(Self::entry_size(), 0);
    buf
  }

  fn make_header_buf() -> Vec<u8> {
    let mut buf = vec![];
    buf.resize(Self::header_size(), 0);
    buf
  }

  const fn entry_size() -> usize {
    size_of::<E>()
  }

  const fn header_size() -> usize {
    size_of::<H>()
  }

  fn read_file_header(&self) -> Result<H> {
    let mut buf = Self::make_header_buf();
    self.file.read_at(&mut buf, 0)?;
    Ok(from_raw(&buf)?)
  }

  fn write_file_header(&mut self, header: &H) -> Result<()> {
    let buf = to_raw(header);
    self.file.write_at(&buf, 0)?;
    Ok(())
  }

  fn inc(&mut self) -> Result<()> {
    let mut header = self.read_file_header()?;
    header.inc();
    self.write_file_header(&header)?;
    Ok(())
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn mtime(&self) -> Result<DateTime<Utc>> {
    let header = self.read_file_header()?;
    let secs = header.timestamp() / 1000;
    let nsecs = (header.timestamp() % 1000) * 1000;
    let dt = DateTime::from_timestamp(secs as i64, nsecs as u32).unwrap_or(Utc::now());
    Ok(dt)
  }

  pub fn count(&self) -> Result<u64> {
    let header = self.read_file_header()?;
    Ok(header.count())
  }

  pub fn destroy(self) -> Result<()> {
    std::fs::remove_file(&self.name)?;
    Ok(())
  }

  pub fn append(&mut self, e: &E) -> Result<()> {
    let header = self.read_file_header()?;
    let count = header.count() as usize;
    let offset = if count < 2 {
      // if less than 2 points exist, append only
      0
    } else {
      let mut last_two = self.read_multiple_at(count - 2, 2)?;
      let last = last_two.pop().unwrap();
      let prev = last_two.pop().unwrap();
      if last == prev && prev == *e {
        // if the last two points are equal and the new one equals to them
        // replace the last one, overwriting only timestamp
        -(Self::entry_size() as i64)
      } else {
        // otherwise, append
        0
      }
    };

    if offset == 0 {
      self.inc()?
    }

    let data = to_raw(e);
    self.file.seek(SeekFrom::End(offset))?;
    self.file.write_all(&data)?;
    Ok(())
  }

  pub fn read_at(&self, pos: usize) -> Result<E> {
    let header = self.read_file_header()?;
    if pos as u64 >= header.count() {
      Err(TrackFileError::IndexError(pos))
    } else {
      let mut buf = Self::make_entry_buf();
      let offset = Self::header_size() + pos * Self::entry_size();
      self.file.read_at(&mut buf, offset as u64)?;
      let e = from_raw(&buf)?;
      Ok(e)
    }
  }

  pub fn read_multiple_at(&self, pos: usize, len: usize) -> Result<Vec<E>> {
    let header = self.read_file_header()?;
    let count = header.count() as usize;
    let mut len = len;

    if pos + len > count {
      len = count - pos;
    }

    if len < 1 {
      return Ok(Vec::new());
    }

    let mut buf = vec![];
    let entry_len = Self::entry_size();
    buf.resize(len * entry_len, 0);

    let offset = Self::header_size() + pos * entry_len;
    self.file.read_at(&mut buf, offset as u64)?;

    let mut entries = vec![];
    for idx in 0..len {
      let start = idx * entry_len;
      let end = (idx + 1) * entry_len;
      let e = from_raw(&buf[start..end])?;
      entries.push(e);
    }

    Ok(entries)
  }

  pub fn read_all(&self) -> Result<Vec<E>> {
    let header = self.read_file_header()?;

    let mut buf = Self::make_entry_buf();
    let mut res = vec![];
    for idx in 0..header.count() {
      let idx = idx as usize;
      let offset = Self::header_size() + idx * Self::entry_size();
      self.file.read_at(&mut buf, offset as u64)?;
      let tp = from_raw(&buf)?;
      res.push(tp);
    }
    Ok(res)
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;
  use std::{
    env::temp_dir,
    fs::{self, remove_file},
    io::Read,
  };

  const TRACK_VERSION: u64 = 1;
  const TRACK_MAGIC_NUMBER: u64 = 0x119F3E5F006A42C8;

  #[derive(Debug, Clone)]
  #[repr(C)]
  pub struct Header {
    magic: u64,
    version: u64,
    ts: u64,
    count: u64,
  }

  impl Default for Header {
    fn default() -> Self {
      Self {
        magic: TRACK_MAGIC_NUMBER,
        version: TRACK_VERSION,
        ts: Utc::now().timestamp_millis() as u64,
        count: 0,
      }
    }
  }

  impl TrackFileHeader for Header {
    fn check_magic(&self) -> bool {
      self.magic == TRACK_MAGIC_NUMBER
    }

    fn version(&self) -> u64 {
      self.version
    }

    fn timestamp(&self) -> u64 {
      self.ts
    }

    fn count(&self) -> u64 {
      self.count
    }

    fn inc(&mut self) {
      self.ts = Utc::now().timestamp_millis() as u64;
      self.count += 1;
    }
  }

  #[derive(Clone, Debug)]
  struct Entry {
    value: u32,
  }

  impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
      self.value == other.value
    }
  }

  fn vec_compare(v1: &[u8], v2: &[u8]) -> bool {
    v1.len() == v2.len() && v1.iter().zip(v2).all(|(i1, i2)| *i1 == *i2)
  }

  #[test]
  fn test_track_file() {
    let path = temp_dir();
    let path = path.join("track.bin");
    let path = path.to_str().unwrap();
    let _ = remove_file(path);
    {
      let mut tf: TrackFile<Entry, Header> = TrackFile::new(path).unwrap();
      let res = tf.append(&Entry { value: 1 });
      assert!(res.is_ok());
      let res = tf.append(&Entry { value: 2 });
      assert!(res.is_ok());
      let res = tf.append(&Entry { value: 2 });
      assert!(res.is_ok());
      let res = tf.append(&Entry { value: 2 });
      assert!(res.is_ok());
    }

    let meta = fs::metadata(path).unwrap();
    let expected_len = 3 * size_of::<Entry>() + size_of::<Header>();
    assert_eq!(expected_len, meta.len() as usize);

    let mut raw = vec![];
    raw.resize(size_of::<Header>(), 0);

    let mut f = File::open(path).unwrap();
    f.read(&mut raw).unwrap();

    let expected_raw = &[
      0xc8, 0x42, 0x6a, 0x00, 0x5f, 0x3e, 0x9f, 0x11, // magic
      0x01, 0, 0, 0, 0, 0, 0, 0, // version
    ];
    assert!(vec_compare(&raw[..16], expected_raw));

    let expected_count = 3;
    assert_eq!(raw[24], expected_count);

    remove_file(path).unwrap();
  }
}
