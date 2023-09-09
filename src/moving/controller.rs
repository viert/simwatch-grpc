use std::fmt::Display;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::service::camden;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Facility {
  Reject = 0,
  ATIS = 1,
  Delivery = 2,
  Ground = 3,
  Tower = 4,
  Approach = 5,
  Radar = 6,
}

impl Display for Facility {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Facility::Reject => write!(f, "reject"),
      Facility::ATIS => write!(f, "atis"),
      Facility::Delivery => write!(f, "delivery"),
      Facility::Ground => write!(f, "ground"),
      Facility::Tower => write!(f, "tower"),
      Facility::Approach => write!(f, "approach"),
      Facility::Radar => write!(f, "radar"),
    }
  }
}

impl From<i8> for Facility {
  fn from(v: i8) -> Self {
    match v {
      1 => Facility::ATIS,
      2 => Facility::Delivery,
      3 => Facility::Ground,
      4 => Facility::Tower,
      5 => Facility::Approach,
      6 => Facility::Radar,
      _ => Facility::Reject,
    }
  }
}

impl From<Facility> for camden::Facility {
  fn from(value: Facility) -> Self {
    match value {
      Facility::Reject => camden::Facility::Reject,
      Facility::ATIS => camden::Facility::Atis,
      Facility::Delivery => camden::Facility::Delivery,
      Facility::Ground => camden::Facility::Ground,
      Facility::Tower => camden::Facility::Tower,
      Facility::Approach => camden::Facility::Approach,
      Facility::Radar => camden::Facility::Radar,
    }
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct Controller {
  pub cid: u32,
  pub name: String,
  pub callsign: String,
  pub freq: u32,
  pub facility: Facility,
  pub rating: i32,
  pub server: String,
  pub visual_range: u32,
  pub atis_code: String,
  pub text_atis: String,
  pub human_readable: Option<String>,
  pub last_updated: DateTime<Utc>,
  pub logon_time: DateTime<Utc>,
}

impl PartialEq for Controller {
  // custom PartialEq for Controller as we don't care about last_updated
  // field as long as the others stay the same
  fn eq(&self, other: &Self) -> bool {
    self.cid == other.cid
      && self.name == other.name
      && self.callsign == other.callsign
      && self.freq == other.freq
      && self.facility == other.facility
      && self.rating == other.rating
      && self.server == other.server
      && self.visual_range == other.visual_range
      && self.atis_code == other.atis_code
      && self.text_atis == other.text_atis
      && self.human_readable == other.human_readable
      && self.logon_time == other.logon_time
  }
}

impl From<Controller> for camden::Controller {
  fn from(value: Controller) -> Self {
    let facility: camden::Facility = value.facility.into();
    Self {
      cid: value.cid,
      name: value.name,
      callsign: value.callsign,
      freq: value.freq,
      facility: facility as i32,
      rating: value.rating,
      server: value.server,
      visual_range: value.visual_range,
      atis_code: value.atis_code,
      text_atis: value.text_atis,
      human_readable: value.human_readable,
      last_updated: value.last_updated.timestamp_millis() as u64,
      logon_time: value.logon_time.timestamp_millis() as u64,
    }
  }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct ControllerSet {
  pub atis: Option<Controller>,
  pub delivery: Option<Controller>,
  pub ground: Option<Controller>,
  pub tower: Option<Controller>,
  pub approach: Option<Controller>,
}

impl ControllerSet {
  pub fn empty() -> Self {
    Self {
      atis: None,
      delivery: None,
      ground: None,
      tower: None,
      approach: None,
    }
  }

  pub fn is_empty(&self) -> bool {
    self.atis.is_none()
      && self.delivery.is_none()
      && self.ground.is_none()
      && self.tower.is_none()
      && self.approach.is_none()
  }
}

impl From<ControllerSet> for camden::ControllerSet {
  fn from(value: ControllerSet) -> Self {
    Self {
      atis: value.atis.map(|v| v.into()),
      delivery: value.delivery.map(|v| v.into()),
      ground: value.ground.map(|v| v.into()),
      tower: value.tower.map(|v| v.into()),
      approach: value.approach.map(|v| v.into()),
    }
  }
}

impl From<super::exttypes::Controller> for Controller {
  fn from(ctrl: super::exttypes::Controller) -> Self {
    let freq = ctrl.frequency.parse::<f64>().unwrap_or(0.0);
    let freq = freq * 1000.0;
    let freq = freq as u32;
    let facility: Facility = ctrl.facility.into();

    let text_atis = if let Some(ta) = ctrl.text_atis {
      ta.join("\n")
    } else {
      "".to_owned()
    };
    let now = Utc::now();

    let logon_time = DateTime::parse_from_rfc3339(&ctrl.logon_time)
      .map(|dt| dt.with_timezone(&Utc))
      .unwrap_or(now);
    let last_updated = DateTime::parse_from_rfc3339(&ctrl.last_updated)
      .map(|dt| dt.with_timezone(&Utc))
      .unwrap_or(now);

    Self {
      cid: ctrl.cid,
      name: ctrl.name,
      callsign: ctrl.callsign,
      freq,
      facility,
      rating: ctrl.rating,
      server: ctrl.server,
      visual_range: ctrl.visual_range,
      atis_code: ctrl.atis_code.unwrap_or_else(|| "".to_owned()),
      text_atis,
      last_updated,
      logon_time,
      human_readable: None,
    }
  }
}
