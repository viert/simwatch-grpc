use crate::{moving::pilot::Pilot, service::camden};

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
