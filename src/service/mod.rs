pub mod camden {
  tonic::include_proto!("camden");
}
mod calc;
mod filter;

use self::camden::map_updates_request::Request as ServiceRequest;
use self::camden::{
  AirportRequest, AirportResponse, BuildInfoResponse, NoParams, PilotRequest, PilotResponse,
  QueryRequest, QueryResponse,
};
use crate::lee::make_expr;
use crate::lee::parser::expression::CompileFunc;
use crate::manager::Manager;
use crate::moving::pilot::Pilot;
use crate::service::filter::compile_filter;
use crate::types::Rect;
use crate::util::seconds_since;
use camden::camden_server::Camden;
use camden::{update::Object, MapUpdatesRequest, Update, UpdateType};
use chrono::Utc;
use log::{debug, error, info};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{self, Sender};
use tokio::time::sleep;
use tokio_stream::{Stream, StreamExt};
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

async fn proxy_requests(mut stream: Streaming<MapUpdatesRequest>, tx: Sender<MapUpdatesRequest>) {
  while let Some(msg) = stream.next().await {
    if let Ok(msg) = msg {
      let res = tx.send(msg).await;
      if let Err(err) = res {
        error!("error sending request via channel: {err:?}");
        break;
      }
    }
  }
}

#[tonic::async_trait]
impl Camden for CamdenService {
  type MapUpdatesStream = Pin<Box<dyn Stream<Item = Result<Update, Status>> + Send + 'static>>;

  async fn map_updates(
    &self,
    request: Request<Streaming<MapUpdatesRequest>>,
  ) -> Result<Response<Self::MapUpdatesStream>, Status> {
    let manager = self.manager.clone();
    let remote = request.remote_addr().unwrap();
    let remote = format!("{:?}", remote);
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

    let output = async_stream::try_stream! {
      let mut rx = rx;

      loop {
        let mut settings_changed = false;
        let res = rx.try_recv();
        match res {
          Err(TryRecvError::Disconnected) => {
            info!("received disconnected error");
            break
          },
          Err(TryRecvError::Empty) => {},
          Ok(msg) => {
            settings_changed = true;
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
              }
            }
          }
        };

        match bounds.as_ref() {
          Some(b) => {
            let rect: Rect = b.clone().into();

            if !settings_changed {
              sleep(Duration::from_secs(5)).await;
            }
            let no_bounds = b.zoom < MIN_ZOOM;

            let t = Utc::now();
            let mut pilots = if no_bounds {
              manager.get_all_pilots().await
            } else {
              manager.get_pilots(&rect).await
            };

            debug!("[{remote}] {} pilots loaded in {}s", pilots.len(), seconds_since(t));

            if let Some(f) = filter.as_ref() {
              pilots.retain(|pilot| f.evaluate(pilot));
            }

            let t = Utc::now();
            let (pilots_set, pilots_delete) = calc::calc_pilots(&pilots, &mut pilots_state);
            debug!("[{remote}] {} pilots diff calculated in {}s, set={}/del={}", pilots.len(), seconds_since(t), pilots_set.len(), pilots_delete.len());

            for pilot in pilots_set.into_iter() {
              let update = Update {
                update_type: UpdateType::Set.into(),
                object: Some(Object::Pilot(pilot.into())),
              };
              yield update;
            }

            for pilot in pilots_delete.into_iter() {
              let update = Update {
                update_type: UpdateType::Delete.into(),
                object: Some(Object::Pilot(pilot.into())),
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

            for arpt in arpts_set.into_iter() {
              let update = Update {
                update_type: UpdateType::Set.into(),
                object: Some(Object::Airport(arpt.into())),
              };
              yield update;
            }

            for arpt in arpts_delete.into_iter() {
              let update = Update {
                update_type: UpdateType::Delete.into(),
                object: Some(Object::Airport(arpt.into())),
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

            for fir in firs_set.into_iter() {
              let update = Update {
                update_type: UpdateType::Set.into(),
                object: Some(Object::Fir(fir.into())),
              };
              yield update;
            }

            for fir in firs_delete.into_iter() {
              let update = Update {
                update_type: UpdateType::Delete.into(),
                object: Some(Object::Fir(fir.into())),
              };
              yield update;
            }
          },
          None => {
            sleep(Duration::from_millis(100)).await;
          }
        }
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
    Ok(Response::new(PilotResponse {
      pilot: pilot.map(|v| v.into()),
    }))
  }

  async fn get_airport(
    &self,
    request: Request<AirportRequest>,
  ) -> Result<Response<AirportResponse>, Status> {
    let request = request.into_inner();
    let airport = self.manager.find_airport(&request.code).await;
    Ok(Response::new(AirportResponse {
      airport: airport.map(|v| v.into()),
    }))
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
}
