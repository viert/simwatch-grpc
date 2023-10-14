use chrono::Utc;

use crate::trackfile::TrackFileHeader;

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
