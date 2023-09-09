use crate::{
  fixed::types::{Airport, FIR},
  moving::pilot::Pilot,
  types::{Point, Rect},
};
use rstar::{RTreeObject, AABB};

#[derive(Debug, Clone)]
pub struct PointObject {
  pub id: String,
  point: Point,
}

impl RTreeObject for PointObject {
  type Envelope = AABB<Point>;

  fn envelope(&self) -> Self::Envelope {
    AABB::from_point(self.point)
  }
}

impl From<&Airport> for PointObject {
  fn from(arpt: &Airport) -> Self {
    Self {
      id: arpt.compound_id(),
      point: arpt.position,
    }
  }
}

impl From<&Pilot> for PointObject {
  fn from(pilot: &Pilot) -> Self {
    Self {
      id: pilot.callsign.clone(),
      point: pilot.position,
    }
  }
}

impl PartialEq for PointObject {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

#[derive(Debug, Clone)]
pub struct RectObject {
  pub id: String,
  rect: Rect,
}

impl RTreeObject for RectObject {
  type Envelope = AABB<Point>;

  fn envelope(&self) -> Self::Envelope {
    AABB::from_corners(self.rect.south_west, self.rect.north_east)
  }
}

impl From<&FIR> for RectObject {
  fn from(fir: &FIR) -> Self {
    Self {
      id: fir.icao.clone(),
      rect: Rect {
        south_west: fir.boundaries.min,
        north_east: fir.boundaries.max,
      },
    }
  }
}

impl PartialEq for RectObject {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use rstar::RTree;

  #[test]
  fn test_intersection() {
    let mut tree = RTree::new();
    let obj = RectObject {
      id: "1".into(),
      rect: Rect {
        south_west: Point { lat: 1.0, lng: 1.0 },
        north_east: Point { lat: 3.0, lng: 3.0 },
      },
    };
    tree.insert(obj.clone());

    let env = AABB::from_corners(Point { lat: 0.0, lng: 0.0 }, Point { lat: 2.0, lng: 2.0 });

    let objs = tree
      .locate_in_envelope_intersecting(&env)
      .collect::<Vec<_>>();
    assert_eq!(objs.len(), 1);
    let objs = tree.locate_in_envelope(&env).collect::<Vec<_>>();
    assert_eq!(objs.len(), 0);
  }
}
