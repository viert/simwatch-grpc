use super::{
  boundaries::load_boundaries,
  data::FixedData,
  geonames::Geonames,
  ourairports::{load_runways, Runway},
  types::{Airport, Boundaries, Country, FIR, UIR},
};
use crate::{config::Config, moving::controller::ControllerSet, types::Point};
use log::error;
use std::{collections::HashMap, error::Error, fmt::Display};

enum ParserState {
  Idle,
  ReadCountries,
  ReadAirports,
  ReadFIRs,
  ReadUIRs,
}

#[derive(Debug)]
pub struct ParseError {
  msg: String,
}

impl Display for ParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "parse error: {}", self.msg)
  }
}
impl Error for ParseError {}

fn parse(
  src: &str,
  bdrs: HashMap<String, Boundaries>,
  mut runway_map: HashMap<String, Vec<Runway>>,
  geonames: Geonames,
) -> Result<FixedData, ParseError> {
  let mut state = ParserState::Idle;
  let mut countries = vec![];
  let mut airports = vec![];
  let mut firs = vec![];
  let mut uirs = vec![];

  for line in src.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with(';') {
      continue;
    }

    if line.starts_with('[') {
      let section = &line[1..line.len() - 1];
      match section {
        "Countries" => state = ParserState::ReadCountries,
        "Airports" => state = ParserState::ReadAirports,
        "FIRs" => state = ParserState::ReadFIRs,
        "UIRs" => state = ParserState::ReadUIRs,
        "IDL" => break,
        _ => continue,
      }
    } else {
      match state {
        ParserState::Idle => error!("unexpected line \"{}\" while parser is idle", line),
        ParserState::ReadCountries => {
          let tokens: Vec<&str> = line.split('|').collect();
          if tokens.len() != 3 {
            error!("invalid country line \"{}\"", line)
          } else {
            let c = Country {
              name: tokens[0].into(),
              prefix: tokens[1].into(),
              control_name: if tokens[2].is_empty() {
                None
              } else {
                Some(tokens[2].into())
              },
            };
            countries.push(c);
          }
        }
        ParserState::ReadAirports => {
          let tokens: Vec<&str> = line.split('|').collect();
          if tokens.len() != 7 {
            error!("invalid airport line \"{}\"", line)
          } else {
            let lat = tokens[2].parse::<f64>();
            if lat.is_err() {
              error!(
                "can't parse latitude \"{}\" for airport {}",
                tokens[2], tokens[0]
              );
              continue;
            }
            let lng = tokens[3].parse::<f64>();
            if lng.is_err() {
              error!(
                "can't parse longitude \"{}\" for airport {}",
                tokens[3], tokens[0]
              );
              continue;
            }

            let icao = tokens[0].into();
            let rwys = runway_map.remove(&icao);
            let mut runways = HashMap::new();
            if let Some(rwys) = rwys {
              for rwy in rwys.into_iter() {
                runways.insert(rwy.ident.clone(), rwy);
              }
            }

            let position = Point {
              lat: lat.unwrap(),
              lng: lng.unwrap(),
            };
            let country = geonames.get_country_by_position(position);

            let a = Airport {
              icao,
              iata: tokens[4].into(),
              name: tokens[1].into(),
              position,
              fir_id: tokens[5].into(),
              is_pseudo: tokens[6] == "1",
              controllers: ControllerSet::empty(),
              runways,
              country,
              wx: None,
            };

            airports.push(a);
          }
        }
        ParserState::ReadFIRs => {
          let tokens: Vec<&str> = line.split('|').collect();
          if tokens.len() != 4 {
            error!("invalid fir line \"{}\"", line)
          } else {
            let mut b_id = tokens[3];

            // Ugly hack for FIRs not having boundaries region id
            // We have to use FIR ICAO instead as the corresponding boundaries exist
            // Ex: ZGHA and other Chinese FIRs as of Nov 10th 2022
            if b_id.is_empty() {
              b_id = tokens[0];
            }

            let boundaries = bdrs.get(b_id);
            if let Some(boundaries) = boundaries {
              let country = geonames.get_country_by_position(boundaries.center);
              let fir = FIR {
                icao: tokens[0].into(),
                name: tokens[1].into(),
                prefix: tokens[2].into(),
                boundaries: boundaries.clone(),
                controllers: HashMap::new(),
                country,
              };
              firs.push(fir);
            } else {
              error!(
                "can't find boundaries \"{}\" for FIR \"{}\"",
                tokens[3], tokens[0]
              );
            }
          }
        }
        ParserState::ReadUIRs => {
          let tokens: Vec<&str> = line.split('|').collect();
          if tokens.len() != 3 {
            error!("invalid uir line \"{}\"", line)
          } else {
            let fir_ids = tokens[2].split(',').map(|t| t.into()).collect();
            let uir = UIR {
              icao: tokens[0].into(),
              name: tokens[1].into(),
              fir_ids,
            };
            uirs.push(uir);
          }
        }
      }
    }
  }

  Ok(FixedData::new(countries, airports, firs, uirs, geonames))
}

pub async fn load_fixed(cfg: &Config) -> Result<FixedData, Box<dyn Error>> {
  let boundaries = load_boundaries(&cfg.fixed.boundaries_url).await?;
  let text = reqwest::get(&cfg.fixed.data_url).await?.text().await?;
  let runways = load_runways(cfg).await?;
  let geonames = Geonames::load(cfg).await?;
  let data = parse(&text, boundaries, runways, geonames)?;
  Ok(data)
}
