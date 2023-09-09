use regex::Regex;
use std::{fmt::Display, str::FromStr};

#[derive(Debug, Clone)]
pub enum Operator {
  Matches,
  NotMatches,
  Equals,
  NotEquals,
  Less,
  LessOrEqual,
  Greater,
  GreaterOrEqual,
}

impl Operator {
  pub fn literal(&self) -> &'static str {
    match self {
      Operator::Matches => "=~",
      Operator::NotMatches => "!~",
      Operator::Equals => "==",
      Operator::NotEquals => "!=",
      Operator::Less => "<",
      Operator::LessOrEqual => "<=",
      Operator::Greater => ">",
      Operator::GreaterOrEqual => ">=",
    }
  }
}

#[derive(Clone)]
pub enum Value {
  Integer(i64),
  Float(f64),
  String(String),
}

impl Value {
  pub fn value_type(&self) -> &'static str {
    match self {
      Value::Integer(_) => "integer",
      Value::Float(_) => "float",
      Value::String(_) => "string",
    }
  }

  pub fn as_string(&self) -> String {
    match self {
      Value::Integer(v) => format!("int({})", v),
      Value::Float(v) => format!("float({})", v),
      Value::String(v) => format!("string({})", v),
    }
  }

  pub fn eval_i64(&self, ext_val: i64, operator: Operator) -> bool {
    match *self {
      Value::Integer(v) => match operator {
        Operator::Equals => ext_val == v,
        Operator::NotEquals => ext_val != v,
        Operator::Less => ext_val < v,
        Operator::LessOrEqual => ext_val <= v,
        Operator::Greater => ext_val > v,
        Operator::GreaterOrEqual => ext_val >= v,
        _ => false,
      },
      Value::Float(v) => {
        let ext_val = ext_val as f64;
        match operator {
          Operator::Equals => ext_val == v,
          Operator::NotEquals => ext_val != v,
          Operator::Less => ext_val < v,
          Operator::LessOrEqual => ext_val <= v,
          Operator::Greater => ext_val > v,
          Operator::GreaterOrEqual => ext_val >= v,
          _ => false,
        }
      }
      Value::String(_) => false,
    }
  }

  pub fn eval_f64(&self, ext_val: f64, operator: Operator) -> bool {
    match *self {
      Value::Integer(v) => {
        let v = v as f64;
        match operator {
          Operator::Equals => ext_val == v,
          Operator::NotEquals => ext_val != v,
          Operator::Less => ext_val < v,
          Operator::LessOrEqual => ext_val <= v,
          Operator::Greater => ext_val > v,
          Operator::GreaterOrEqual => ext_val >= v,
          _ => false,
        }
      }
      Value::Float(v) => match operator {
        Operator::Equals => ext_val == v,
        Operator::NotEquals => ext_val != v,
        Operator::Less => ext_val < v,
        Operator::LessOrEqual => ext_val <= v,
        Operator::Greater => ext_val > v,
        Operator::GreaterOrEqual => ext_val >= v,
        _ => false,
      },
      Value::String(_) => false,
    }
  }

  pub fn eval_str(&self, ext_val: &str, operator: Operator) -> bool {
    match self {
      Value::Integer(_) => false,
      Value::Float(_) => false,
      Value::String(v) => match operator {
        Operator::Matches => {
          let re = Regex::from_str(v);
          if let Ok(re) = re {
            re.is_match(ext_val)
          } else {
            false
          }
        }
        Operator::NotMatches => {
          // TODO: evaluation errors
          let re = Regex::from_str(v);
          if let Ok(re) = re {
            !re.is_match(ext_val)
          } else {
            true
          }
        }
        Operator::Equals => ext_val == v,
        Operator::NotEquals => ext_val != v,
        _ => false,
      },
    }
  }
}

#[derive(Clone)]
pub struct Condition {
  pub ident: String,
  pub operator: Operator,
  pub value: Value,
}

impl Display for Condition {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Condition<({} {} {})>",
      self.ident,
      self.operator.literal(),
      self.value.as_string()
    )
  }
}
