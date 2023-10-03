mod data;

use lazy_static::lazy_static;
use serde::Serialize;
use std::collections::HashMap;

use crate::service::camden;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum EngineType {
  Electric,
  Jet,
  Piston,
  Rocket,
  Turboprop,
}

impl From<&EngineType> for camden::EngineType {
  fn from(value: &EngineType) -> Self {
    match value {
      EngineType::Electric => Self::EtElectric,
      EngineType::Jet => Self::EtJet,
      EngineType::Piston => Self::EtPiston,
      EngineType::Rocket => Self::EtRocket,
      EngineType::Turboprop => Self::EtTurboprop,
    }
  }
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

impl From<&AircraftType> for camden::AircraftType {
  fn from(value: &AircraftType) -> Self {
    match value {
      AircraftType::Amphibian => Self::AtAmphibian,
      AircraftType::Gyrocopter => Self::AtGyrocopter,
      AircraftType::Helicopter => Self::AtHelicopter,
      AircraftType::LandPlane => Self::AtLandplane,
      AircraftType::SeaPlane => Self::AtSeaplane,
      AircraftType::Tiltrotor => Self::AtTiltrotor,
    }
  }
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

impl From<&Aircraft> for camden::Aircraft {
  fn from(value: &Aircraft) -> Self {
    let aircraft_type: camden::AircraftType = (&value.aircraft_type).into();
    let engine_type: camden::EngineType = (&value.engine_type).into();
    Self {
      name: value.name.into(),
      description: value.description.into(),
      wtc: value.wtc.into(),
      wtg: value.wtg.into(),
      designator: value.designator.into(),
      manufacturer_code: value.manufacturer_code.into(),
      aircraft_type: aircraft_type as i32,
      engine_count: value.engine_count as u32,
      engine_type: engine_type as i32,
    }
  }
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

pub fn guess_aircraft_types(code: &str) -> Option<&'static Aircraft> {
  // pff unicode is tough
  let mut indices: Vec<usize> = code.char_indices().map(|(i, _)| i).collect();
  indices.push(code.len());
  let mut l = (indices.len() - 1).clamp(0, 5);

  while l > 0 {
    let idx = indices[l];
    let partial_code = &code[..idx];
    let atypes = DB.get(partial_code);
    if let Some(atypes) = atypes {
      if !atypes.is_empty() {
        return atypes.get(0).map(|at| *at);
      }
    }
    l -= 1;
  }
  None
}
