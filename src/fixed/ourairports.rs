use chrono::Utc;
use csv::StringRecord;
use log::{error, info};
use serde::Serialize;
use std::{
  collections::HashMap,
  error::Error,
  fmt::Display,
  fs::File,
  num::{ParseFloatError, ParseIntError},
};

use crate::{config::Config, fixed::cached_loader, service::camden, util::seconds_since};

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Runway {
  pub icao: String,
  pub length_ft: u32,
  pub width_ft: u32,
  pub surface: String,
  pub lighted: bool,
  pub closed: bool,
  pub ident: String,
  pub latitude: f64,
  pub longitude: f64,
  pub elevation_ft: i32,
  pub heading: u16,
  pub active_to: bool,
  pub active_lnd: bool,
}

impl From<Runway> for camden::Runway {
  fn from(value: Runway) -> Self {
    Self {
      icao: value.icao,
      length_ft: value.length_ft,
      width_ft: value.width_ft,
      surface: value.surface,
      lighted: value.lighted,
      closed: value.closed,
      ident: value.ident,
      latitude: value.latitude,
      longitude: value.longitude,
      elevation_ft: value.elevation_ft,
      heading: value.heading as i32,
      active_to: value.active_to,
      active_lnd: value.active_to,
    }
  }
}

#[derive(Debug)]
pub enum ParseError {
  ParseStringError(String),
  ParseIntError(String, ParseIntError),
  ParseFloatError(String, ParseFloatError),
}

impl Display for ParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ParseError::ParseStringError(s) => write!(f, "error parsing quoted string '{}'", s),
      ParseError::ParseIntError(s, err) => write!(f, "{} @ {}", err, s),
      ParseError::ParseFloatError(s, err) => write!(f, "{} @ {}", err, s),
    }
  }
}

impl Error for ParseError {}

fn parse_u32(s: &str) -> Result<u32, ParseError> {
  s.parse::<u32>()
    .map_err(|err| ParseError::ParseIntError(s.to_owned(), err))
}

fn parse_i32(s: &str) -> Result<i32, ParseError> {
  s.parse::<i32>()
    .map_err(|err| ParseError::ParseIntError(s.to_owned(), err))
}

fn parse_f64(s: &str) -> Result<f64, ParseError> {
  s.parse::<f64>()
    .map_err(|err| ParseError::ParseFloatError(s.to_owned(), err))
}

fn parse_runway(tokens: &StringRecord) -> Result<(Runway, Runway), ParseError> {
  let icao = &tokens[2];
  let length_ft = parse_u32(&tokens[3]).unwrap_or(0);
  let width_ft = parse_u32(&tokens[4]).unwrap_or(0);
  let surface = &tokens[5];
  let lighted = &tokens[6] == "1";
  let closed = &tokens[7] == "1";
  let le_ident = &tokens[8];
  let le_lat = parse_f64(&tokens[9]).unwrap_or(0.0);
  let le_lng = parse_f64(&tokens[10]).unwrap_or(0.0);
  let le_elev = parse_i32(&tokens[11]).unwrap_or(0);
  let le_hdg = parse_f64(&tokens[12]).unwrap_or(0.0);
  let he_ident = &tokens[14];
  let he_lat = parse_f64(&tokens[15]).unwrap_or(0.0);
  let he_lng = parse_f64(&tokens[16]).unwrap_or(0.0);
  let he_elev = parse_i32(&tokens[17]).unwrap_or(0);
  let he_hdg = parse_f64(&tokens[18]).unwrap_or(0.0);

  let rwy1 = Runway {
    icao: icao.into(),
    length_ft,
    width_ft,
    surface: surface.into(),
    lighted,
    closed,
    ident: le_ident.into(),
    latitude: le_lat,
    longitude: le_lng,
    elevation_ft: le_elev,
    heading: le_hdg as u16,
    active_to: false,
    active_lnd: false,
  };
  let rwy2 = Runway {
    icao: icao.into(),
    length_ft,
    width_ft,
    surface: surface.into(),
    lighted,
    closed,
    ident: he_ident.into(),
    latitude: he_lat,
    longitude: he_lng,
    elevation_ft: he_elev,
    heading: he_hdg as u16,
    active_to: false,
    active_lnd: false,
  };
  Ok((rwy1, rwy2))
}

async fn parse(src: File) -> Result<HashMap<String, Vec<Runway>>, Box<dyn Error>> {
  let mut rdr = csv::Reader::from_reader(src);
  let mut runways: HashMap<String, Vec<Runway>> = HashMap::new();

  for record in rdr.records() {
    let record = record?;
    let res = parse_runway(&record);
    if let Err(err) = res {
      error!("error parsing runway {:?}: {}", &record, err);
      continue;
    }

    let (rwy1, rwy2) = res.unwrap();

    let rwys = runways.get_mut(&rwy1.icao);
    if let Some(rwys) = rwys {
      rwys.push(rwy1);
      rwys.push(rwy2);
    } else {
      runways.insert(rwy1.icao.clone(), vec![rwy1, rwy2]);
    }
  }
  Ok(runways)
}

pub async fn load_runways(cfg: &Config) -> Result<HashMap<String, Vec<Runway>>, Box<dyn Error>> {
  let cache_file = cached_loader(&cfg.fixed.runways_url, &cfg.cache.runways).await?;
  let t = Utc::now();
  let res = parse(cache_file).await;
  info!("runways data parsed in {}s", seconds_since(t));
  res
}

#[cfg(test)]
mod tests {
  use super::{parse_runway, Runway};
  use csv::StringRecord;

  const TEST_RUNWAY: &str = "239398,2434,EGLL,12001,148,ASP,1,0,09R,51.464900970458984,-0.48677200078964233,75,90,1013,27L,51.46500015258789,-0.4340749979019165,77,270,";

  #[test]
  fn test_parser() {
    let tokens: Vec<&str> = TEST_RUNWAY.split(",").collect();
    let record = StringRecord::from(tokens);
    let runways = parse_runway(&record);
    assert!(runways.is_ok());
    let runways = runways.unwrap();
    assert_eq!(
      runways.0,
      Runway {
        icao: "EGLL".into(),
        length_ft: 12001,
        width_ft: 148,
        surface: "ASP".into(),
        lighted: true,
        closed: false,
        ident: "09R".into(),
        latitude: 51.464900970458984,
        longitude: -0.48677200078964233,
        elevation_ft: 75,
        heading: 90,
        active_to: false,
        active_lnd: false
      }
    );
    assert_eq!(
      runways.1,
      Runway {
        icao: "EGLL".into(),
        length_ft: 12001,
        width_ft: 148,
        surface: "ASP".into(),
        lighted: true,
        closed: false,
        ident: "27L".into(),
        latitude: 51.46500015258789,
        longitude: -0.4340749979019165,
        elevation_ft: 77,
        heading: 270,
        active_to: false,
        active_lnd: false
      }
    );
  }
}
