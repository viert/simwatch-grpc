use crate::{config::Config, moving::pilot::Pilot};
use chrono::{Duration, Utc};
use log::{error, info};
use mongodb::{
  bson::{doc, oid::ObjectId, DateTime},
  options::{ClientOptions, FindOptions},
  Client, Collection, Database, IndexModel,
};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;

#[derive(Debug)]
pub struct Persistent {
  db: Database,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Track {
  pub _id: Option<ObjectId>,
  pub code: String,
  pub created_at: DateTime,
}

impl Track {
  pub fn collection() -> &'static str {
    "tracks"
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackPoint {
  #[serde(skip_serializing)]
  pub _id: Option<ObjectId>,
  #[serde(skip_serializing)]
  pub track_id: ObjectId,

  pub lat: f64,
  pub lng: f64,
  pub alt: i32,
  pub hdg: i16,
  pub gs: i32,
  pub ts: i64,
}

impl TrackPoint {
  pub fn collection() -> &'static str {
    "track_points"
  }
}

impl Persistent {
  pub async fn new(cfg: &Config) -> Result<Self, mongodb::error::Error> {
    let opts = ClientOptions::parse(&cfg.track.uri).await?;
    let client = Client::with_options(opts)?;
    let db = client.database(&cfg.track.dbname);
    Ok(Self { db })
  }

  pub async fn indexes(&self) -> Result<(), mongodb::error::Error> {
    let coll: Collection<Track> = self.db.collection(Track::collection());
    coll
      .create_index(
        IndexModel::builder()
          .keys(doc! {
            "code": 1
          })
          .build(),
        None,
      )
      .await?;

    let coll: Collection<TrackPoint> = self.db.collection(TrackPoint::collection());
    coll
      .create_index(
        IndexModel::builder()
          .keys(doc! {
            "track_id": 1,
            "ts": 1,
          })
          .build(),
        None,
      )
      .await?;
    Ok(())
  }

  pub async fn cleanup(&self) -> Result<(), mongodb::error::Error> {
    let coll: Collection<Track> = self.db.collection(Track::collection());
    let dt = Utc::now() - Duration::days(2);
    let dt = DateTime::from_chrono(dt);
    let query = doc! {
      "created_at": doc! {
        "$lt": dt
      }
    };
    let mut cur = coll.find(query, None).await?;
    let mut count = 0;
    let mut tp_count = 0;
    while let Some(track) = cur.try_next().await? {
      let res = self.drop_track(&track).await;
      match res {
        Err(err) => error!("error dropping track: {err}"),
        Ok(cnt) => {
          count += 1;
          tp_count += cnt;
        }
      }
    }
    info!("{count} tracks and {tp_count} track points dropped");
    Ok(())
  }

  pub async fn counters(&self) -> Result<(u64, u64), mongodb::error::Error> {
    let coll: Collection<TrackPoint> = self.db.collection(TrackPoint::collection());
    let tp_count = coll.estimated_document_count(None).await?;
    let coll: Collection<Track> = self.db.collection(Track::collection());
    let t_count = coll.estimated_document_count(None).await?;
    Ok((t_count, tp_count))
  }

  pub async fn drop_track(&self, track: &Track) -> Result<u64, mongodb::error::Error> {
    let coll: Collection<TrackPoint> = self.db.collection(TrackPoint::collection());
    let query = doc! {"track_id": track._id.unwrap() };
    let del_res = coll.delete_many(query, None).await?;
    let tp_count = del_res.deleted_count;

    let coll: Collection<Track> = self.db.collection(Track::collection());
    let query = doc! {"_id": track._id.unwrap() };
    coll.delete_one(query, None).await?;
    Ok(tp_count)
  }

  pub async fn store_track(&self, pilot: &Pilot) -> Result<(), mongodb::error::Error> {
    let now = DateTime::now();
    let coll: Collection<Track> = self.db.collection(Track::collection());
    let code = pilot.track_code();
    let track = coll.find_one(doc! { "code": code.clone() }, None).await?;
    let track_id = if track.is_none() {
      let d = doc! {
        "code": code,
        "created_at": now
      };
      let res = self
        .db
        .collection(Track::collection())
        .insert_one(d, None)
        .await?;
      res.inserted_id.as_object_id().unwrap()
    } else {
      track.unwrap()._id.unwrap()
    };

    let point = doc! {
      "track_id": track_id,
      "lat": pilot.position.lat,
      "lng": pilot.position.lng,
      "alt": pilot.altitude,
      "hdg": pilot.heading as i32,
      "gs": pilot.groundspeed,
      "ts": now.timestamp_millis(),
    };
    let coll = self.db.collection(TrackPoint::collection());
    coll.insert_one(point, None).await?;
    Ok(())
  }

  async fn get_track_by_code(&self, code: &str) -> Result<Option<Track>, mongodb::error::Error> {
    let query = doc! {"code": code};
    let coll: Collection<Track> = self.db.collection(Track::collection());
    coll.find_one(query, None).await
  }

  pub async fn get_track_points(
    &self,
    pilot: &Pilot,
  ) -> Result<Option<Vec<TrackPoint>>, mongodb::error::Error> {
    let track = self.get_track_by_code(&pilot.track_code()).await?;
    if let Some(track) = track {
      let track_id = track._id.unwrap();
      let coll: Collection<TrackPoint> = self.db.collection(TrackPoint::collection());
      let opts = FindOptions::builder().sort(doc! {"ts": 1}).build();
      let mut cur = coll.find(doc! {"track_id": track_id}, opts).await?;
      let mut tps = vec![];
      while let Some(tp) = cur.try_next().await? {
        tps.push(tp);
      }
      Ok(Some(tps))
    } else {
      Ok(None)
    }
  }
}
