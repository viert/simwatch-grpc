mod ext_types;

use std::{
  collections::HashMap,
  sync::atomic::{AtomicUsize, Ordering},
};

use self::ext_types::{Metar, WindDirection};
use crate::service::camden;
use chrono::{DateTime, Duration, Utc};
use log::{debug, error, info};
use reqwest::Client;
use serde::Serialize;
use tokio::{
  join,
  sync::RwLock,
  time::{sleep, Duration as TDuration},
};

const BASE_API: &str = "https://aviationweather.gov/cgi-bin/data";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeatherInfo {
  pub temperature: Option<f64>,
  pub dew_point: Option<f64>,
  pub wind_speed: Option<u64>,
  pub wind_gust: Option<u64>,
  pub wind_direction: Option<WindDirection>,
  pub raw: String,
  pub ts: DateTime<Utc>,
}

impl From<Metar> for WeatherInfo {
  fn from(value: Metar) -> Self {
    Self {
      temperature: value.temp,
      dew_point: value.dewp,
      wind_speed: value.wspd,
      wind_gust: value.wgst,
      wind_direction: value.wdir,
      raw: value.raw_ob,
      ts: value.receipt_time,
    }
  }
}

impl From<WeatherInfo> for camden::WeatherInfo {
  fn from(value: WeatherInfo) -> Self {
    Self {
      temperature: value.temperature,
      dew_point: value.dew_point,
      wind_speed: value.wind_speed,
      wind_gust: value.wind_gust,
      raw: value.raw,
      ts: value.ts.timestamp_millis() as u64,
      wind_direction: value.wind_direction.map(|v| v.into()),
    }
  }
}

#[derive(Debug)]
struct BlackListItem {
  set_at: DateTime<Utc>,
  duration: Duration,
}

impl BlackListItem {
  pub fn new() -> Self {
    Self {
      set_at: Utc::now(),
      duration: Duration::seconds(3600),
    }
  }

  pub fn double(&self) -> Self {
    Self {
      set_at: Utc::now(),
      duration: self.duration * 2,
    }
  }

  pub fn expired(&self) -> bool {
    let now = Utc::now();
    now > self.set_at + self.duration
  }
}

#[derive(Debug)]
pub struct WeatherManager {
  metar_ttl: Duration,
  cache: RwLock<HashMap<String, WeatherInfo>>,
  blacklist: RwLock<HashMap<String, BlackListItem>>,
  apireq_num: AtomicUsize,
}

impl WeatherManager {
  pub fn new(metar_ttl: Duration) -> Self {
    Self {
      metar_ttl,
      cache: Default::default(),
      blacklist: Default::default(),
      apireq_num: AtomicUsize::new(0),
    }
  }

  pub fn request_num(&self) -> usize {
    self.apireq_num.load(Ordering::Relaxed)
  }

  async fn has_valid_cache_for(&self, location: &str) -> bool {
    let cache = self.cache.read().await;
    let value = cache.get(location);
    if let Some(value) = value {
      let now = Utc::now();
      let delta = now - value.ts;
      delta < self.metar_ttl
    } else {
      false
    }
  }

  pub async fn run(&self) {
    let sleep_time = TDuration::from_secs(300);
    info!("starting weather update loop");
    loop {
      let expired = {
        let cache = self.cache.read().await;
        let mut expired = vec![];
        let now = Utc::now();
        for (key, wx) in cache.iter() {
          let delta = now - wx.ts;
          if delta >= self.metar_ttl {
            expired.push(key.clone());
          }
        }
        expired
      };

      if !expired.is_empty() {
        debug!(
          "autoupdate loop: {} locations have expired, renewing",
          expired.len()
        );
        let locations = expired.iter().map(|s| s.as_str()).collect();
        self.preload(locations).await;
      }

      sleep(sleep_time).await;
    }
  }

  async fn is_blacklisted(&self, location: &str) -> bool {
    let blacklist = self.blacklist.read().await;
    let blitem = blacklist.get(location);
    match blitem {
      Some(blitem) => !blitem.expired(),
      None => false,
    }
  }

  fn inc_apireq(&self) {
    self.apireq_num.fetch_add(1, Ordering::Acquire);
  }

  pub async fn preload(&self, locations: Vec<&str>) {
    let locations = {
      let mut results = vec![];
      for location in locations {
        let blacklisted = self.is_blacklisted(location);
        let has_valid_cache = self.has_valid_cache_for(location);
        let (blacklisted, has_valid_cache) = join!(blacklisted, has_valid_cache);
        if !blacklisted && !has_valid_cache {
          results.push(location);
        }
      }
      results
    };

    if locations.is_empty() {
      return;
    }

    let locations = locations.join(",");
    info!("preloading weather for {locations}");

    let path = format!("{BASE_API}/metar.php?ids={locations}&format=json");
    let client = Client::new();

    self.inc_apireq();
    let res = client.get(path).send().await;

    if let Err(err) = res {
      error!("error loading wx data: {err}");
      return;
    }

    let res = res.unwrap().json::<Vec<Metar>>().await;
    if let Err(err) = res {
      error!("error parsing wx data: {err}");
      return;
    }

    let metars = res.unwrap();
    let mut cache = self.cache.write().await;
    for metar in metars {
      let loc = metar.icao_id.clone();
      cache.insert(loc, metar.into());
    }
  }

  async fn get_cache(&self, location: &str) -> Option<WeatherInfo> {
    debug!("collecting weather for {location} from cache");
    let value = {
      let cache = self.cache.read().await;
      cache.get(location).cloned()?
    };
    let now = Utc::now();
    let delta = now - value.ts;
    if delta > self.metar_ttl {
      None
    } else {
      Some(value)
    }
  }

  async fn get_remote(&self, location: &str) -> Option<WeatherInfo> {
    let is_blacklisted = self.is_blacklisted(location).await;
    if is_blacklisted {
      debug!("location {location} is blacklisted");
      return None;
    }

    info!("collecting weather for {location} from remote api");

    let path = format!("{BASE_API}/metar.php?ids={location}&format=json");
    let client = Client::new();

    self.inc_apireq();
    let res = client.get(path).send().await;

    if let Err(err) = res {
      error!("error loading {location} wx data: {err}");
      return None;
    }

    let metar = res.unwrap().json::<Vec<Metar>>().await;
    if let Err(err) = metar {
      error!("error parsing {location} wx data: {err}");
      return None;
    }

    let metar = metar.unwrap().get(0).cloned();
    if let Some(metar) = metar {
      Some(metar.into())
    } else {
      error!("got empty array of wx data at {location}");
      let mut blacklist = self.blacklist.write().await;

      let blitem = blacklist.get(location);
      let blitem = match blitem {
        Some(blitem) => blitem.double(),
        None => BlackListItem::new(),
      };
      debug!("blacklisting {location} for {}", blitem.duration);
      blacklist.insert(location.to_owned(), blitem);
      None
    }
  }

  pub async fn get(&self, location: &str) -> Option<WeatherInfo> {
    let wx = self.get_cache(location).await;
    if let Some(wx) = wx {
      Some(wx)
    } else {
      let wx = self.get_remote(location).await;
      if let Some(wx) = wx {
        let mut cache = self.cache.write().await;
        cache.insert(location.to_owned(), wx.clone());
        Some(wx)
      } else {
        None
      }
    }
  }
}
