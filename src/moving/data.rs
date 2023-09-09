use super::{
  controller::{Controller, Facility},
  pilot::Pilot,
};
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct General {
  pub version: u64,
  pub reload: u64,
  pub connected_clients: u32,
  pub unique_users: u32,
  pub updated_at: DateTime<Utc>,
}

impl From<super::exttypes::General> for General {
  fn from(src: super::exttypes::General) -> Self {
    let updated_at = DateTime::parse_from_rfc3339(&src.update_timestamp)
      .map(|dt| dt.with_timezone(&Utc))
      .unwrap_or_else(|_| Utc::now());
    Self {
      version: src.version,
      reload: src.reload,
      connected_clients: src.connected_clients,
      unique_users: src.unique_users,
      updated_at,
    }
  }
}

#[derive(Debug)]
pub struct Data {
  pub general: General,
  pub pilots: Vec<Pilot>,
  pub controllers: Vec<Controller>,
}

impl From<super::exttypes::Data> for Data {
  fn from(src: super::exttypes::Data) -> Self {
    let pilots: Vec<Pilot> = src.pilots.into_iter().map(|p| p.into()).collect();
    let mut controllers: Vec<Controller> = src.controllers.into_iter().map(|c| c.into()).collect();
    for ctrl in src.atis {
      let mut ctrl: Controller = ctrl.into();
      ctrl.facility = Facility::ATIS;
      controllers.push(ctrl);
    }

    Self {
      general: src.general.into(),
      pilots,
      controllers,
    }
  }
}
