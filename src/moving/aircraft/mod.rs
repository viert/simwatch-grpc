mod data;

use lazy_static::lazy_static;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum EngineType {
  Electric,
  Jet,
  Piston,
  Rocket,
  Turboprop,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum AircraftType {
  Amphibian,
  Gyrocopter,
  Helicopter,
  LandPlane,
  SeaPlane,
  Tiltrotor,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Aircraft {
  pub name: &'static str,
  pub description: &'static str,
  pub wtc: &'static str,
  pub wtg: &'static str,
  pub designator: &'static str,
  pub manufacturer_code: &'static str,
  pub aircraft_type: AircraftType,
  pub engine_count: u8,
  pub engine_type: EngineType,
}

lazy_static! {
  static ref DB: HashMap<&'static str, Vec<&'static Aircraft>> = {
    let mut db: HashMap<&'static str, Vec<&'static Aircraft>> = HashMap::new();
    for atype in data::MODELS {
      let ex = db.get_mut(atype.designator);
      if let Some(ex) = ex {
        ex.push(atype);
      } else {
        db.insert(atype.designator, vec![atype]);
      }
    }
    db
  };
}

pub fn guess_aircraft_types(code: &str) -> Option<Vec<&'static Aircraft>> {
  // pff unicode is tough
  let mut indices: Vec<usize> = code.char_indices().map(|(i, _)| i).collect();
  indices.push(code.len());
  let mut l = (indices.len() - 1).clamp(0, 5);

  while l > 0 {
    let idx = indices[l];
    let partial_code = &code[..idx];
    let atypes = DB.get(partial_code);
    if atypes.is_some() {
      return atypes.cloned();
    }
    l -= 1;
  }
  None
}
