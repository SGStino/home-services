use hs_device_contracts::{
    Availability, CapabilityDescriptor, CapabilityKind, DiscoveryMessage, StateMessage,
};
use prost::Message;
use serde_json::Value;

use crate::sparkplug::{payload, DataType, Payload};

pub fn discovery_payload(discovery: &DiscoveryMessage, seq: u64, now_ms: u64) -> Vec<u8> {
    let metrics: Vec<payload::Metric> = discovery
        .capabilities
        .iter()
        .map(|capability| payload::Metric {
            name: Some(capability.capability_id.clone()),
            alias: None,
            datatype: Some(datatype_for_capability(capability) as u32),
            timestamp: Some(now_ms),
            is_null: Some(true),
            value: None,
            ..Default::default()
        })
        .collect();

    Payload {
        timestamp: Some(now_ms),
        metrics,
        seq: Some(seq),
        uuid: None,
        body: None,
    }
    .encode_to_vec()
}

pub fn state_payload(state: &StateMessage, seq: u64) -> Vec<u8> {
    let metric = payload::Metric {
        name: Some(state.capability_id.clone()),
        alias: None,
        datatype: Some(datatype_for_value(&state.value) as u32),
        timestamp: Some(state.observed_ms),
        is_null: Some(false),
        value: metric_value_from_json(&state.value),
        ..Default::default()
    };

    Payload {
        timestamp: Some(state.observed_ms),
        metrics: vec![metric],
        seq: Some(seq),
        uuid: None,
        body: None,
    }
    .encode_to_vec()
}

pub fn availability_payload(status: &Availability) -> &'static str {
    match status {
        Availability::Online => "ONLINE",
        Availability::Offline => "OFFLINE",
        Availability::Degraded => "DEGRADED",
    }
}

pub fn datatype_for_capability(capability: &CapabilityDescriptor) -> DataType {
    match capability.kind {
        CapabilityKind::BinarySensor { .. } | CapabilityKind::Switch => DataType::Boolean,
        CapabilityKind::Button => DataType::String,
        CapabilityKind::Sensor { .. }
        | CapabilityKind::Number { .. }
        | CapabilityKind::Light { .. }
        | CapabilityKind::Select { .. }
        | CapabilityKind::Cover
        | CapabilityKind::Climate => DataType::Double,
    }
}

pub fn datatype_for_value(value: &Value) -> DataType {
    if value.is_boolean() {
        return DataType::Boolean;
    }
    if value.is_number() {
        return DataType::Double;
    }

    DataType::String
}

pub fn decode_payload(bytes: &[u8]) -> Option<Payload> {
    Payload::decode(bytes).ok()
}

/// Build a Sparkplug NBIRTH payload for the edge node.
/// Declares `Node Control/Rebirth` as a controllable metric (initial value false per spec).
pub fn nbirth_payload(now_ms: u64) -> Vec<u8> {
    Payload {
        timestamp: Some(now_ms),
        metrics: vec![payload::Metric {
            name: Some("Node Control/Rebirth".to_string()),
            datatype: Some(DataType::Boolean as u32),
            value: Some(payload::metric::Value::BooleanValue(false)),
            ..Default::default()
        }],
        seq: Some(0),
        uuid: None,
        body: None,
    }
    .encode_to_vec()
}

pub fn metric_value_to_json(metric: &payload::Metric) -> Option<Value> {
    match &metric.value {
        Some(payload::metric::Value::BooleanValue(v)) => Some(Value::Bool(*v)),
        Some(payload::metric::Value::StringValue(v)) => Some(Value::String(v.clone())),
        Some(payload::metric::Value::IntValue(v)) => Some(Value::Number((*v).into())),
        Some(payload::metric::Value::LongValue(v)) => Some(Value::Number((*v).into())),
        Some(payload::metric::Value::FloatValue(v)) => {
            serde_json::Number::from_f64(*v as f64).map(Value::Number)
        }
        Some(payload::metric::Value::DoubleValue(v)) => {
            serde_json::Number::from_f64(*v).map(Value::Number)
        }
        Some(payload::metric::Value::BytesValue(v)) => Some(Value::Array(
            v.iter().map(|b| Value::Number((*b as u64).into())).collect(),
        )),
        // Dataset, Template, and Extension are complex types; surface as null for now.
        Some(payload::metric::Value::DatasetValue(_))
        | Some(payload::metric::Value::TemplateValue(_))
        | Some(payload::metric::Value::ExtensionValue(_)) => None,
        None => None,
    }
}

fn metric_value_from_json(value: &Value) -> Option<payload::metric::Value> {
    match value {
        Value::Bool(v) => Some(payload::metric::Value::BooleanValue(*v)),
        Value::Number(v) => {
            if let Some(as_u64) = v.as_u64() {
                Some(payload::metric::Value::LongValue(as_u64))
            } else {
                v.as_f64().map(payload::metric::Value::DoubleValue)
            }
        }
        Value::String(v) => Some(payload::metric::Value::StringValue(v.clone())),
        _ => Some(payload::metric::Value::StringValue(value.to_string())),
    }
}
