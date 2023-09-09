use std::fmt::Display;

#[derive(Debug)]
pub struct GeonamesParseError {
  pub msg: &'static str,
}

impl Display for GeonamesParseError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "error converting geojson feature: {}", self.msg)
  }
}
impl std::error::Error for GeonamesParseError {}
