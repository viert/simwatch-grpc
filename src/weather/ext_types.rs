use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use crate::service::camden;

const WHY_IS_IT_EVEN_CUSTOM_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
  D: Deserializer<'de>,
{
  let s = String::deserialize(deserializer)?;
  let dt = DateTime::parse_from_str(&s, WHY_IS_IT_EVEN_CUSTOM_FORMAT)
    .map_err(serde::de::Error::custom)?
    .with_timezone(&Utc);
  Ok(dt)
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum WindDirection {
  Variable(String),
  Degree(u64),
}

impl From<WindDirection> for camden::weather_info::WindDirection {
  fn from(value: WindDirection) -> Self {
    match value {
      WindDirection::Variable(v) => camden::weather_info::WindDirection::WindDirectionVariable(v),
      WindDirection::Degree(v) => camden::weather_info::WindDirection::WindDirectionDeg(v as u32),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Metar {
  pub metar_id: u64,
  #[serde(rename(deserialize = "icaoId"))]
  pub icao_id: String,
  #[serde(
    rename(deserialize = "receiptTime"),
    deserialize_with = "deserialize_datetime"
  )]
  pub receipt_time: DateTime<Utc>,
  #[serde(
    rename(deserialize = "reportTime"),
    deserialize_with = "deserialize_datetime"
  )]
  pub report_time: DateTime<Utc>,
  pub temp: Option<f64>,
  pub dewp: Option<f64>,
  pub wdir: Option<WindDirection>,
  pub wspd: Option<u64>,
  pub wgst: Option<u64>,
  #[serde(rename(deserialize = "rawOb"))]
  pub raw_ob: String,
}

#[cfg(test)]
pub mod tests {
  use super::*;

  #[tokio::test]
  async fn test_struct() {
    let res =
      reqwest::get("https://beta.aviationweather.gov/cgi-bin/data/metar.php?ids=EGLL&format=json")
        .await;

    if let Err(err) = res {
      println!("{err}");
      return;
    }

    let resp = res.unwrap();
    let text = resp.text().await.unwrap();

    let res = serde_json::from_str::<Vec<Metar>>(&text);
    match res {
      Ok(data) => println!("{data:?}"),
      Err(err) => println!("{err}"),
    }
  }
}
