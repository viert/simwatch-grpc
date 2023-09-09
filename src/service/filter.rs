use crate::{
  lee::parser::{
    condition::{Condition, Value},
    error::CompileError,
    expression::EvaluateFunc,
  },
  moving::pilot::Pilot,
};
use lazy_static::lazy_static;

lazy_static! {
  static ref ALLOWED_FIELDS: &'static [&'static str] = &[
    "callsign",
    "name",
    "alt",
    "gs",
    "lat",
    "lng",
    "aircraft",
    "arrival",
    "departure",
    "rules",
  ];
}

// Compilation callback
// TODO: add checks for supported condition identifiers
pub fn compile_filter(cond: Condition) -> Result<Box<EvaluateFunc<Pilot>>, CompileError> {
  let ident = cond.ident.clone();
  let value = cond.value.clone();
  let operator = cond.operator.clone();

  let evalfunc: Box<EvaluateFunc<Pilot>> = match ident.as_str() {
    "rules" => {
      let norm_value = match value {
        Value::String(v) => {
          let v = v.to_lowercase();
          match v.as_str() {
            "i" | "ifr" => "I",
            "v" | "vfr" => "V",
            _ => {
              return Err(CompileError {
                msg: "invalid rules value, valid ones are ['v', 'i', 'vfr', 'ifr']".into(),
              })
            }
          }
        }
        _ => {
          return Err(CompileError {
            msg: format!("invalid rules value type {}", value.value_type()),
          });
        }
      };
      let norm_value = Value::String(norm_value.to_owned());
      Box::new(move |pilot| {
        pilot
          .flight_plan
          .as_ref()
          .map(|fp| norm_value.eval_str(&fp.flight_rules, operator.clone()))
          .unwrap_or(false)
      })
    }
    "callsign" => Box::new(move |pilot| value.eval_str(&pilot.callsign, operator.clone())),
    "name" => Box::new(move |pilot| value.eval_str(&pilot.name, operator.clone())),
    "alt" => Box::new(move |pilot| value.eval_i64(pilot.altitude as i64, operator.clone())),
    "gs" => Box::new(move |pilot| value.eval_i64(pilot.groundspeed as i64, operator.clone())),
    "lat" => Box::new(move |pilot| value.eval_f64(pilot.position.lat, operator.clone())),
    "lng" => Box::new(move |pilot| value.eval_f64(pilot.position.lng, operator.clone())),
    "aircraft" => Box::new(move |pilot| {
      pilot
        .flight_plan
        .as_ref()
        .map(|fp| value.eval_str(&fp.aircraft, operator.clone()))
        .unwrap_or(false)
    }),
    "arrival" => Box::new(move |pilot| {
      pilot
        .flight_plan
        .as_ref()
        .map(|fp| value.eval_str(&fp.arrival, operator.clone()))
        .unwrap_or(false)
    }),
    "departure" => Box::new(move |pilot| {
      pilot
        .flight_plan
        .as_ref()
        .map(|fp| value.eval_str(&fp.departure, operator.clone()))
        .unwrap_or(false)
    }),
    _ => {
      return Err(CompileError {
        msg: format!(
          "{} is not a valid field to query, valid fields are: [{}]",
          cond.ident,
          ALLOWED_FIELDS.join(", ")
        ),
      })
    }
  };
  Ok(evalfunc)
}
