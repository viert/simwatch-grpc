pub mod camden {
  tonic::include_proto!("camden");
}

mod calc;
mod filter;

use crate::lee::parser::expression::CompileFunc;
use crate::manager::Manager;
use crate::moving::pilot::Pilot;
use crate::service::filter::compile_filter;
use crate::types::Rect;
use crate::util::seconds_since;
use crate::{lee::make_expr, util::proxy_requests};
use camden::{
  camden_server::Camden, map_updates_request::Request as ServiceRequest, update::ObjectUpdate,
  AirportRequest, AirportResponse, AirportUpdate, BuildInfoResponse, FirUpdate, MapUpdatesRequest,
  MetricSet, MetricSetTextResponse, NoParams, PilotListResponse, PilotRequest, PilotResponse,
  PilotUpdate, QueryRequest, QueryResponse, QuerySubscriptionRequest, QuerySubscriptionRequestType,
  QuerySubscriptionUpdate, QuerySubscriptionUpdateType, Update, UpdateType,
};
use chrono::Utc;
use log::{debug, info};
use std::{
  collections::hash_map::Entry,
  collections::{HashMap, HashSet},
  pin::Pin,
  sync::Arc,
  time::Duration,
};
use tokio::sync::mpsc::{self, error::TryRecvError};
use tokio::time::sleep;
use tokio_stream::Stream;
use tonic::{Request, Response, Status, Streaming};

#[derive(Debug)]
pub struct CamdenService {
  manager: Arc<Manager>,
}

impl CamdenService {
  pub fn new(manager: Arc<Manager>) -> Self {
    Self { manager }
  }
}

// if zoom is less than this, the map might be wrapped on screen, thus we
// need to show all the objects without checking current user map boundaries
const MIN_ZOOM: f64 = 3.0;

#[tonic::async_trait]
impl Camden for CamdenService {
  type MapUpdatesStream = Pin<Box<dyn Stream<Item = Result<Update, Status>> + Send + 'static>>;
  type SubscribeQueryStream =
    Pin<Box<dyn Stream<Item = Result<QuerySubscriptionUpdate, Status>> + Send + 'static>>;

  async fn subscribe_query(
    &self,
    request: Request<Streaming<QuerySubscriptionRequest>>,
  ) -> Result<Response<Self::SubscribeQueryStream>, Status> {
    let manager = self.manager.clone();
    let remote = request.remote_addr().unwrap();
    let remote = format!("subscribe_query:{:?}", remote);
    info!("[{remote}] client connected");
    let stream = request.into_inner();

    let (tx, rx) = mpsc::channel(100);
    tokio::spawn(async move { proxy_requests(stream, tx).await });
    let mut pilots_state = HashMap::new();
    let mut subscriptions = HashMap::new();

    let output = async_stream::try_stream! {
      let mut rx = rx;
      let mut next_update = Utc::now();

      loop {
        let res = rx.try_recv();
        match res {
          Err(TryRecvError::Disconnected) => {
            info!("received disconnected error");
            break
          },
          Err(TryRecvError::Empty) => {},
          Ok(msg) => {
            if let Some(subscription) = msg.subscription {
              const ADD: i32 = QuerySubscriptionRequestType::SubscriptionAdd as i32;
              const DEL: i32 = QuerySubscriptionRequestType::SubscriptionDelete as i32;
              match msg.request_type {
                ADD => {
                  debug!("sub add {subscription:?}");
                  if let Entry::Vacant(e) = subscriptions.entry(subscription.id) {
                    if !subscription.query.is_empty() {
                      let res = make_expr::<Pilot>(&subscription.query);
                      if let Ok(mut expr) = res {
                        let cb: Box<CompileFunc<Pilot>> = Box::new(compile_filter);
                        let filter = expr.compile(&cb).map(|_| expr);
                        if let Ok(filter) = filter {
                          e.insert(filter);
                          next_update = Utc::now();
                        }
                      }
                    }
                  }
                },
                DEL => {
                  debug!("sub del {subscription:?}");
                  if subscriptions.contains_key(&subscription.id) {
                    subscriptions.remove(&subscription.id);
                    next_update = Utc::now();
                  }
                },
                _ => unreachable!()
              }
            }
          }
        }

        let now = Utc::now();
        if now >= next_update {
          let pilots = manager.get_all_pilots().await;
          let (pilots_add, pilots_delete, pilots_fp) = calc::calc_pilots_online(&pilots, &mut pilots_state);

          for pilot in pilots_add.iter() {
            for (id, filter) in subscriptions.iter() {
              if filter.evaluate(pilot) {
                let update = QuerySubscriptionUpdate {
                  subscription_id: id.to_owned(),
                  update_type: QuerySubscriptionUpdateType::Online as i32,
                  pilot: Some(pilot.clone().into())
                };
                yield update;
              }
            }
          }

          for pilot in pilots_fp.iter() {
            for (id, filter) in subscriptions.iter() {
              if filter.evaluate(pilot) {
                let update = QuerySubscriptionUpdate {
                  subscription_id: id.to_owned(),
                  update_type: QuerySubscriptionUpdateType::Flightplan as i32,
                  pilot: Some(pilot.clone().into())
                };
                yield update;
              }
            }
          }

          for pilot in pilots_delete.iter() {
            for (id, filter) in subscriptions.iter() {
              if filter.evaluate(pilot) {
                let update = QuerySubscriptionUpdate {
                  subscription_id: id.to_owned(),
                  update_type: QuerySubscriptionUpdateType::Offline as i32,
                  pilot: Some(pilot.clone().into())
                };
                yield update;
              }
            }
          }

          next_update = Utc::now() + Duration::from_secs(5);
        }
        sleep(Duration::from_millis(50)).await;
      }

      info!("[{remote}] client disconnected");

    };
    Ok(Response::new(Box::pin(output) as Self::SubscribeQueryStream))
  }

  async fn map_updates(
    &self,
    request: Request<Streaming<MapUpdatesRequest>>,
  ) -> Result<Response<Self::MapUpdatesStream>, Status> {
    let manager = self.manager.clone();
    let remote = request.remote_addr().unwrap();
    let remote = format!("map_updates:{:?}", remote);
    info!("[{remote}] client connected");
    let stream = request.into_inner();
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(async move { proxy_requests(stream, tx).await });

    let mut bounds = None;
    let mut filter = None;
    let mut show_wx = false;

    let mut pilots_state = HashMap::new();
    let mut airports_state = HashMap::new();
    let mut firs_state = HashMap::new();
    let mut subscriptions = HashSet::new();

    let output = async_stream::try_stream! {
      let mut rx = rx;
      let mut next_update = Utc::now();

      loop {
        let res = rx.try_recv();

        match res {
          Err(TryRecvError::Disconnected) => {
            info!("received disconnected error");
            break
          },
          Err(TryRecvError::Empty) => {},
          Ok(msg) => {
            next_update = Utc::now();
            if msg.request.is_some() {
              let req = msg.request.unwrap();
              match req {
                ServiceRequest::Filter(flt) => {
                  debug!("client {:?} filter request {}", remote, flt);
                  filter = {
                    if !flt.is_empty() {
                      let res = make_expr::<Pilot>(&flt);
                      if let Ok(mut expr) = res {
                        let cb: Box<CompileFunc<Pilot>> = Box::new(compile_filter);
                        expr.compile(&cb).map(|_| expr).ok()
                      } else {
                        None
                      }
                    } else {
                      None
                    }
                  };
                }
                ServiceRequest::Bounds(bds) => {
                  debug!("client {:?} bounds request {:?}", remote, bds);
                  bounds = Some(bds);
                }
                ServiceRequest::ShowWx(value) => {
                  debug!("client {:?} show_wx request {}", remote, value);
                  show_wx = value;
                }
                ServiceRequest::SubscribeId(value) => {
                  debug!("client {:?} subscribe request {}", remote, value);
                  subscriptions.insert(value);
                }
                ServiceRequest::UnsubscribeId(value) => {
                  debug!("client {:?} unsubscribe request {}", remote, value);
                  subscriptions.remove(&value);
                }
              }
            }
          }
        };

        match bounds.as_ref() {
          Some(b) => {

            let dt = Utc::now();
            if dt >= next_update {
              let rect: Rect = b.clone().into();
              let no_bounds = b.zoom < MIN_ZOOM;

              let t = Utc::now();
              let mut pilots = if no_bounds {
                manager.get_all_pilots().await
              } else {
                manager.get_pilots(&rect, &subscriptions).await
              };

              debug!("[{remote}] {} pilots loaded in {}s", pilots.len(), seconds_since(t));

              if let Some(f) = filter.as_ref() {
                pilots.retain(|pilot| subscriptions.contains(&pilot.callsign) || f.evaluate(pilot));
              }

              let t = Utc::now();
              let (pilots_set, pilots_delete) = calc::calc_pilots(&pilots, &mut pilots_state);
              debug!("[{remote}] {} pilots diff calculated in {}s, set={}/del={}", pilots.len(), seconds_since(t), pilots_set.len(), pilots_delete.len());

              let objects: Vec<camden::Pilot> = pilots_set.into_iter().map(|p| p.into()).collect();
              if !objects.is_empty() {
                let update = Update {
                  object_update: Some(ObjectUpdate::PilotUpdate(PilotUpdate {
                    update_type: UpdateType::Set as i32,
                    pilots: objects,
                  })),
                };
                yield update;
              }

              let objects: Vec<camden::Pilot> = pilots_delete.into_iter().map(|p| p.into()).collect();
              if !objects.is_empty() {
                let update = Update {
                  object_update: Some(ObjectUpdate::PilotUpdate(PilotUpdate {
                    update_type: UpdateType::Delete as i32,
                    pilots: objects,
                  })),
                };
                yield update;
              }


              let t = Utc::now();
              let airports = if no_bounds {
                manager.get_all_airports(show_wx).await
              } else {
                manager.get_airports(&rect, show_wx).await
              };

              debug!("[{remote}] {} airports loaded in {}s", airports.len(), seconds_since(t));
              let t = Utc::now();
              let (arpts_set, arpts_delete) = calc::calc_airports(&airports, &mut airports_state);
              debug!("[{remote}] {} airports diff calculated in {}s, set={}/del={}", airports.len(), seconds_since(t), arpts_set.len(), arpts_delete.len());

              let objects: Vec<camden::Airport> = arpts_set.into_iter().map(|a| a.into()).collect();
              if !objects.is_empty() {
                let update = Update {
                 object_update: Some(ObjectUpdate::AirportUpdate(AirportUpdate {
                    update_type: UpdateType::Set as i32,
                    airports: objects,
                  })),
                };
                yield update;
              }

              let objects: Vec<camden::Airport> = arpts_delete.into_iter().map(|a| a.into()).collect();
              if !objects.is_empty() {
                let update = Update {
                  object_update: Some(ObjectUpdate::AirportUpdate(AirportUpdate {
                    update_type: UpdateType::Delete as i32,
                    airports: objects,
                  })),
                };
                yield update;
              }

              let t = Utc::now();
              let firs = if no_bounds {
                manager.get_all_firs().await
              } else {
                manager.get_firs(&rect).await
              };

              debug!("[{remote}] {} firs loaded in {}s", firs.len(), seconds_since(t));
              let t = Utc::now();
              let (firs_set, firs_delete) = calc::calc_firs(&firs, &mut firs_state);
              debug!("[{remote}] {} firs diff calculated in {}s, set={}/del={}", firs.len(), seconds_since(t), firs_set.len(), firs_delete.len());

              let objects: Vec<camden::Fir> = firs_set.into_iter().map(|f| f.into()).collect();
              if !objects.is_empty() {
                let update = Update {
                  object_update: Some(ObjectUpdate::FirUpdate(FirUpdate {
                    update_type: UpdateType::Set as i32,
                    firs: objects,
                  })),
                };
                yield update;
              }

              let objects: Vec<camden::Fir> = firs_delete.into_iter().map(|f| f.into()).collect();
              if !objects.is_empty() {
                let update = Update {
                  object_update: Some(ObjectUpdate::FirUpdate(FirUpdate {
                    update_type: UpdateType::Delete as i32,
                    firs: objects,
                  })),
                };
                yield update;
              }

              next_update = dt + Duration::from_secs(5);
            }
          },
          None => {}
        }
        sleep(Duration::from_millis(50)).await;
      }

      info!("[{remote}] client disconnected");
    };

    Ok(Response::new(Box::pin(output) as Self::MapUpdatesStream))
  }

  async fn get_pilot(
    &self,
    request: Request<PilotRequest>,
  ) -> Result<Response<PilotResponse>, Status> {
    let request = request.into_inner();
    let pilot = self.manager.get_pilot_by_callsign(&request.callsign).await;
    match pilot {
      Some(pilot) => {
        let tps = self
          .manager
          .get_pilot_track(&pilot)
          .await
          .map_err(|err| Status::unavailable(format!("{err}")))?;
        let mut pilot: camden::Pilot = pilot.into();

        pilot.track = tps.into_iter().map(|tp| tp.into()).collect();

        Ok(Response::new(PilotResponse { pilot: Some(pilot) }))
      }
      None => Err(Status::not_found("pilot not found")),
    }
  }

  async fn list_pilots(
    &self,
    request: Request<QueryRequest>,
  ) -> Result<Response<PilotListResponse>, Status> {
    let request = request.into_inner();
    let mut pilots = self.manager.get_all_pilots().await;

    if !request.query.is_empty() {
      let expr = make_expr::<Pilot>(&request.query);
      match expr {
        Ok(mut expr) => {
          let cb: Box<CompileFunc<Pilot>> = Box::new(compile_filter);
          let res = expr.compile(&cb);
          match res {
            Ok(_) => {
              pilots = pilots
                .into_iter()
                .filter(|pilot| expr.evaluate(pilot))
                .collect()
            }
            Err(err) => {
              return Err(Status::failed_precondition(format!(
                "query compile error: {err}"
              )));
            }
          }
        }
        Err(err) => {
          return Err(Status::failed_precondition(format!(
            "query parse error: {err}"
          )));
        }
      }
    }

    Ok(Response::new(PilotListResponse {
      pilots: pilots.into_iter().map(|pilot| pilot.into()).collect(),
    }))
  }

  async fn get_airport(
    &self,
    request: Request<AirportRequest>,
  ) -> Result<Response<AirportResponse>, Status> {
    let request = request.into_inner();
    let airport = self.manager.find_airport(&request.code).await;
    match airport {
      Some(airport) => Ok(Response::new(AirportResponse {
        airport: Some(airport.into()),
      })),
      None => Err(Status::not_found("airport not found")),
    }
  }

  async fn check_query(
    &self,
    request: Request<QueryRequest>,
  ) -> Result<Response<QueryResponse>, Status> {
    let request = request.into_inner();
    let res = make_expr::<Pilot>(&request.query);
    match res {
      Ok(expr) => {
        let mut expr = expr;
        let cb: Box<CompileFunc<Pilot>> = Box::new(compile_filter);
        let res = expr.compile(&cb);
        match res {
          Ok(_) => Ok(Response::new(QueryResponse {
            valid: true,
            error_message: None,
          })),
          Err(err) => Ok(Response::new(QueryResponse {
            valid: false,
            error_message: Some(format!("{err}")),
          })),
        }
      }
      Err(err) => Ok(Response::new(QueryResponse {
        valid: false,
        error_message: Some(format!("{err}")),
      })),
    }
  }

  async fn build_info(&self, _: Request<NoParams>) -> Result<Response<BuildInfoResponse>, Status> {
    let pkgname = env!("CARGO_PKG_NAME").to_owned();
    let pkgversion = env!("CARGO_PKG_VERSION").to_owned();
    let repository = env!("CARGO_PKG_REPOSITORY").to_owned();
    let license_file = env!("CARGO_PKG_LICENSE_FILE").to_owned();
    Ok(Response::new(BuildInfoResponse {
      name: pkgname,
      version: pkgversion,
      repository,
      license: license_file,
    }))
  }

  async fn get_metrics(&self, _: Request<NoParams>) -> Result<Response<MetricSet>, Status> {
    let metrics = self.manager.get_metrics_clone().await;
    Ok(Response::new(metrics.into()))
  }

  async fn get_metrics_text(
    &self,
    _: Request<NoParams>,
  ) -> Result<Response<MetricSetTextResponse>, Status> {
    let text = self.manager.render_metrics().await;
    Ok(Response::new(MetricSetTextResponse { text }))
  }
}
