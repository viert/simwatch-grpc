pub mod metrics;
pub mod spatial;

use self::{
  metrics::Metrics,
  spatial::{PointObject, RectObject},
};

use crate::{
  config::Config,
  fixed::{
    data::FixedData,
    parser::load_fixed,
    types::{Airport, FIR},
  },
  labels,
  moving::{
    controller::{Controller, Facility},
    load_vatsim_data,
    pilot::Pilot,
  },
  track::{trackpoint::TrackPoint, Store},
  types::Rect,
  util::{seconds_since, Counter},
  weather::WeatherManager,
};

use chrono::{Duration, Utc};
use log::{debug, error, info};
use rstar::RTree;
use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};
use tokio::{sync::RwLock, time::sleep};

const CLEANUP_EVERY_X_ITER: u8 = 5;

#[derive(Debug)]
pub struct Manager {
  cfg: Config,
  fixed: RwLock<FixedData>,

  pilots: RwLock<HashMap<String, Pilot>>,
  pilots2d: RwLock<RTree<PointObject>>,
  pilots_po: RwLock<HashMap<String, PointObject>>,

  airports2d: RwLock<RTree<PointObject>>,
  firs2d: RwLock<RTree<RectObject>>,
  tracks: RwLock<Store>,

  metrics: RwLock<Metrics>,
}

impl Manager {
  pub async fn new(cfg: Config) -> Self {
    info!("setting vatsim data manager up");

    let tracks = Store::new(&cfg.track.folder);

    info!("cleaning up tracks");
    let t = Utc::now();
    let res = tracks.cleanup();
    if let Err(err) = res {
      error!("error cleaning up: {}", err);
    } else {
      let process_time = seconds_since(t);
      info!("boot-time track store cleanup took {process_time}s");
    }

    Self {
      cfg,
      fixed: RwLock::new(FixedData::empty()),
      pilots: RwLock::new(HashMap::new()),
      pilots2d: RwLock::new(RTree::new()),
      pilots_po: RwLock::new(HashMap::new()),
      airports2d: RwLock::new(RTree::new()),
      firs2d: RwLock::new(RTree::new()),
      tracks: RwLock::new(tracks),
      metrics: RwLock::new(Metrics::new()),
    }
  }

  pub fn config(&self) -> &Config {
    &self.cfg
  }

  pub async fn render_metrics(&self) -> String {
    self.metrics.read().await.render()
  }

  pub async fn get_all_pilots(&self) -> Vec<Pilot> {
    let pilots_idx = self.pilots.read().await;
    pilots_idx.values().cloned().collect()
  }

  pub async fn get_all_airports(&self, show_uncontrolled_wx: bool) -> Vec<Airport> {
    let fixed = self.fixed.read().await;
    fixed
      .airports()
      .iter()
      .filter(|arpt| !arpt.controllers.is_empty() || (show_uncontrolled_wx && arpt.wx.is_some()))
      .cloned()
      .collect()
  }

  pub async fn get_all_firs(&self) -> Vec<FIR> {
    let fixed = self.fixed.read().await;
    fixed
      .firs()
      .iter()
      .filter(|fir| !fir.is_empty())
      .cloned()
      .collect()
  }

  pub async fn get_pilots(&self, rect: &Rect, subscribed_ids: &HashSet<String>) -> Vec<Pilot> {
    let pilots2d = self.pilots2d.read().await;
    let pilots_idx = self.pilots.read().await;
    let mut pilots = vec![];
    let mut subs = subscribed_ids.clone();

    for env in rect.envelopes() {
      for po in pilots2d.locate_in_envelope(&env) {
        let pilot = pilots_idx.get(&po.id);
        if let Some(pilot) = pilot {
          subs.remove(&pilot.callsign);
          pilots.push(pilot.clone());
        }
      }
    }

    for sub in subs.into_iter() {
      let pilot = pilots_idx.get(&sub);
      if let Some(pilot) = pilot {
        pilots.push(pilot.clone());
      }
    }

    pilots
  }

  pub async fn get_airports(&self, rect: &Rect, show_uncontrolled_wx: bool) -> Vec<Airport> {
    let airports2d = self.airports2d.read().await;
    let fixed = self.fixed.read().await;
    let mut airports = vec![];

    for env in rect.envelopes() {
      for po in airports2d.locate_in_envelope(&env) {
        let airport = fixed.find_airport_compound(&po.id);
        if let Some(airport) = airport {
          if !airport.controllers.is_empty() || (show_uncontrolled_wx && airport.wx.is_some()) {
            airports.push(airport)
          }
        }
      }
    }
    airports
  }

  pub async fn get_firs(&self, rect: &Rect) -> Vec<FIR> {
    let firs2d = self.firs2d.read().await;
    let fixed = self.fixed.read().await;
    let mut firs = HashMap::new();

    for env in rect.envelopes() {
      for po in firs2d.locate_in_envelope_intersecting(&env) {
        let fir_list = fixed.find_firs(&po.id);
        for fir in fir_list.into_iter().filter(|f| !f.is_empty()) {
          firs.insert(fir.icao.clone(), fir);
        }
      }
    }
    firs.into_values().collect()
  }

  pub async fn find_airport(&self, code: &str) -> Option<Airport> {
    self.fixed.read().await.find_airport(code)
  }

  async fn setup_fixed_data(&self) -> Result<(), Box<dyn std::error::Error>> {
    info!("loading fixed data");
    let fixed = load_fixed(&self.cfg).await?; // TODO retries
    for arpt in fixed.airports() {
      self.airports2d.write().await.insert(arpt.into());
    }
    for fir in fixed.firs() {
      self.firs2d.write().await.insert(fir.into())
    }
    self.fixed.write().await.fill(fixed);
    info!("fixed data configured");
    Ok(())
  }

  async fn remove_pilot(&self, callsign: &str) -> bool {
    let po = { self.pilots_po.write().await.remove(callsign) };
    if let Some(po) = po {
      self.pilots2d.write().await.remove(&po);
      self.pilots.write().await.remove(callsign);
      true
    } else {
      false
    }
  }

  pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
    self.setup_fixed_data().await?;

    let mut pilots_callsigns = HashSet::new();
    let mut controllers: HashMap<String, Controller> = HashMap::new();
    let mut data_updated_at = 0;
    let mut cleanup = CLEANUP_EVERY_X_ITER;

    // TODO: configurable weather ttl
    let wx_manager = WeatherManager::new(Duration::seconds(1800));
    let wx_manager = Arc::new(wx_manager);
    let wx_move = wx_manager.clone();
    tokio::spawn(async move { wx_move.run().await });

    loop {
      info!("loading vatsim data");
      let t = Utc::now();
      let data = load_vatsim_data(&self.cfg).await;
      let process_time = seconds_since(t);
      self
        .metrics
        .write()
        .await
        .vatsim_data_load_time_sec
        .set_single(process_time);
      info!("vatsim data loaded in {}s", process_time);
      if let Some(data) = data {
        let ts = data.general.updated_at.timestamp();
        if ts > data_updated_at {
          data_updated_at = ts;
          self.metrics.write().await.vatsim_data_timestamp = ts;
          // region:pilots_processing
          let mut fresh_pilots_callsigns = HashSet::new();

          info!("processing pilots");
          let t = Utc::now();
          let pcount = data.pilots.len();

          let mut pilots_grouped = Counter::new();
          {
            for pilot in data.pilots.into_iter() {
              // avoid duplication in rtree
              self.remove_pilot(&pilot.callsign).await;

              // collecting pilots callsigns to find those disappeared since
              // the previous iteration
              fresh_pilots_callsigns.insert(pilot.callsign.clone());

              let po: PointObject = (&pilot).into();

              let mut pilots2d = self.pilots2d.write().await;
              let mut pilots_po = self.pilots_po.write().await;
              let mut pilots = self.pilots.write().await;

              // tracking first, to avoid additional cloning while inserting into hashmap later
              let tracks = self.tracks.write().await;
              let res = tracks.store_track(&pilot);
              if let Err(err) = res {
                error!("error storing pilot track: {}", err);
              }

              let country = self
                .fixed
                .read()
                .await
                .get_geonames_country_by_position(pilot.position);
              if let Some(country) = country {
                pilots_grouped.inc(country.geoname_id);
              }

              // We have to keep point objects in both hashmap and rtree
              // because rtree doesn't support searching by id:
              //
              // We need to search point objects by id for removing a pilot
              // from RTree "by id". We search for a point object in the HashMap
              // then we pass it to .remove() method of the tree where it's
              // being searched by coords and then checked with PartialEq,
              // so it's OK that the HashMap and RTree contain copies of the object.
              // See remove_pilot() method for details
              pilots2d.insert(po.clone());
              pilots_po.insert(pilot.callsign.clone(), po);
              pilots.insert(pilot.callsign.clone(), pilot);
            }
          }

          // for each callsign not met this iteration let's remove it from the indexes
          for cs in pilots_callsigns.difference(&fresh_pilots_callsigns) {
            self.remove_pilot(cs).await;
          }

          // setup this iteration as "previous"
          pilots_callsigns = fresh_pilots_callsigns;

          let process_time = seconds_since(t);
          {
            let mut metrics = self.metrics.write().await;
            metrics
              .processing_time_sec
              .set(labels!("object_type" = "pilot"), process_time);

            let fixed = self.fixed.read().await;
            for (geo_id, count) in pilots_grouped.iter() {
              let country = fixed.get_geonames_country_by_id(geo_id).unwrap();
              metrics.vatsim_objects_online.set(
                labels!(
                  "object_type" = "pilot",
                  "country_code" = &country.iso,
                  "continent_code" = &country.continent
                ),
                *count,
              );
            }
          }
          info!("{} pilots processed in {}s", pcount, process_time);
          // endregion:pilots_processing

          // region:controllers_processing
          info!("processing controllers");
          let t = Utc::now();
          let mut fresh_controllers = HashMap::new();
          let mut ccount = 0;
          let mut ctrl_grouped = Counter::new();
          let mut controlled_arpt = HashSet::new();
          {
            let mut fixed = self.fixed.write().await;

            for ctrl in data.controllers.into_iter() {
              match &ctrl.facility {
                Facility::Reject => {
                  continue;
                }
                Facility::Radar => {
                  fresh_controllers.insert(ctrl.callsign.clone(), ctrl.clone());
                  let fir = fixed.set_fir_controller(ctrl);
                  if let Some(fir) = fir {
                    let country = fir.country.as_ref();
                    if let Some(country) = country {
                      let key = format!("{}:radar", country.geoname_id);
                      ctrl_grouped.inc(key);
                    }
                  }
                }
                _ => {
                  fresh_controllers.insert(ctrl.callsign.clone(), ctrl.clone());
                  let facility = ctrl.facility.clone();
                  let arpt = fixed.set_airport_controller(ctrl);
                  if let Some(arpt) = arpt {
                    controlled_arpt.insert(arpt.icao.clone());
                    let country = arpt.country.as_ref();
                    if let Some(country) = country {
                      let key = format!("{}:{}", country.geoname_id, facility);
                      ctrl_grouped.inc(key);
                    }
                  }
                }
              }
              ccount += 1;
            }

            let locations: Vec<&str> = controlled_arpt.iter().map(|s| s.as_str()).collect();
            wx_manager.preload(locations).await;

            for icao in controlled_arpt.iter() {
              let wx = wx_manager.get(icao).await;
              if let Some(wx) = wx {
                fixed.set_airport_weather(icao, wx);
              }
            }
          }

          for (cs, ctrl) in controllers.iter() {
            if !fresh_controllers.contains_key(cs) {
              match ctrl.facility {
                Facility::Radar => self.fixed.write().await.reset_fir_controller(ctrl),
                _ => {
                  self.fixed.write().await.reset_airport_controller(ctrl);
                }
              }
            }
          }
          controllers = fresh_controllers;

          let process_time = seconds_since(t);
          {
            let mut metrics = self.metrics.write().await;
            metrics
              .processing_time_sec
              .set(labels!("object_type" = "controller"), process_time);

            let fixed = self.fixed.read().await;
            for (key, count) in ctrl_grouped.iter() {
              let tokens: Vec<&str> = key.split(':').collect();
              let country = fixed.get_geonames_country_by_id(tokens[0]).unwrap();
              let facility = tokens[1];
              metrics.vatsim_objects_online.set(
                labels!(
                  "object_type" = "controller",
                  "controller_type" = facility,
                  "country_code" = &country.iso,
                  "continent_code" = &country.continent
                ),
                *count,
              );
            }
          }
          info!("{} controllers processed in {}s", ccount, process_time);
          // endregion:controllers_processing
        }

        let t = Utc::now();
        let res = self.tracks.read().await.counters();
        let process_time = seconds_since(t);
        match res {
          Ok((tc, tpc)) => {
            let mut metrics = self.metrics.write().await;
            metrics
              .database_objects_count
              .set(labels!("object_type" = "track"), tc);
            metrics
              .database_objects_count
              .set(labels!("object_type" = "trackpoint"), tpc);
            metrics
              .database_objects_count_fetch_time_sec
              .set_single(process_time);
          }
          Err(err) => {
            error!("error getting track store counters: {err}");
          }
        }

        cleanup -= 1;
        if cleanup == 0 {
          let t = Utc::now();
          let res = self.tracks.write().await.cleanup();
          match res {
            Err(err) => error!("error cleaning up track store: {err}"),
            Ok(_) => {
              let process_time = seconds_since(t);
              info!("track store cleanup took {process_time}s");
              cleanup = CLEANUP_EVERY_X_ITER;
            }
          }
        } else {
          debug!("{cleanup} iterations to track store cleanup");
        }

        sleep(self.cfg.api.poll_period).await;
      }
    }
  }

  pub async fn get_pilot_by_callsign(&self, callsign: &str) -> Option<Pilot> {
    self.pilots.read().await.get(callsign).cloned()
  }

  pub async fn get_pilot_track(
    &self,
    pilot: &Pilot,
  ) -> Result<Vec<TrackPoint>, Box<dyn std::error::Error>> {
    Ok(self.tracks.read().await.get_track_points(pilot)?)
  }

  pub async fn get_metrics_clone(&self) -> Metrics {
    self.metrics.read().await.clone()
  }
}
