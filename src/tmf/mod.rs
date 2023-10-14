pub mod proto {
  tonic::include_proto!("tmf");
}
pub mod types;

use self::proto::{
  track_server::Track, track_stream_request, EchoResponse, TrackStreamAck, TrackStreamRequest,
  TrackStreamResponse,
};
use self::types::{Header, TrackEntry};
use crate::{trackfile::TrackFile, util::proxy_requests};
use chrono::Utc;
use log::info;
use std::{fs::create_dir_all, path::Path, pin::Pin, time::Duration};
use tokio::sync::mpsc::{self, error::TryRecvError};
use tokio::time::sleep;
use tokio_stream::Stream;
use tonic::{metadata::MetadataMap, Request, Response, Status, Streaming};

#[derive(Debug)]
pub struct TrackService {
  folder: String,
}

impl TrackService {
  pub fn new(dirname: &str) -> Self {
    Self {
      folder: dirname.into(),
    }
  }
}

struct FlightMeta {
  flight_id: String,
  atc_id: String,
  atc_type: Option<String>,
  atc_flight_number: Option<String>,
  aircraft_title: Option<String>,
}

fn extract_key(meta: &MetadataMap, key: &str) -> Result<String, Status> {
  match meta.get(key) {
    Some(value) => {
      let res = value.to_str();
      match res {
        Ok(value) => Ok(value.into()),
        Err(_) => Err(Status::invalid_argument(&format!("invalid {key} header"))),
      }
    }
    None => Err(Status::invalid_argument(format!("{key} header is missing"))),
  }
}

impl TryFrom<&MetadataMap> for FlightMeta {
  type Error = Status;

  fn try_from(value: &MetadataMap) -> Result<Self, Self::Error> {
    let flight_id = extract_key(value, "x-flight-id")?;
    let atc_id = extract_key(value, "x-atc-id")?;
    let atc_type = extract_key(value, "x-atc-type").ok();
    let atc_flight_number = extract_key(value, "x-atc-flight-number").ok();
    let aircraft_title = extract_key(value, "x-title").ok();
    Ok(Self {
      flight_id,
      atc_id,
      atc_type,
      atc_flight_number,
      aircraft_title,
    })
  }
}

#[tonic::async_trait]
impl Track for TrackService {
  type TrackStreamStream =
    Pin<Box<dyn Stream<Item = Result<TrackStreamResponse, Status>> + Send + 'static>>;

  async fn track_stream(
    &self,
    request: Request<Streaming<TrackStreamRequest>>,
  ) -> Result<Response<Self::TrackStreamStream>, Status> {
    let remote = request.remote_addr().unwrap();
    let remote = format!("track_my_flight:{:?}", remote);
    let mut seq_number = 1;
    info!("[{remote}] client connected");

    let flight_meta: FlightMeta = request.metadata().try_into()?;
    let stream = request.into_inner();
    let filename = Path::new(&self.folder);
    let res = create_dir_all(filename);
    if let Err(err) = res {
      return Err(Status::internal(format!(
        "can't create track folder: {err}"
      )));
    }
    let filename = filename.join(format!("{}.bin", flight_meta.flight_id));
    let mut tf: TrackFile<TrackEntry, Header> = TrackFile::new(filename.to_str().unwrap())?;

    let (tx, rx) = mpsc::channel(100);
    tokio::spawn(async move { proxy_requests(stream, tx).await });

    let output = async_stream::try_stream! {
      let mut rx = rx;
      loop {
        let res = rx.try_recv();
        match res {
          Err(TryRecvError::Disconnected) => {
            info!("received disconnected error");
            break
          },
          Err(TryRecvError::Empty) => {
            sleep(Duration::from_millis(10)).await;
          },
          Ok(msg) => {
            match msg.union.unwrap() {
              track_stream_request::Union::TrackMessage(msg) => {
                let entry: TrackEntry = msg.into();
                tf.append(&entry)?;

                let msg = TrackStreamResponse {
                  ack: Some(TrackStreamAck {
                    seq_number,
                    echo_response: None
                  })
                };
                yield msg;
              },
              track_stream_request::Union::EchoRequest(req) => {
                let client_ts = req.timestamp_us;
                let server_ts = Utc::now().timestamp_micros() as u64;
                let resp = EchoResponse {
                  client_timestamp_us: client_ts,
                  server_timestamp_us: server_ts,
                };
                let msg = TrackStreamResponse {
                  ack: Some(TrackStreamAck {
                    seq_number,
                    echo_response: Some(resp)
                  })
                };
                yield msg;
              },
            };
            seq_number += 1;
          }
        }
      }
      info!("[{remote}] client disconnected");
    };

    Ok(Response::new(Box::pin(output) as Self::TrackStreamStream))
  }
}
