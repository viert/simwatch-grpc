use super::types::Boundaries;
use crate::types::Point;
use geojson::{Feature, FeatureCollection, GeoJson};
use log::error;
use std::{collections::HashMap, error::Error};

fn lng_less(a: f64, b: f64) -> bool {
  let d1 = (b - a).rem_euclid(360.0);
  let d2 = (a - b).rem_euclid(360.0);
  d1 < d2
}

fn lng_center(min: f64, max: f64) -> f64 {
  if min < max {
    (min + max) / 2.0
  } else {
    let min = (min + 360.0) % 360.0;
    let max = (max + 360.0) % 360.0;
    let center = (min + max) / 2.0;
    center - 360.0
  }
}

fn extract_boundaries(feat: &Feature) -> Option<Boundaries> {
  let props = &feat.properties;
  let geom = feat.geometry.as_ref()?;
  if let Some(props) = props {
    let id = props.get("id")?.as_str()?.to_owned();
    let is_oceanic = props.get("oceanic")?.as_str()? == "1";
    let region = props.get("region")?.as_str()?.to_owned();
    let division = props.get("division")?.as_str()?.to_owned();
    let mut points = vec![];
    let mut min_lng = 0.0;
    let mut max_lng = 0.0;
    let mut min_lat = 0.0;
    let mut max_lat = 0.0;
    let mut minmax_initialised = false;
    match &geom.value {
      geojson::Value::MultiPolygon(mpoly) => {
        for poly in mpoly {
          let mut ppoly = vec![];
          for inner in poly {
            for inner in inner {
              let (lng, lat) = (inner[0], inner[1]);

              if minmax_initialised {
                if min_lat > lat {
                  min_lat = lat;
                }
                if max_lat < lat {
                  max_lat = lat;
                }
                if lng_less(max_lng, lng) {
                  max_lng = lng;
                }
                if lng_less(lng, min_lng) {
                  min_lng = lng;
                }
              } else {
                min_lat = lat;
                max_lat = lat;
                min_lng = lng;
                max_lng = lng;
                minmax_initialised = true;
              }

              ppoly.push(Point { lat, lng });
            }
          }
          points.push(ppoly)
        }
      }
      _ => return None,
    };

    let min = Point {
      lat: min_lat,
      lng: min_lng,
    };

    let max = Point {
      lat: max_lat,
      lng: max_lng,
    };

    let center_lat = (min_lat + max_lat) / 2.0;
    let center_lng = lng_center(min_lng, max_lng);
    let center = Point {
      lat: center_lat,
      lng: center_lng,
    };

    Some(Boundaries {
      id,
      region,
      division,
      is_oceanic,
      min,
      max,
      center,
      points,
    })
  } else {
    error!("no props found in feature {:?}", feat);
    None
  }
}

pub async fn load_boundaries(url: &str) -> Result<HashMap<String, Boundaries>, Box<dyn Error>> {
  let raw_geojson = reqwest::get(url).await?.text().await?;
  let geo = raw_geojson.parse::<GeoJson>()?;
  let coll = FeatureCollection::try_from(geo)?;
  let mut res = HashMap::new();
  for feature in coll {
    let boundaries = extract_boundaries(&feature);
    if let Some(boundaries) = boundaries {
      res.insert(boundaries.id.clone(), boundaries);
    }
  }
  Ok(res)
}

#[cfg(test)]
mod test {
  use super::lng_less;

  #[test]
  fn test_lng_less() {
    assert!(lng_less(0.0, 10.0));
    assert!(lng_less(-10.0, 10.0));
    assert!(lng_less(170.0, -150.0))
  }
}
