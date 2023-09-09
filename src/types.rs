use geo_types::{Coord, Point as GeoPoint};
use rstar::AABB;
use serde::Serialize;

use crate::service::camden::{self, MapBounds};

const MAX_LNG: f64 = 179.9999;
const MIN_LNG: f64 = -179.9999;

#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
pub struct Point {
  pub lat: f64,
  pub lng: f64,
}

impl From<Point> for GeoPoint {
  fn from(val: Point) -> Self {
    Self(Coord {
      x: val.lng,
      y: val.lat,
    })
  }
}

impl From<Point> for camden::Point {
  fn from(value: Point) -> Self {
    Self {
      lat: value.lat,
      lng: value.lng,
    }
  }
}

impl Point {
  pub fn clamp(&self) -> Self {
    Self {
      lat: self.lat.clamp(-90.0, 90.0), // don't wrap lat, just clamp
      lng: (self.lng + 180.0).rem_euclid(360.0) - 180.0, // make sure lng is wrapped to stay within -180..180
    }
  }

  pub fn envelope(self) -> AABB<Point> {
    AABB::from_point(self)
  }
}

impl rstar::Point for Point {
  type Scalar = f64;
  const DIMENSIONS: usize = 2;

  fn generate(mut generator: impl FnMut(usize) -> Self::Scalar) -> Self {
    let lng = generator(0);
    let lat = generator(1);
    Self { lat, lng }
  }

  fn nth(&self, index: usize) -> Self::Scalar {
    match index {
      0 => self.lng,
      1 => self.lat,
      _ => unreachable!(),
    }
  }

  fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
    match index {
      0 => &mut self.lng,
      1 => &mut self.lat,
      _ => unreachable!(),
    }
  }
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct Rect {
  pub south_west: Point,
  pub north_east: Point,
}

impl Rect {
  pub fn new(min_lng: f64, min_lat: f64, max_lng: f64, max_lat: f64) -> Self {
    Self {
      south_west: Point {
        lng: min_lng,
        lat: min_lat,
      },
      north_east: Point {
        lng: max_lng,
        lat: max_lat,
      },
    }
  }

  fn width(&self) -> f64 {
    (self.north_east.lng + 180.0) - (self.south_west.lng + 180.0)
  }

  fn height(&self) -> f64 {
    self.north_east.lat - self.south_west.lat
  }

  pub fn scale(&self, multiplier: f64) -> Self {
    let ext = multiplier - 1.0;
    let lng_ext = self.width() * ext / 2.0;
    let lat_ext = self.height() * ext / 2.0;
    let south_west = Point {
      lat: self.south_west.lat - lat_ext,
      lng: self.south_west.lng - lng_ext,
    };
    let north_east = Point {
      lat: self.north_east.lat + lat_ext,
      lng: self.north_east.lng + lng_ext,
    };
    Self {
      south_west: south_west.clamp(),
      north_east: north_east.clamp(),
    }
  }

  pub fn envelopes(&self) -> Vec<AABB<Point>> {
    // AABB does silly things when the leftmost point has a positive longitude
    // and the rightmost one has a negative one. AABB simply swaps them in constructor,
    // that's not the behaviour we need.
    if self.south_west.lng > 0.0 && self.north_east.lng < 0.0 {
      vec![
        AABB::from_corners(
          Point {
            lat: self.south_west.lat,
            lng: self.south_west.lng,
          },
          Point {
            lat: self.north_east.lat,
            lng: MAX_LNG,
          },
        ),
        AABB::from_corners(
          Point {
            lat: self.south_west.lat,
            lng: MIN_LNG,
          },
          Point {
            lat: self.north_east.lat,
            lng: self.north_east.lng,
          },
        ),
      ]
    } else {
      vec![AABB::from_corners(self.south_west, self.north_east)]
    }
  }
}

impl From<MapBounds> for Rect {
  fn from(value: MapBounds) -> Self {
    let sw = match value.sw {
      Some(v) => Point {
        lat: v.lat,
        lng: v.lng,
      },
      None => Point { lat: 0.0, lng: 0.0 },
    };
    let ne = match value.ne {
      Some(v) => Point {
        lat: v.lat,
        lng: v.lng,
      },
      None => Point { lat: 0.0, lng: 0.0 },
    };

    Self {
      south_west: Point {
        lat: sw.lat,
        lng: sw.lng,
      },
      north_east: Point {
        lat: ne.lat,
        lng: ne.lng,
      },
    }
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;

  #[test]
  fn test_rect_wrap() {
    let rect = Rect::new(170.0, 0.0, -170.0, 10.0);
    let envs = rect.envelopes();
    assert_eq!(envs.len(), 2);

    assert_eq!(
      envs[0].lower(),
      Point {
        lat: 0.0,
        lng: 170.0
      }
    );
    assert_eq!(
      envs[0].upper(),
      Point {
        lat: 10.0,
        lng: MAX_LNG
      }
    );

    assert_eq!(
      envs[1].lower(),
      Point {
        lat: 0.0,
        lng: MIN_LNG
      }
    );
    assert_eq!(
      envs[1].upper(),
      Point {
        lat: 10.0,
        lng: -170.0
      }
    );
  }

  #[test]
  fn test_nowrap() {
    let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
    let envs = rect.envelopes();
    assert_eq!(envs.len(), 1);
    assert_eq!(envs[0].lower(), Point { lat: 0.0, lng: 0.0 });
    assert_eq!(
      envs[0].upper(),
      Point {
        lat: 10.0,
        lng: 10.0
      }
    );
  }
}
