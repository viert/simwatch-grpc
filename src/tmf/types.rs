use chrono::Utc;

use crate::trackfile::TrackFileHeader;

use super::proto::{track_message::Union, TrackMessage};

const TRACK_VERSION: u64 = 1;
const TRACK_MAGIC_NUMBER: u64 = 0xAA99AA881011889C;

#[derive(Debug, Clone)]
pub struct TrackPoint {
  pub ts: u64,
  pub lat: f64,
  pub lng: f64,
  pub hdg_true: f64,
  pub alt_amsl: f64,
  pub alt_agl: f64,
  pub gnd_height: f64,
  pub crs: f64,
  pub ias: f64,
  pub tas: f64,
  pub gs: f64,
  pub ap_master: bool,
  pub gear_pct: i64,
  pub flaps: i64,
  pub on_gnd: bool,
  pub on_rwy: bool,
  pub wind_vel: f64,
  pub wind_dir: f64,
  pub distance: f64,
}

#[derive(Debug, Clone)]
pub struct TouchDown {
  pub ts: u64,
  pub bank: f64,
  pub hdg_mag: f64,
  pub hdg_true: f64,
  pub vel_nrm: f64,
  pub pitch: f64,
  pub lat: f64,
  pub lng: f64,
}

#[derive(Debug, Clone)]
pub enum TrackEntry {
  TrackPoint(TrackPoint),
  TouchDown(TouchDown),
}

impl From<TrackMessage> for TrackEntry {
  fn from(value: TrackMessage) -> Self {
    match value.union.unwrap() {
      Union::Point(point) => Self::TrackPoint(TrackPoint {
        ts: point.ts,
        lat: point.lat,
        lng: point.lng,
        hdg_true: point.hdg_true,
        alt_amsl: point.alt_amsl,
        alt_agl: point.alt_agl,
        gnd_height: point.gnd_height,
        crs: point.crs,
        ias: point.ias,
        tas: point.tas,
        gs: point.gs,
        ap_master: point.ap_master,
        gear_pct: point.gear_pct,
        flaps: point.flaps,
        on_gnd: point.on_gnd,
        on_rwy: point.on_rwy,
        wind_vel: point.wind_vel,
        wind_dir: point.wind_dir,
        distance: point.distance,
      }),
      Union::TouchDown(td) => Self::TouchDown(TouchDown {
        ts: td.ts,
        bank: td.bank,
        hdg_mag: td.hdg_mag,
        hdg_true: td.hdg_true,
        vel_nrm: td.vel_nrm,
        pitch: td.pitch,
        lat: td.lat,
        lng: td.lng,
      }),
    }
  }
}

impl PartialEq for TrackEntry {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::TrackPoint(l0), Self::TrackPoint(r0)) => {
        l0.lat == r0.lat
          && l0.lng == r0.lng
          && l0.hdg_true == r0.hdg_true
          && l0.alt_amsl == r0.alt_amsl
          && l0.alt_agl == r0.alt_agl
          && l0.gnd_height == r0.gnd_height
          && l0.crs == r0.crs
          && l0.ias == r0.ias
          && l0.tas == r0.tas
          && l0.gs == r0.gs
          && l0.ap_master == r0.ap_master
          && l0.gear_pct == r0.gear_pct
          && l0.flaps == r0.flaps
          && l0.on_gnd == r0.on_gnd
          && l0.on_rwy == r0.on_rwy
          && l0.wind_dir == r0.wind_dir
          && l0.wind_vel == r0.wind_vel
          && l0.distance == r0.distance
      }
      (Self::TouchDown(_), Self::TouchDown(_)) => false,
      _ => false,
    }
  }
}

#[derive(Debug, Clone)]
pub struct Header {
  version: u64,
  magic: u64,
  ts: u64,
  count: u64,
  uuid: [u8; 36],
}

impl Header {
  pub fn uuid(&self) -> String {
    String::from_utf8(self.uuid.to_vec()).unwrap()
  }
  pub fn set_uuid(&mut self, uuid: &str) {
    for (i, c) in uuid.as_bytes().into_iter().enumerate() {
      self.uuid[i] = *c;
    }
  }
}

impl Default for Header {
  fn default() -> Self {
    let mut header = Self {
      magic: TRACK_MAGIC_NUMBER,
      version: TRACK_VERSION,
      ts: Utc::now().timestamp_millis() as u64,
      count: 0,
      uuid: [0; 36],
    };
    header.set_uuid("47cb897f-ada4-4a94-b9c3-ad8e4dd1f73f");
    header
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
    self.count += 1;
    self.ts = Utc::now().timestamp_millis() as u64;
  }
}
