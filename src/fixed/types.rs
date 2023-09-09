use super::{errors::GeonamesParseError, ourairports::Runway};
use crate::{
  atis::runways::{detect_arrivals, detect_departures, normalize_atis_text},
  moving::controller::{Controller, ControllerSet},
  service::camden,
  types::Point,
  weather::WeatherInfo,
};
use geo_types::Polygon;
use geo_types::{geometry::Coord, LineString};
use geojson::{Feature, Value};
use rstar::{RTreeObject, AABB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Country {
  pub name: String,
  pub prefix: String,
  pub control_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Airport {
  pub icao: String,
  pub iata: String,
  pub name: String,
  pub position: Point,
  pub fir_id: String,
  pub is_pseudo: bool,
  pub controllers: ControllerSet,
  pub runways: HashMap<String, Runway>,
  #[serde(skip_serializing)]
  pub country: Option<GeonamesCountry>,
  pub wx: Option<WeatherInfo>,
}

impl Airport {
  pub fn compound_id(&self) -> String {
    format!("{}:{}", self.icao, self.iata)
  }

  pub fn reset_active_runways(&mut self) {
    for (_, rwy) in self.runways.iter_mut() {
      rwy.active_lnd = false;
      rwy.active_to = false;
    }
  }

  pub fn set_active_runways(&mut self) {
    self.reset_active_runways();
    if let Some(atis) = &self.controllers.atis {
      let norm_atis = normalize_atis_text(&atis.text_atis, true);
      let arrivals = detect_arrivals(&norm_atis);
      let departures = detect_departures(&norm_atis);
      for ident in arrivals.iter() {
        let rwy = self.runways.get_mut(ident);
        if let Some(rwy) = rwy {
          rwy.active_lnd = true
        }
      }
      for ident in departures.iter() {
        let rwy = self.runways.get_mut(ident);
        if let Some(rwy) = rwy {
          rwy.active_to = true
        }
      }
    }
  }
}

impl From<Airport> for camden::Airport {
  fn from(value: Airport) -> Self {
    Self {
      icao: value.icao,
      iata: value.iata,
      name: value.name,
      position: Some(value.position.into()),
      fir_id: value.fir_id,
      is_pseudo: value.is_pseudo,
      runways: value
        .runways
        .into_iter()
        .map(|(k, v)| (k, v.into()))
        .collect(),
      wx: value.wx.map(|v| v.into()),
      controllers: Some(value.controllers.into()),
    }
  }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FIR {
  pub icao: String,
  pub name: String,
  pub prefix: String,
  pub boundaries: Boundaries,
  pub controllers: HashMap<String, Controller>,
  #[serde(skip_serializing)]
  pub country: Option<GeonamesCountry>,
}

impl FIR {
  pub fn is_empty(&self) -> bool {
    self.controllers.len() == 0
  }
}

impl From<FIR> for camden::Fir {
  fn from(value: FIR) -> Self {
    Self {
      icao: value.icao,
      name: value.name,
      prefix: value.prefix,
      controllers: value
        .controllers
        .into_iter()
        .map(|(k, v)| (k, v.into()))
        .collect(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct UIR {
  pub icao: String,
  pub name: String,
  pub fir_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Boundaries {
  pub id: String,
  pub region: String,
  pub division: String,
  pub is_oceanic: bool,
  pub min: Point,
  pub max: Point,
  pub center: Point,
  pub points: Vec<Vec<Point>>,
}

impl PartialEq for Boundaries {
  // simplify partial eq as boundaries don't change within a single app run
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct GeonamesCountry {
  pub iso: String,
  pub iso3: String,
  pub iso_numeric: String,
  pub fips: String,
  pub name: String,
  pub capital: String,
  pub area: f64,
  pub population: u64,
  pub continent: String,
  pub tld: String,
  pub currency_code: String,
  pub currency_name: String,
  pub phone: String,
  pub postal_code_format: String,
  pub postal_code_regex: String,
  pub languages: String,
  pub geoname_id: String,
  pub neighbours: String,
  pub equivalent_fips_code: String,
}

// TODO: it's time to consider a universal rtree-insertable type
#[derive(Debug, Clone)]
pub struct GeonamesShape {
  pub poly: Polygon,
  pub ref_id: String,
}

impl RTreeObject for GeonamesShape {
  type Envelope = AABB<geo_types::Point<f64>>;

  fn envelope(&self) -> Self::Envelope {
    self.poly.envelope()
  }
}

impl GeonamesShape {
  pub fn from_vec(ref_id: impl Into<String>, rings: Vec<Vec<Vec<f64>>>) -> Self {
    let mut rings: Vec<Vec<Coord>> = rings
      .into_iter()
      .map(|sp| {
        sp.into_iter()
          .map(|ssp| Coord {
            x: ssp[0],
            y: ssp[1],
          })
          .collect()
      })
      .collect();
    let exterior = LineString::from(rings.swap_remove(0));
    let interior: Vec<LineString> = rings.into_iter().map(LineString::from).collect();
    let poly = Polygon::new(exterior, interior);
    Self {
      poly,
      ref_id: ref_id.into(),
    }
  }
}

#[derive(Debug)]
pub enum GeonamesShapeSet {
  Single(GeonamesShape),
  Multi(Vec<GeonamesShape>),
}

impl TryFrom<Feature> for GeonamesShapeSet {
  type Error = GeonamesParseError;

  fn try_from(value: Feature) -> Result<Self, Self::Error> {
    let geom = value.geometry.as_ref();
    let props = value.properties.as_ref();

    let geoname_id = if let Some(props) = props {
      let prop = props.get("geoNameId");
      if let Some(prop) = prop {
        match prop {
          serde_json::Value::String(s) => s.to_owned(),
          _ => {
            return Err(Self::Error {
              msg: "geoNameId is of incorrect type",
            });
          }
        }
      } else {
        return Err(Self::Error {
          msg: "geoNameId is absent in feature properties",
        });
      }
    } else {
      return Err(Self::Error {
        msg: "no geojson properties defined for feature",
      });
    };

    if let Some(geom) = geom {
      match geom.value.clone() {
        Value::MultiPolygon(mp) => {
          let mut gss = vec![];
          for poly in mp.into_iter() {
            let gs = GeonamesShape::from_vec(geoname_id.to_owned(), poly);
            gss.push(gs);
          }
          Ok(Self::Multi(gss))
        }
        Value::Polygon(poly) => {
          let gs = GeonamesShape::from_vec(geoname_id, poly);
          Ok(Self::Single(gs))
        }
        _ => unimplemented!(),
      }
    } else {
      Err(Self::Error {
        msg: "feature geometry is absent",
      })
    }
  }
}
