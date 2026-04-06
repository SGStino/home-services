use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub enum DataPointField {
    Number(f64),
    Bool(bool),
    Text(String),
}

#[derive(Clone, Debug)]
pub struct DataPoint {
    pub measurement: String,
    pub tags: BTreeMap<String, String>,
    pub fields: BTreeMap<String, DataPointField>,
    pub observed_ms: u64,
}

pub type DataPointList = Vec<DataPoint>;
