use crate::{
  fixed::types::{Airport, FIR},
  moving::pilot::Pilot,
};
use std::collections::{HashMap, HashSet};

pub fn calc_pilots(
  pilots: &[Pilot],
  prev: &mut HashMap<String, Pilot>,
) -> (Vec<Pilot>, Vec<Pilot>) {
  let mut pilots_set = vec![];
  let mut pilots_delete = vec![];
  let mut keys = HashSet::new();

  for pilot in pilots.iter() {
    keys.insert(pilot.callsign.clone());
    let existing = prev.get(&pilot.callsign);

    if let Some(existing) = existing {
      if existing == pilot {
        continue;
      }
    }

    pilots_set.push(pilot.clone());
    prev.insert(pilot.callsign.clone(), pilot.clone());
  }

  let prev_keys = HashSet::from_iter(prev.keys().cloned());
  let keys_to_remove = prev_keys.difference(&keys);

  for cs in keys_to_remove {
    let pilot = prev.remove(cs).unwrap();
    pilots_delete.push(pilot);
  }
  (pilots_set, pilots_delete)
}

pub fn calc_airports(
  airports: &[Airport],
  prev: &mut HashMap<String, Airport>,
) -> (Vec<Airport>, Vec<Airport>) {
  let mut arpts_set = vec![];
  let mut arpts_delete = vec![];
  let mut keys = HashSet::new();

  for arpt in airports.iter() {
    let cmp_id = arpt.compound_id();
    let existing = prev.get(&cmp_id);
    keys.insert(cmp_id);

    if let Some(existing) = existing {
      if existing == arpt {
        continue;
      }
    }

    arpts_set.push(arpt.clone());
    prev.insert(arpt.compound_id(), arpt.clone());
  }

  let prev_keys = HashSet::from_iter(prev.keys().cloned());
  let keys_to_remove = prev_keys.difference(&keys);

  for cs in keys_to_remove {
    let arpt = prev.remove(cs).unwrap();
    arpts_delete.push(arpt);
  }
  (arpts_set, arpts_delete)
}

pub fn calc_firs(firs: &[FIR], prev: &mut HashMap<String, FIR>) -> (Vec<FIR>, Vec<FIR>) {
  let mut firs_set = vec![];
  let mut firs_delete = vec![];
  let mut keys = HashSet::new();

  for fir in firs.iter() {
    let existing = prev.get(&fir.icao);
    keys.insert(fir.icao.clone());
    if let Some(existing) = existing {
      if existing == fir {
        continue;
      }
    }
    firs_set.push(fir.clone());
    prev.insert(fir.icao.clone(), fir.clone());
  }

  let prev_keys = HashSet::from_iter(prev.keys().cloned());
  let keys_to_remove = prev_keys.difference(&keys);
  for key in keys_to_remove {
    let fir = prev.remove(key).unwrap();
    firs_delete.push(fir);
  }

  (firs_set, firs_delete)
}
