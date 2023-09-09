use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FlightPlan {
  pub flight_rules: String,
  pub aircraft: String,
  pub departure: String,
  pub arrival: String,
  pub alternate: String,
  pub cruise_tas: String,
  pub altitude: String,
  pub deptime: String,
  pub enroute_time: String,
  pub fuel_time: String,
  pub remarks: String,
  pub route: String,
}

#[derive(Debug, Deserialize)]
pub struct Pilot {
  pub cid: u32,
  pub name: String,
  pub callsign: String,
  pub server: String,
  pub pilot_rating: i32,
  pub latitude: f64,
  pub longitude: f64,
  pub altitude: i32,
  pub groundspeed: i32,
  pub transponder: String,
  pub heading: i16,
  pub qnh_i_hg: f64,
  pub qnh_mb: i16,
  pub flight_plan: Option<FlightPlan>,
  pub logon_time: String,
  pub last_updated: String,
}

#[derive(Debug, Deserialize)]
pub struct General {
  pub version: u64,
  pub reload: u64,
  pub update: String,
  pub update_timestamp: String,
  pub connected_clients: u32,
  pub unique_users: u32,
}

#[derive(Debug, Deserialize)]
pub struct Controller {
  pub cid: u32,
  pub callsign: String,
  pub name: String,
  pub frequency: String,
  pub facility: i8,
  pub rating: i32,
  pub server: String,
  pub visual_range: u32,
  pub atis_code: Option<String>,
  pub text_atis: Option<Vec<String>>,
  pub logon_time: String,
  pub last_updated: String,
}

#[derive(Debug, Deserialize)]
pub struct Data {
  pub general: General,
  pub pilots: Vec<Pilot>,
  pub controllers: Vec<Controller>,
  pub atis: Vec<Controller>,
}
