use std::{collections::HashSet, str::FromStr};

use lazy_static::lazy_static;
use regex::Regex;

const IDENT_EXPR: &str = r"(\d{2}(?:[LRC]|\s(?:LEFT|RIGHT|CENTER))?)(?:\s(?:(?:AND|OR)\s)?(\d{2}(?:[LRC]|\s(?:LEFT|RIGHT|CENTER))?))?(?:\s(?:(?:AND|OR)\s)?(\d{2}(?:[LRC]|\s(?:LEFT|RIGHT|CENTER))?))?";

lazy_static! {
  static ref WHITESPACE: Regex = Regex::from_str(r"\s+").unwrap();
  static ref SPECIAL: Regex = Regex::from_str(r"[^A-Z0-9\s]").unwrap();
  static ref COLLAPSE_NUMS: Regex = Regex::from_str(r"(\d)\s+(\d)").unwrap();
  static ref ARRIVAL_EXPR: Vec<Regex> = [
    r"(?:(?:APPROACH|ARRIVAL|LANDING|LDG)\s)+(?:RUNWAY|RWY)S?\s".to_owned() + IDENT_EXPR,
    r"(?:RUNWAY|RWY)S?\s".to_owned() + IDENT_EXPR + r"\sFOR\s(?:ARRIVAL|LANDING|LDG|APPROACH)",
    r"(?:RUNWAY|RWY)S?\s".to_owned() + IDENT_EXPR + r"\sIN\sUSE",
    r"(?:RUNWAY|RWY)S?\sIN\sUSE\s".to_owned() + IDENT_EXPR,
    r"(?:APPROACH|ARRIVAL|LANDING|LDG)\sAND\s(?:TAKEOFF|DEPARTURE|DEPARTING|DEP)\s(?:RUNWAY|RWY)S?\s".to_owned()
      + IDENT_EXPR,
  ].into_iter().map(|expr| Regex::from_str(&expr).unwrap()).collect();

  static ref DEPARTURE_EXPR: Vec<Regex> = [
    r"(?:TAKEOFF|DEPARTURE|DEPARTING|DEP)\s(?:RUNWAY|RWY)S?\s".to_owned() + IDENT_EXPR,
    r"(?:RUNWAY|RWY)S?\s".to_owned() + IDENT_EXPR + r"\sFOR\s(?:TAKEOFF|DEPARTURE|DEP)",
    r"(?:RUNWAY|RWY)S?\s".to_owned() + IDENT_EXPR + r"\sIN\sUSE",
    r"(?:RUNWAY|RWY)S?\sIN\sUSE\s".to_owned() + IDENT_EXPR,
    r"(?:APPROACH|ARRIVAL|LANDING|LDG)\sAND\s(?:TAKEOFF|DEPARTURE|DEPARTING|DEP)\s(?:RUNWAY|RWY)S?\s".to_owned()
      + IDENT_EXPR,
  ].into_iter().map(|expr| Regex::from_str(&expr).unwrap()).collect();
}

fn normalize_runway_ident(ident: &str) -> String {
  let ident = WHITESPACE.replace_all(ident, "").to_string();
  if ident.len() > 3 {
    ident[0..3].to_owned()
  } else {
    ident
  }
}

pub fn normalize_atis_text(text: &str, collapse_nums: bool) -> String {
  let text = text.to_uppercase();
  let text = SPECIAL.replace_all(&text, "");
  let text = WHITESPACE.replace_all(&text, " ");
  let text = if collapse_nums {
    COLLAPSE_NUMS.replace_all(&text, "$1$2")
  } else {
    text
  };
  text.trim().to_owned()
}

pub fn detect_arrivals(norm_atis: &str) -> Vec<String> {
  let mut res = HashSet::new();
  if !norm_atis.is_empty() {
    for expr in ARRIVAL_EXPR.iter() {
      let cap = expr.captures(norm_atis);
      if let Some(cap) = cap {
        for i in 1..cap.len() {
          let m = cap.get(i);
          if let Some(m) = m {
            res.insert(normalize_runway_ident(m.as_str()));
          }
        }
      }
    }
  }
  res.into_iter().collect()
}

pub fn detect_departures(norm_atis: &str) -> Vec<String> {
  let mut res = HashSet::new();
  if !norm_atis.is_empty() {
    for expr in DEPARTURE_EXPR.iter() {
      let cap = expr.captures(norm_atis);
      if let Some(cap) = cap {
        for i in 1..cap.len() {
          let m = cap.get(i);
          if let Some(m) = m {
            res.insert(normalize_runway_ident(m.as_str()));
          }
        }
      }
    }
  }
  res.into_iter().collect()
}

#[cfg(test)]
mod tests {
  use crate::atis::runways::{
    detect_arrivals, detect_departures, normalize_atis_text, normalize_runway_ident,
  };

  #[test]
  fn test_normalize_runway_ident() {
    struct TC {
      src: &'static str,
      exp: &'static str,
    }

    let test_cases = vec![
      TC {
        src: "35L",
        exp: "35L",
      },
      TC {
        src: "22",
        exp: "22",
      },
      TC {
        src: "01 CENTER",
        exp: "01C",
      },
    ];

    for tc in test_cases.iter() {
      let ident = normalize_runway_ident(tc.src);
      assert_eq!(ident, tc.exp);
    }
  }

  #[test]
  fn test_detect() {
    struct TC {
      atis: &'static str,
      arrivals: Vec<&'static str>,
      departures: Vec<&'static str>,
    }

    let mut testcases = vec![
      TC {atis: "BONJOUR. THIS IS CHARLES DE GAULLE INFORMATION YANKEE RECORDED AT 1 6 4 3 U T C. LANDING RUNWAY 26 LEFT AND 27 RIGHT, TAKEOFF RUNWAY 26 RIGHT AND 27 LEFT. EXPECTED APPROACH ILS. EXPECTED DEPARTURES 5 ALPHA , 5 BRAVO , 5 ZULU. TRANSITION LEVEL 6 0. AFTER VACATING THE OUTER RUNWAY, HOLD SHORT OF THE INNER RUNWAY. BIRD ACTIVITY REPORTED. WIND 2 6 0 DEGREES, 9 KNOTS. VISIBILITY 1 0 KILOMETERS. CLOUDS SCATTERED 1800 FEET. TEMPERATURE 8, DEW POINT 5. Q N H 1 0 0 1, Q F E 0 9 8 7. CONFIRM ON FIRST CONTACT THAT YOU HAVE RECEIVED INFORMATION YANKEE.", arrivals: vec!["26L", "27R"], departures: vec!["27L", "26R"] },
      TC {atis: "HANNOVER INFORMATION A MET REPORT TIME 1720 EXPECT ILS Z APPROACH RUNWAY 27C 27L OR 27R RUNWAYS IN USE 27C 27L AND 27R TRL 70 WIND 270 DEGREES 22 KNOTS GUSTS UP TO 33 KNOTS VISIBILITY 10 KILOMETERS IN THE VICINITY SHOWER CLOUDS BROKEN 2400 FEET TEMPERATURE 7 DEW POINT 3 QNH 985 TREND NOSIG HANNOVER INFORMATION A OUT", arrivals: vec!["27C", "27L", "27R"], departures: vec!["27C", "27L", "27R"]},
      TC {atis: "HAMBURG INFORMATION E MET REPORT TIME 1720 EXPECT ILS APPROACH RUNWAY 23 RUNWAY 23 IN USE FOR LANDING AND TAKE OFF TRL 70 WHEN PASSING 2000 FEET CONTACT BREMEN RADAR ON FREQUENCY 123.925 WIND 240 DEGREES 25 KNOTS GUSTS UP TO 37 KNOTS VARIABLE BETWEEN 210 AND 270 DEGREES VISIBILITY 10 KILOMETERS LIGHT SHOWERS OF RAIN CLOUDS BROKEN CB 1800 FEET TEMPERATURE 6 DEW POINT 3 QNH 978 TREND TEMPORARY WIND 250 DEGREES 25 KNOTS GUSTS UP TO 45 KNOTS MODERATE SHOWERS OF RAIN INFORMATION E OUT", arrivals: vec!["23"], departures: vec!["23"]},
      TC {atis: "THIS IS KASTRUP AIRPORT DEPARTURE AND ARRIVAL INFO W METREPORT 1720 EXPECT ILS APPROACH VISUAL APPROACH ON REQUEST ARRIVAL RUNWAY 22L AFTER LANDING VACATE RUNWAY DEPARTURE RUNWAY 22R TRANSITION LEVEL 75 WIND 200 DEGREES 19 KNOTS VISIBILITY MORE THAN 10 KILOMETERS LIGHT RAIN SKY CONDITION OVERCAST 1400 FEET TEMPERATURE 7 DEW POINT 5 QNH 974 TEMPORARY SKY CONDITION BROKEN 800 FEET IF UNABLE TO FOLLOW SID ADVICE ON INITIAL CONTACT SQUAWKMODE CHARLIE ON PUSHBACK THIS WAS KASTRUP AIRPORT DEPARTURE AND ARRIVAL INFO W", arrivals: vec!["22L"], departures: vec!["22R"]},
    ];

    for tc in testcases.iter_mut() {
      let norm_atis = normalize_atis_text(tc.atis, true);
      let mut arrivals = detect_arrivals(&norm_atis);
      let mut departures = detect_departures(&norm_atis);

      arrivals.sort();
      departures.sort();

      tc.arrivals.sort();
      tc.departures.sort();

      assert_eq!(arrivals, tc.arrivals);
      assert_eq!(departures, tc.departures);
    }
  }
}
