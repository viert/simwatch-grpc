use duration_str::deserialize_duration;
use log::LevelFilter;
use serde::Deserialize;
use std::{fs::File, io::Read, path::Path, time::Duration};

#[derive(Deserialize, Debug, Clone)]
pub struct Camden {
  pub map_win_multiplier: f64,
}

impl Default for Camden {
  fn default() -> Self {
    Self {
      map_win_multiplier: 1.3,
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Cache {
  pub runways: String,
  pub geonames_countries: String,
  pub geonames_shapes: String,
}

impl Default for Cache {
  fn default() -> Self {
    Self {
      runways: "/tmp/runways.csv.cache".to_owned(),
      geonames_countries: "/tmp/geonames.countries.csv.cache".to_owned(),
      geonames_shapes: "/tmp/geonames.shapes.json.zip".to_owned(),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Api {
  pub url: String,
  #[serde(deserialize_with = "deserialize_duration")]
  pub poll_period: Duration,
  #[serde(deserialize_with = "deserialize_duration")]
  pub timeout: Duration,
}

impl Default for Api {
  fn default() -> Self {
    Self {
      url: "https://data.vatsim.net/v3/vatsim-data.json".to_owned(),
      poll_period: Duration::from_secs(15),
      timeout: Duration::from_secs(1),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Log {
  pub level: LevelFilter,
}

impl Default for Log {
  fn default() -> Self {
    Self {
      level: LevelFilter::Debug,
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Web {
  pub port: u16,
}

impl Default for Web {
  fn default() -> Self {
    Self { port: 8000 }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Fixed {
  pub data_url: String,
  pub boundaries_url: String,
  pub runways_url: String,
  pub geonames_countries_url: String,
  pub geonames_shapes_url: String,
}

impl Default for Fixed {
  fn default() -> Self {
    Self {
      data_url:
        "https://raw.githubusercontent.com/vatsimnetwork/vatspy-data-project/master/VATSpy.dat"
          .to_owned(),
      boundaries_url: "https://raw.githubusercontent.com/vatsimnetwork/vatspy-data-project/master/Boundaries.geojson".to_owned(),
      runways_url: "https://ourairports.com/data/runways.csv".to_owned(),
      geonames_countries_url: "http://download.geonames.org/export/dump/countryInfo.txt".to_owned(),
      geonames_shapes_url: "http://download.geonames.org/export/dump/shapes_simplified_low.json.zip".to_owned()
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Track {
  pub uri: String,
  pub dbname: String,
}

impl Default for Track {
  fn default() -> Self {
    Self {
      uri: "mongodb://localhost:27017".to_owned(),
      dbname: "camden-dev".to_owned(),
    }
  }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Config {
  pub log: Log,
  pub web: Web,
  pub api: Api,
  pub fixed: Fixed,
  pub track: Track,
  pub cache: Cache,
  pub camden: Camden,
}

pub fn read_config(filename: Option<&str>) -> Config {
  let mut filenames = vec!["./camden.toml", "/etc/camden.toml"];
  if let Some(filename) = filename {
    filenames.insert(0, filename);
  }

  for fname in filenames {
    let path = Path::new(fname);
    println!("Trying config file {}...", fname);
    if path.is_file() {
      let res = File::open(path);
      if let Err(err) = res {
        println!("Error opening config file {}: {}", fname, err);
        continue;
      }
      let mut f = res.unwrap();
      let mut config_raw = String::new();
      let res = f.read_to_string(&mut config_raw);
      if let Err(err) = res {
        println!("Error reading config file {}: {}", fname, err);
        continue;
      }
      let res: Result<Config, toml::de::Error> = toml::from_str(&config_raw);
      if let Err(err) = res {
        println!("Error parsing config file {}: {}", fname, err);
        continue;
      }
      return res.unwrap();
    }
    println!("Config file {} does not exist", fname);
  }
  println!("No config files can be read, using default settings");
  Default::default()
}
