pub mod aircraft;
pub mod controller;
pub mod data;
mod exttypes;
pub mod pilot;

use crate::config::Config;
use data::Data;
use log::error;

pub async fn load_vatsim_data(cfg: &Config) -> Option<Data> {
  let res = reqwest::get(&cfg.api.url).await;
  let response = match res {
    Ok(response) => response,
    Err(err) => {
      error!("error loading vatsim data: {err:?}");
      return None;
    }
  };
  let res = response.json::<exttypes::Data>().await;
  let data = match res {
    Ok(data) => data,
    Err(err) => {
      error!("error parsing vatsim data: {err:?}");
      return None;
    }
  };
  Some(data.into())
}
