use super::types::GeonamesCountry;
use crate::{
  config::Config,
  fixed::{
    cached_loader,
    types::{GeonamesShape, GeonamesShapeSet},
  },
  types::Point,
  util::seconds_since,
};
use chrono::Utc;
use csv::StringRecord;
use geo::Contains;
use geojson::{FeatureCollection, GeoJson};
use log::info;
use rstar::{RTree, AABB};
use std::{collections::HashMap, fs::File, io::Read};
use zip::ZipArchive;

#[derive(Debug)]
pub struct Geonames {
  countries: HashMap<String, GeonamesCountry>,
  countries2d: RTree<GeonamesShape>,
}

impl Geonames {
  pub fn empty() -> Self {
    Self {
      countries: HashMap::new(),
      countries2d: RTree::new(),
    }
  }

  pub fn fill(&mut self, other: Self) -> Self {
    Self {
      countries: other.countries,
      countries2d: other.countries2d,
    }
  }

  pub async fn load(cfg: &Config) -> Result<Self, Box<dyn std::error::Error>> {
    let countries = load_countries(cfg).await?;
    let geonames_shapes = load_shapes(cfg).await?;
    let countries2d = RTree::bulk_load(geonames_shapes);

    Ok(Self {
      countries,
      countries2d,
    })
  }

  pub fn get_country_by_position(&self, position: Point) -> Option<GeonamesCountry> {
    let pcoord: geo_types::Point = position.into();
    let envelope = AABB::from_point(pcoord);
    let mut res = self.countries2d.locate_in_envelope_intersecting(&envelope);
    let geo_id = res
      .find(|gs| gs.poly.contains(&pcoord))
      .map(|gs| &gs.ref_id);
    if let Some(geo_id) = geo_id {
      self.countries.get(geo_id).cloned()
    } else {
      None
    }
  }

  pub fn get_country_by_id(&self, id: &str) -> Option<GeonamesCountry> {
    self.countries.get(id).cloned()
  }
}

fn parse_countries(
  file: File,
) -> Result<HashMap<String, GeonamesCountry>, Box<dyn std::error::Error>> {
  let mut rdr = csv::ReaderBuilder::new()
    .has_headers(false)
    .delimiter(b'\t')
    .flexible(true)
    .comment(Some(b'#'))
    .from_reader(file);
  let headers = StringRecord::from(vec![
    "iso",
    "iso3",
    "iso_numeric",
    "fips",
    "name",
    "capital",
    "area",
    "population",
    "continent",
    "tld",
    "currency_code",
    "currency_name",
    "phone",
    "postal_code_format",
    "postal_code_regex",
    "languages",
    "geoname_id",
    "neighbours",
    "equivalent_fips_code",
  ]);
  rdr.set_headers(headers);
  let mut countries = HashMap::new();

  for res in rdr.deserialize() {
    if let Err(err) = res {
      println!("{err}, {:?}", err.position());
    } else {
      let country: GeonamesCountry = res.unwrap();
      countries.insert(country.geoname_id.clone(), country);
    }
  }
  Ok(countries)
}

async fn load_countries(
  cfg: &Config,
) -> Result<HashMap<String, GeonamesCountry>, Box<dyn std::error::Error>> {
  let cache_file = cached_loader(
    &cfg.fixed.geonames_countries_url,
    &cfg.cache.geonames_countries,
  )
  .await?;

  let t = Utc::now();
  let countries = parse_countries(cache_file)?;
  info!("geonames countries parsed in {}s", seconds_since(t));
  Ok(countries)
}

async fn load_shapes(cfg: &Config) -> Result<Vec<GeonamesShape>, Box<dyn std::error::Error>> {
  let cache_file =
    cached_loader(&cfg.fixed.geonames_shapes_url, &cfg.cache.geonames_shapes).await?;
  let t = Utc::now();
  let mut z = ZipArchive::new(cache_file)?;
  let mut raw_data = String::new();

  let mut file = z.by_name("shapes_simplified_low.json")?;
  file.read_to_string(&mut raw_data)?;

  let geodata = raw_data.parse::<GeoJson>()?;
  info!("geonames geojson parsed in {}s", seconds_since(t));

  let mut shapes = vec![];
  let fc = FeatureCollection::try_from(geodata)?;
  for feature in fc {
    let gss: GeonamesShapeSet = feature.try_into()?;
    match gss {
      GeonamesShapeSet::Single(gs) => shapes.push(gs),
      GeonamesShapeSet::Multi(gsv) => shapes.extend(gsv),
    }
  }
  Ok(shapes)
}
