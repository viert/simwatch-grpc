use super::{
  geonames::Geonames,
  types::{Airport, Country, GeonamesCountry, FIR, UIR},
};
use crate::{
  moving::controller::{Controller, Facility},
  types::Point,
  weather::WeatherInfo,
};
use log::error;
use std::collections::HashMap;

#[derive(Debug)]
pub struct FixedData {
  countries: Vec<Country>,
  airports: Vec<Airport>,
  firs: Vec<FIR>,
  uirs: Vec<UIR>,
  arpt_icao_idx: HashMap<String, Vec<usize>>,
  arpt_iata_idx: HashMap<String, usize>,
  arpt_compound_idx: HashMap<String, usize>,
  country_idx: HashMap<String, usize>,
  firs_icao_idx: HashMap<String, usize>,
  firs_prefix_idx: HashMap<String, usize>,
  uirs_idx: HashMap<String, usize>,
  geonames: Geonames,
}

impl FixedData {
  pub fn empty() -> Self {
    Self {
      countries: vec![],
      airports: vec![],
      firs: vec![],
      uirs: vec![],
      arpt_icao_idx: HashMap::new(),
      arpt_iata_idx: HashMap::new(),
      arpt_compound_idx: HashMap::new(),
      country_idx: HashMap::new(),
      firs_icao_idx: HashMap::new(),
      firs_prefix_idx: HashMap::new(),
      uirs_idx: HashMap::new(),
      geonames: Geonames::empty(),
    }
  }

  pub fn fill(&mut self, other: FixedData) {
    self.countries = other.countries;
    self.airports = other.airports;
    self.firs = other.firs;
    self.uirs = other.uirs;
    self.arpt_icao_idx = other.arpt_icao_idx;
    self.arpt_iata_idx = other.arpt_iata_idx;
    self.arpt_compound_idx = other.arpt_compound_idx;
    self.country_idx = other.country_idx;
    self.firs_icao_idx = other.firs_icao_idx;
    self.firs_prefix_idx = other.firs_prefix_idx;
    self.uirs_idx = other.uirs_idx;
    self.geonames = other.geonames;
  }

  pub fn new(
    countries: Vec<Country>,
    airports: Vec<Airport>,
    firs: Vec<FIR>,
    uirs: Vec<UIR>,
    geonames: Geonames,
  ) -> Self {
    let mut arpt_icao_idx: HashMap<String, Vec<usize>> = HashMap::new();
    let mut arpt_iata_idx: HashMap<String, usize> = HashMap::new();
    let mut arpt_compound_idx: HashMap<String, usize> = HashMap::new();
    for (idx, arpt) in airports.iter().enumerate() {
      if !arpt.icao.is_empty() {
        let values = arpt_icao_idx.get_mut(&arpt.icao);
        if let Some(values) = values {
          values.push(idx);
        } else {
          let values = vec![idx];
          arpt_icao_idx.insert(arpt.icao.clone(), values);
        }
      }
      if !arpt.iata.is_empty() {
        arpt_iata_idx.insert(arpt.iata.clone(), idx);
      }
      arpt_compound_idx.insert(arpt.compound_id(), idx);
    }

    let mut country_idx = HashMap::new();
    for (idx, country) in countries.iter().enumerate() {
      country_idx.insert(country.prefix.clone(), idx);
    }

    let mut firs_icao_idx = HashMap::new();
    let mut firs_prefix_idx = HashMap::new();
    for (idx, fir) in firs.iter().enumerate() {
      firs_icao_idx.insert(fir.icao.clone(), idx);
      firs_prefix_idx.insert(fir.prefix.clone(), idx);
    }

    let mut uirs_idx = HashMap::new();
    for (idx, uir) in uirs.iter().enumerate() {
      uirs_idx.insert(uir.icao.clone(), idx);
    }

    Self {
      countries,
      airports,
      firs,
      uirs,
      arpt_icao_idx,
      arpt_iata_idx,
      arpt_compound_idx,
      country_idx,
      firs_icao_idx,
      firs_prefix_idx,
      uirs_idx,
      geonames,
    }
  }

  pub fn airports(&self) -> &Vec<Airport> {
    &self.airports
  }

  pub fn firs(&self) -> &Vec<FIR> {
    &self.firs
  }

  pub fn set_airport_weather(&mut self, icao: &str, wx: WeatherInfo) {
    let idx = self.find_airport_idx(icao);
    if let Some(idx) = idx {
      let arpt = self.airports.get_mut(idx);
      if let Some(arpt) = arpt {
        arpt.wx = Some(wx);
      }
    }
  }

  pub fn set_airport_controller(&mut self, ctrl: Controller) -> Option<&Airport> {
    let mut ctrl = ctrl;
    let tokens: Vec<&str> = ctrl.callsign.split('_').collect();
    let code = tokens[0];
    let idx = self.find_airport_idx(code);
    if let Some(idx) = idx {
      let arpt = self.airports.get_mut(idx);
      if let Some(arpt) = arpt {
        ctrl.human_readable = match &ctrl.facility {
          Facility::ATIS => Some(format!("{} ATIS", arpt.name)),
          Facility::Delivery => Some(format!("{} Delivery", arpt.name)),
          Facility::Ground => Some(format!("{} Ground", arpt.name)),
          Facility::Tower => Some(format!("{} Tower", arpt.name)),
          Facility::Approach => Some(format!("{} Approach", arpt.name)),
          _ => unreachable!(),
        };
        match &ctrl.facility {
          Facility::ATIS => {
            arpt.controllers.atis = Some(ctrl);
            arpt.set_active_runways();
          }
          Facility::Delivery => arpt.controllers.delivery = Some(ctrl),
          Facility::Ground => arpt.controllers.ground = Some(ctrl),
          Facility::Tower => arpt.controllers.tower = Some(ctrl),
          Facility::Approach => arpt.controllers.approach = Some(ctrl),
          _ => unreachable!(),
        }
        return Some(arpt);
      } else {
        error!(
          "can't find airport for controller {} by index {}, this is deffy a bug",
          ctrl.callsign, idx
        );
      }
    } else {
      error!("can't find airport for controller {}", ctrl.callsign);
    }
    None
  }

  pub fn reset_airport_controller(&mut self, ctrl: &Controller) {
    let tokens: Vec<&str> = ctrl.callsign.split('_').collect();
    let code = tokens[0];
    let idx = self.find_airport_idx(code);
    if let Some(idx) = idx {
      let arpt = self.airports.get_mut(idx);
      if let Some(arpt) = arpt {
        match &ctrl.facility {
          Facility::ATIS => {
            arpt.controllers.atis = None;
            arpt.reset_active_runways();
          }
          Facility::Delivery => arpt.controllers.delivery = None,
          Facility::Ground => arpt.controllers.ground = None,
          Facility::Tower => arpt.controllers.tower = None,
          Facility::Approach => arpt.controllers.approach = None,
          _ => unreachable!(),
        }
      } else {
        error!(
          "can't find airport for controller {} by index {}, this is deffy a bug",
          ctrl.callsign, idx
        );
      }
    } else {
      error!("can't find airport for controller {}", ctrl.callsign);
    }
  }

  pub fn set_fir_controller(&mut self, ctrl: Controller) -> Option<FIR> {
    let tokens: Vec<&str> = ctrl.callsign.split('_').collect();
    let code = tokens[0];
    let country = self
      .country_idx
      .get(&code[..2])
      .map(|idx| self.countries.get(*idx).unwrap());

    let fir_ids = self.find_fir_indices(code);
    let mut fir_found = None;
    for idx in fir_ids {
      let fir = self.firs.get_mut(idx);
      if let Some(fir) = fir {
        // region:set_human_readable
        let ctrl = if let Some(country) = country {
          let mut ctrl = ctrl.clone();
          if let Some(cn) = &country.control_name {
            ctrl.human_readable = Some(format!("{} {}", fir.name, cn));
          } else {
            ctrl.human_readable = Some(fir.name.clone())
          }
          ctrl
        } else {
          ctrl.clone()
        };
        // endregion:set_human_readable
        fir.controllers.insert(ctrl.callsign.clone(), ctrl);
        fir_found = Some(fir.clone());
      }
    }
    fir_found
  }

  pub fn reset_fir_controller(&mut self, ctrl: &Controller) {
    let tokens: Vec<&str> = ctrl.callsign.split('_').collect();
    let code = tokens[0];
    let fir_ids = self.find_fir_indices(code);
    for idx in fir_ids {
      let fir = self.firs.get_mut(idx);
      if let Some(fir) = fir {
        fir.controllers.remove(&ctrl.callsign);
      }
    }
  }

  fn find_fir_idx_by_icao(&self, query: &str) -> Option<usize> {
    self.firs_icao_idx.get(query).copied()
  }

  fn find_fir_idx_by_prefix(&self, query: &str) -> Option<usize> {
    self.firs_prefix_idx.get(query).copied()
  }

  fn find_fir_indices(&self, query: &str) -> Vec<usize> {
    let idx = self
      .find_fir_idx_by_icao(query)
      .or_else(|| self.find_fir_idx_by_prefix(query));
    if let Some(idx) = idx {
      return vec![idx];
    }

    let arpt = self.find_airport(query);
    if let Some(arpt) = arpt {
      if !arpt.fir_id.is_empty() {
        let idx = self
          .find_fir_idx_by_icao(&arpt.fir_id)
          .or_else(|| self.find_fir_idx_by_prefix(&arpt.fir_id));
        if let Some(idx) = idx {
          return vec![idx];
        }
      }
    }

    let uir = self.uirs_idx.get(query).map(|idx| &self.uirs[*idx]);

    if let Some(uir) = uir {
      let mut idcs = vec![];
      for fir_id in uir.fir_ids.iter() {
        let idx = self
          .find_fir_idx_by_icao(fir_id)
          .or_else(|| self.find_fir_idx_by_prefix(fir_id));
        if let Some(idx) = idx {
          idcs.push(idx)
        }
      }
      idcs
    } else {
      vec![]
    }
  }

  pub fn find_firs(&self, query: &str) -> Vec<FIR> {
    self
      .find_fir_indices(query)
      .into_iter()
      .map(|idx| self.firs[idx].clone())
      .collect()
  }

  pub fn find_country(&self, prefix: &str) -> Option<Country> {
    self
      .country_idx
      .get(prefix)
      .map(|idx| self.countries[*idx].clone())
  }

  pub fn find_airport_idx(&self, code: &str) -> Option<usize> {
    let code = if code.len() > 4 { &code[0..4] } else { code };
    let idx = self.arpt_iata_idx.get(code);
    if let Some(idx) = idx {
      Some(*idx)
    } else {
      let indices = self.arpt_icao_idx.get(code);
      indices.map(|indices| indices[0])
    }
  }

  pub fn find_airport(&self, code: &str) -> Option<Airport> {
    let idx = self.find_airport_idx(code)?;
    Some(self.airports[idx].clone())
  }

  pub fn find_airport_compound(&self, code: &str) -> Option<Airport> {
    let idx = self.arpt_compound_idx.get(code)?;
    let arpt = self.airports.get(*idx)?;
    Some(arpt.clone())
  }

  pub fn get_geonames_country_by_position(&self, position: Point) -> Option<GeonamesCountry> {
    self.geonames.get_country_by_position(position)
  }

  pub fn get_geonames_country_by_id(&self, id: &str) -> Option<GeonamesCountry> {
    self.geonames.get_country_by_id(id)
  }
}
