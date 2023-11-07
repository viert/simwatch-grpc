pub mod header;
pub mod trackpoint;
use self::{header::Header, trackpoint::TrackPoint};
use crate::moving::pilot::Pilot;
use crate::trackfile::{Result, TrackFile};
use chrono::{Duration, Utc};
use log::debug;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Store {
  folder: String,
}

impl Store {
  pub fn new(folder: &str) -> Self {
    Self {
      folder: folder.to_owned(),
    }
  }

  fn collect_track_files<T: AsRef<Path>>(
    &self,
    path: Option<T>,
  ) -> Result<Vec<TrackFile<TrackPoint, Header>>> {
    let real_path = match path {
      Some(ref path) => path.as_ref(),
      None => Path::new(&self.folder),
    };

    let mut files = vec![];

    let contents = std::fs::read_dir(real_path)?;
    for dir_entry in contents.flatten() {
      let ft = dir_entry.file_type();
      if let Ok(ft) = ft {
        if ft.is_dir() {
          let dir_path = real_path.join(dir_entry.file_name());
          files.extend(self.collect_track_files(Some(dir_path))?);
        } else if ft.is_file() {
          let filename = real_path.join(dir_entry.file_name());
          let filename = filename.to_str().unwrap();
          let tf = TrackFile::new(filename);
          if let Ok(tf) = tf {
            files.push(tf)
          }
        }
      }
    }
    Ok(files)
  }

  pub fn counters(&self) -> Result<(u64, u64)> {
    let mut track_count = 0;
    let mut track_point_count = 0;
    for file in self.collect_track_files::<&str>(None)? {
      let count = file.count();
      if let Ok(count) = count {
        track_count += 1;
        track_point_count += count;
      }
    }
    Ok((track_count, track_point_count))
  }

  pub fn cleanup(&self) -> Result<()> {
    for file in self.collect_track_files::<&str>(None)? {
      let mtime = file.mtime();
      if let Ok(mtime) = mtime {
        let min_date = Utc::now() - Duration::days(2);
        if mtime < min_date {
          debug!("destroying file {} older than {:?}", file.name(), min_date);
          let _ = file.destroy();
        }
      }
    }
    Ok(())
  }

  fn pilot_track_filename(&self, pilot: &Pilot) -> String {
    let first = format!("{}", pilot.cid / 10000);
    let second = format!("{}", pilot.cid);
    let pilot_track_folder = Path::new(&self.folder).join(first).join(second);
    let pilot_track_filename = format!(
      "{}.{}.{}.bin",
      pilot.cid,
      pilot.callsign,
      pilot.logon_time.timestamp()
    );
    let pilot_track_filename = pilot_track_folder.join(pilot_track_filename);
    format!("{}", pilot_track_filename.display())
  }

  fn get_pilot_track_file(&self, pilot: &Pilot) -> Result<TrackFile<TrackPoint, Header>> {
    let filename = self.pilot_track_filename(pilot);
    let mut buf = PathBuf::from(&filename);
    buf.pop();
    if !Path::is_dir(&buf) {
      std::fs::create_dir_all(&buf)?;
    }
    let pilot_track = TrackFile::new(&filename)?;
    Ok(pilot_track)
  }

  pub fn store_track(&self, pilot: &Pilot) -> Result<()> {
    let mut pilot_track = self.get_pilot_track_file(pilot)?;
    let track_point = pilot.into();
    pilot_track.append(&track_point)?;
    Ok(())
  }

  pub fn get_track_points(&self, pilot: &Pilot) -> Result<Vec<TrackPoint>> {
    let pilot_track = self.get_pilot_track_file(pilot)?;
    let points = pilot_track.read_all()?;
    Ok(points)
  }
}
