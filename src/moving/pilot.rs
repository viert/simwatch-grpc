use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{service::camden, types::Point};

use super::aircraft::{guess_aircraft_types, Aircraft};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pilot {
  pub cid: u32,
  pub name: String,
  pub callsign: String,
  pub server: String,
  pub pilot_rating: i32,
  pub position: Point,
  pub altitude: i32,
  pub groundspeed: i32,
  pub transponder: String,
  pub heading: i16,
  pub qnh_i_hg: u16,
  pub qnh_mb: u16,
  pub flight_plan: Option<FlightPlan>,
  pub logon_time: DateTime<Utc>,
  pub last_updated: DateTime<Utc>,
  pub aircraft_type: Option<Vec<&'static Aircraft>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FlightPlan {
  pub flight_rules: String,
  pub aircraft: String,
  pub departure: String,
  pub arrival: String,
  pub alternate: String,
  pub cruise_tas: u16,
  pub altitude: u16,
  pub deptime: String,
  pub enroute_time: String,
  pub fuel_time: String,
  pub remarks: String,
  pub route: String,
}

impl From<crate::moving::exttypes::FlightPlan> for FlightPlan {
  fn from(src: crate::moving::exttypes::FlightPlan) -> Self {
    // Use this type converter to normalise FlightPlan data and
    // fix user errors

    let cruise_tas = src.cruise_tas.parse::<u16>().unwrap_or(0);
    let altitude = src.altitude.parse::<u16>().unwrap_or(0);

    Self {
      flight_rules: src.flight_rules,
      aircraft: src.aircraft,
      departure: src.departure,
      arrival: src.arrival,
      alternate: src.alternate,
      cruise_tas,
      altitude,
      deptime: src.deptime,
      enroute_time: src.enroute_time,
      fuel_time: src.fuel_time,
      remarks: src.remarks,
      route: src.route,
    }
  }
}

impl From<FlightPlan> for camden::FlightPlan {
  fn from(value: FlightPlan) -> Self {
    Self {
      flight_rules: value.flight_rules,
      aircraft: value.aircraft,
      departure: value.departure,
      arrival: value.arrival,
      alternate: value.alternate,
      cruise_tas: value.cruise_tas as u32,
      altitude: value.altitude as u32,
      deptime: value.deptime,
      enroute_time: value.enroute_time,
      fuel_time: value.fuel_time,
      remarks: value.remarks,
      route: value.route,
    }
  }
}

impl From<crate::moving::exttypes::Pilot> for Pilot {
  fn from(src: crate::moving::exttypes::Pilot) -> Self {
    let qnh_i_hg = (src.qnh_i_hg * 100.0).round() as u16;
    let now = Utc::now();
    let logon_time = DateTime::parse_from_rfc3339(&src.logon_time)
      .map(|dt| dt.with_timezone(&Utc))
      .unwrap_or(now);
    let last_updated = DateTime::parse_from_rfc3339(&src.last_updated)
      .map(|dt| dt.with_timezone(&Utc))
      .unwrap_or(now);

    let flight_plan: Option<FlightPlan> = src.flight_plan.map(|fp| fp.into());
    let aircraft_type = if let Some(fp) = &flight_plan {
      guess_aircraft_types(&fp.aircraft)
    } else {
      None
    };

    Self {
      cid: src.cid,
      name: src.name,
      callsign: src.callsign,
      server: src.server,
      pilot_rating: src.pilot_rating,
      position: Point {
        lat: src.latitude,
        lng: src.longitude,
      },
      altitude: src.altitude,
      groundspeed: src.groundspeed,
      transponder: src.transponder,
      heading: src.heading,
      qnh_i_hg,
      qnh_mb: src.qnh_mb as u16,
      flight_plan,
      logon_time,
      last_updated,
      aircraft_type,
    }
  }
}

impl From<Pilot> for camden::Pilot {
  fn from(value: Pilot) -> Self {
    let aircraft_type = match value.aircraft_type {
      Some(ats) => ats.into_iter().map(|at| at.into()).collect(),
      None => vec![],
    };

    Self {
      cid: value.cid,
      name: value.name,
      callsign: value.callsign,
      server: value.server,
      pilot_rating: value.pilot_rating,
      position: Some(value.position.into()),
      altitude: value.altitude,
      groundspeed: value.groundspeed,
      transponder: value.transponder,
      heading: value.heading as i32,
      qnh_i_hg: value.qnh_i_hg as u32,
      qnh_mb: value.qnh_mb as u32,
      flight_plan: value.flight_plan.map(|fp| fp.into()),
      last_updated: value.last_updated.timestamp_millis() as u64,
      logon_time: value.logon_time.timestamp_millis() as u64,
      track: vec![],
      aircraft_type,
    }
  }
}
