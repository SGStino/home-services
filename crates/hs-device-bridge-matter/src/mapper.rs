use std::collections::{HashMap, HashSet};

use hs_device_contracts::{
    Availability, AvailabilityMessage, CapabilityDescriptor, CapabilityKind, DeviceClass,
    DeviceDescriptor, DiscoveryMessage, StateMessage,
};
use serde_json::Value;
use tracing::info;

use crate::matter_model;

const CLUSTER_ON_OFF: u64 = 6;
const CLUSTER_LEVEL_CONTROL: u64 = 8;
const CLUSTER_COLOR_CONTROL: u64 = 768;
const CLUSTER_TEMPERATURE_MEASUREMENT: u64 = 1026;
const CLUSTER_RELATIVE_HUMIDITY_MEASUREMENT: u64 = 1029;
const CLUSTER_POWER_SOURCE: u64 = 47;
const CLUSTER_ELECTRICAL_ENERGY_MEASUREMENT: u64 = 144;
const CLUSTER_ELECTRICAL_POWER_MEASUREMENT: u64 = 145;

#[derive(Clone, Debug)]
pub struct NodeSnapshot {
    pub node_id: u64,
    pub available: bool,
    pub attributes: HashMap<String, Value>,
    pub friendly_name: String,
    pub manufacturer: String,
    pub model: String,
    pub sw_version: Option<String>,
}

impl NodeSnapshot {
    pub fn from_value(value: &Value) -> Option<Self> {
        let node_id = value.get("node_id")?.as_u64()?;
        let available = value
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut attributes = HashMap::new();
        if let Some(source) = value.get("attributes").and_then(Value::as_object) {
            for (key, val) in source {
                attributes.insert(key.clone(), val.clone());
            }
        }

        let manufacturer = string_attr(&attributes, "0/40/1").unwrap_or_else(|| "Matter".to_string());
        let model = string_attr(&attributes, "0/40/3").unwrap_or_else(|| "Node".to_string());
        let friendly_name = string_attr(&attributes, "0/40/5")
            .unwrap_or_else(|| format!("Matter Node {}", node_id));
        let sw_version = string_attr(&attributes, "0/40/7");

        let mut clusters: Vec<u64> = attributes
            .keys()
            .filter_map(|path| parse_attribute_path(path).map(|(_, cluster, _)| cluster))
            .collect();
        clusters.sort_unstable();
        clusters.dedup();
        info!(node_id, ?clusters, "Matter node attribute clusters observed");

        Some(Self {
            node_id,
            available,
            attributes,
            friendly_name,
            manufacturer,
            model,
            sw_version,
        })
    }

    pub fn apply_attribute_update(&mut self, path: String, value: Value) {
        self.attributes.insert(path, value);
    }

    pub fn device_id(&self) -> String {
        format!("matter-node-{}", self.node_id)
    }

    pub fn descriptor(&self) -> DeviceDescriptor {
        DeviceDescriptor {
            service_id: "matter-server-bridge".to_string(),
            device_id: self.device_id(),
            manufacturer: self.manufacturer.clone(),
            model: self.model.clone(),
            name: self.friendly_name.clone(),
            sw_version: self.sw_version.clone(),
        }
    }

    pub fn discovery(&self) -> DiscoveryMessage {
        DiscoveryMessage {
            device: self.descriptor(),
            capabilities: self.capabilities(),
            availability_topic: None,
        }
    }

    pub fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();

        for path in self.attributes.keys() {
            if !seen.insert(path.clone()) {
                continue;
            }

            if let Some(capability) = capability_from_path(path) {
                out.push(capability);
            }
        }

        out.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        out
    }

    pub fn state_messages(&self, observed_ms: u64) -> Vec<StateMessage> {
        let mut out = Vec::new();
        for (path, value) in &self.attributes {
            if capability_from_path(path).is_none() {
                continue;
            }

            out.push(StateMessage {
                device_id: self.device_id(),
                capability_id: capability_id_from_path(path),
                value: normalize_matter_value(value.clone()),
                observed_ms,
            });
        }

        out.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        out
    }

    pub fn state_for_attribute(&self, path: &str, value: Value, observed_ms: u64) -> Option<StateMessage> {
        let _ = capability_from_path(path)?;
        Some(StateMessage {
            device_id: self.device_id(),
            capability_id: capability_id_from_path(path),
            value: normalize_matter_value(value),
            observed_ms,
        })
    }

    pub fn availability_message(&self) -> AvailabilityMessage {
        AvailabilityMessage {
            device_id: self.device_id(),
            status: if self.available {
                Availability::Online
            } else {
                Availability::Offline
            },
            detail: "derived from Matter node availability".to_string(),
        }
    }
}

fn capability_from_path(path: &str) -> Option<CapabilityDescriptor> {
    let (endpoint, cluster, attribute) = parse_attribute_path(path)?;

    if matter_model::is_vendor_specific_attribute(attribute) {
        return None;
    }

    let kind = match cluster {
        CLUSTER_ON_OFF => CapabilityKind::Switch,
        CLUSTER_LEVEL_CONTROL | CLUSTER_COLOR_CONTROL => CapabilityKind::Light {
            features: hs_device_contracts::LightFeatures::dimmable(),
        },
        CLUSTER_TEMPERATURE_MEASUREMENT => CapabilityKind::Sensor {
            device_class: Some(DeviceClass::from(hs_device_contracts::sensor_class::TEMPERATURE)),
        },
        CLUSTER_RELATIVE_HUMIDITY_MEASUREMENT => CapabilityKind::Sensor {
            device_class: Some(DeviceClass::from(hs_device_contracts::sensor_class::HUMIDITY)),
        },
        CLUSTER_POWER_SOURCE => CapabilityKind::Sensor {
            device_class: Some(DeviceClass::from(hs_device_contracts::sensor_class::BATTERY)),
        },
        CLUSTER_ELECTRICAL_ENERGY_MEASUREMENT => CapabilityKind::Sensor {
            device_class: Some(DeviceClass::from(hs_device_contracts::sensor_class::ENERGY)),
        },
        CLUSTER_ELECTRICAL_POWER_MEASUREMENT => CapabilityKind::Sensor {
            device_class: Some(DeviceClass::from(hs_device_contracts::sensor_class::POWER)),
        },
        _ => CapabilityKind::Sensor { device_class: None },
    };

    Some(CapabilityDescriptor {
        capability_id: matter_model::capability_id(endpoint, cluster, attribute),
        kind,
        friendly_name: matter_model::friendly_name(endpoint, cluster, attribute),
        unit_of_measurement: matter_model::unit_for_attribute(cluster, attribute)
            .map(str::to_string),
    })
}

fn capability_id_from_path(path: &str) -> String {
    match parse_attribute_path(path) {
        Some((endpoint, cluster, attribute)) => matter_model::capability_id(endpoint, cluster, attribute),
        None => format!("attr_{}", path.replace('/', "_")),
    }
}

fn parse_attribute_path(path: &str) -> Option<(u64, u64, u64)> {
    let mut parts = path.split('/');
    let endpoint = parts.next()?.parse().ok()?;
    let cluster = parts.next()?.parse().ok()?;
    let attribute = parts.next()?.parse().ok()?;

    Some((endpoint, cluster, attribute))
}

fn normalize_matter_value(value: Value) -> Value {
    match value {
        Value::Object(mut object) => object.remove("value").unwrap_or(Value::Object(object)),
        other => other,
    }
}

fn string_attr(attrs: &HashMap<String, Value>, path: &str) -> Option<String> {
    let value = attrs.get(path)?.as_str()?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}
