use crate::{service::camden, util::seconds_since};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, fmt::Display};

#[macro_export]
macro_rules! labels {
  ($($label:literal = $value:expr),+) => {
    {
      let mut c: HashMap<&'static str, String> = HashMap::new();
      $(c.insert(($label).into(), ($value).into());)+
      c
    }
  };
}

#[derive(Debug, Clone)]
pub enum MetricType {
  Counter,
  Gauge,
  Summary,
  Histogram,
}

impl From<MetricType> for camden::MetricType {
  fn from(value: MetricType) -> Self {
    match value {
      MetricType::Counter => camden::MetricType::Counter,
      MetricType::Gauge => camden::MetricType::Gauge,
      MetricType::Summary => camden::MetricType::Summary,
      MetricType::Histogram => camden::MetricType::Histogram,
    }
  }
}

impl Display for MetricType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      MetricType::Counter => write!(f, "counter"),
      MetricType::Gauge => write!(f, "gauge"),
      MetricType::Summary => write!(f, "summary"),
      MetricType::Histogram => write!(f, "histogram"),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Metric<T: Display + Clone + Default> {
  name: String,
  help: String,
  metric_type: MetricType,
  single: bool,
  values: HashMap<String, T>,
}

impl<T: Display + Clone + Default> Metric<T> {
  pub fn new(name: &str, help: &str, mtype: MetricType) -> Self {
    Self {
      name: name.into(),
      help: help.into(),
      metric_type: mtype,
      single: false,
      values: HashMap::new(),
    }
  }

  pub fn reset(&mut self) {
    self.values.clear();
  }

  pub fn set(&mut self, labels: HashMap<&'static str, String>, value: T) {
    self.single = false;
    let mut labels = labels
      .iter()
      .map(|(k, v)| format!("{}=\"{}\"", k, v))
      .collect::<Vec<String>>();
    labels.sort();
    let label_str = labels.join(",");
    self.values.insert(label_str, value);
  }

  pub fn set_single(&mut self, value: T) {
    self.reset();
    self.single = true;
    self.values.insert("_".into(), value);
  }

  pub fn render(&self) -> String {
    if self.values.is_empty() {
      return "".into();
    }

    let comment = format!(
      "# HELP {} {}\n# TYPE {} {}\n",
      self.name, self.help, self.name, self.metric_type
    );

    if self.single {
      let value = self.values.get("_").cloned().unwrap_or_default();
      comment + &format!("{} {}", self.name, value) + "\n"
    } else {
      let values = self
        .values
        .iter()
        .map(|(k, v)| format!("{}{{{}}} {}", self.name, k, v))
        .collect::<Vec<String>>()
        .join("\n");
      comment + &values + "\n"
    }
  }
}

impl From<Metric<u64>> for camden::Metric {
  fn from(value: Metric<u64>) -> Self {
    Self {
      name: value.name,
      help: value.help,
      metric_type: value.metric_type as i32,
      single: value.single,
      is_float: false,
      float_values: Default::default(),
      int_values: value.values,
    }
  }
}

impl From<Metric<usize>> for camden::Metric {
  fn from(value: Metric<usize>) -> Self {
    Self {
      name: value.name,
      help: value.help,
      metric_type: value.metric_type as i32,
      single: value.single,
      is_float: false,
      float_values: Default::default(),
      int_values: value
        .values
        .into_iter()
        .map(|(k, v)| (k, v as u64))
        .collect(),
    }
  }
}

impl From<Metric<f64>> for camden::Metric {
  fn from(value: Metric<f64>) -> Self {
    Self {
      name: value.name,
      help: value.help,
      metric_type: value.metric_type as i32,
      single: value.single,
      is_float: true,
      float_values: value.values,
      int_values: Default::default(),
    }
  }
}

impl From<Metric<f32>> for camden::Metric {
  fn from(value: Metric<f32>) -> Self {
    Self {
      name: value.name,
      help: value.help,
      metric_type: value.metric_type as i32,
      single: value.single,
      is_float: true,
      float_values: value
        .values
        .into_iter()
        .map(|(k, v)| (k, v as f64))
        .collect(),
      int_values: Default::default(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Metrics {
  pub vatsim_objects_online: Metric<usize>,
  pub database_objects_count: Metric<u64>,
  pub database_objects_count_fetch_time_sec: Metric<f32>,
  pub vatsim_data_timestamp: i64,
  pub vatsim_data_load_time_sec: Metric<f32>,
  pub processing_time_sec: Metric<f32>,
  pub db_cleanup_time_sec: Metric<f32>,
  pub process_started_at: DateTime<Utc>,
}

impl Metrics {
  pub fn new() -> Self {
    Self {
      vatsim_objects_online: Metric::new(
        "vatsim_objects_online",
        "Vatsim objects currently tracked",
        MetricType::Gauge,
      ),
      database_objects_count: Metric::new(
        "database_objects_count",
        "Number of objects stored in database",
        MetricType::Gauge,
      ),
      database_objects_count_fetch_time_sec: Metric::new(
        "database_objects_count_fetch_time_sec",
        "Time spent fetching countDocuments()",
        MetricType::Gauge,
      ),
      vatsim_data_timestamp: 0,
      vatsim_data_load_time_sec: Metric::new(
        "vatsim_data_load_time_sec",
        "Vatsim API data load time",
        MetricType::Gauge,
      ),
      processing_time_sec: Metric::new(
        "processing_time_sec",
        "Processing time for various vatsim objects",
        MetricType::Gauge,
      ),
      db_cleanup_time_sec: Metric::new(
        "db_cleanup_time_sec",
        "Time spent cleaning up database stored objects",
        MetricType::Gauge,
      ),
      process_started_at: Utc::now(),
    }
  }

  pub fn render(&self) -> String {
    let t = Utc::now().timestamp();
    let mut metrics = vec![];

    metrics.push(self.vatsim_objects_online.render());
    metrics.push(self.database_objects_count.render());
    metrics.push(self.database_objects_count_fetch_time_sec.render());

    let age = t - self.vatsim_data_timestamp;
    let mut metric = Metric::new(
      "vatsim_data_age_sec",
      "Latest Vatsim data age in seconds",
      MetricType::Gauge,
    );
    metric.set_single(age);
    metrics.push(metric.render());

    metrics.push(self.vatsim_data_load_time_sec.render());
    metrics.push(self.db_cleanup_time_sec.render());

    let mut metric = Metric::new("uptime", "Process uptime in sec", MetricType::Counter);
    let sec = seconds_since(self.process_started_at).ceil() as u64;
    metric.set_single(sec);
    metrics.push(metric.render());

    metrics.join("")
  }
}

impl Default for Metrics {
  fn default() -> Self {
    Self::new()
  }
}

impl From<Metrics> for camden::MetricSet {
  fn from(value: Metrics) -> Self {
    Self {
      vatsim_objects_online: Some(value.vatsim_objects_online.into()),
      database_objects_count: Some(value.database_objects_count.into()),
      database_objects_count_fetch_time_sec: Some(
        value.database_objects_count_fetch_time_sec.into(),
      ),
      vatsim_data_load_time_sec: Some(value.vatsim_data_load_time_sec.into()),
      processing_time_sec: Some(value.processing_time_sec.into()),
      db_cleanup_time_sec: Some(value.db_cleanup_time_sec.into()),
      vatsim_data_timestamp: value.vatsim_data_timestamp as u64,
      process_started_at: value.process_started_at.timestamp_millis() as u64,
    }
  }
}
