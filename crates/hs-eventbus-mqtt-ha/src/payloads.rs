use hs_contracts::{
    Availability, CapabilityDescriptor, CapabilityKind, DiscoveryMessage, StateMessage,
};
use serde_json::{json, Value};

use crate::topics::{availability_topic, command_topic, state_topic};

pub fn discovery_payload(
    discovery: &DiscoveryMessage,
    capability: &CapabilityDescriptor,
    node_id: &str,
) -> Value {
    let device = &discovery.device;
    let mut payload = json!({
        "name": capability.friendly_name,
        "unique_id": format!(
            "{}_{}_{}",
            node_id,
            device.device_id,
            capability.capability_id
        ),
        "state_topic": state_topic(node_id, &device.device_id, &capability.capability_id),
        "availability_topic": availability_topic(node_id),
        "payload_available": "online",
        "payload_not_available": "offline",
        "device": {
            "identifiers": [format!("hs_{}", device.device_id)],
            "name": device.name,
            "manufacturer": device.manufacturer,
            "model": device.model,
            "sw_version": device.sw_version,
        }
    });

    if let CapabilityKind::Sensor { device_class } = &capability.kind {
        if let Some(dc) = device_class {
            payload["device_class"] = json!(dc.as_str());
        }
    }

    if let CapabilityKind::BinarySensor { device_class } = &capability.kind {
        payload["payload_on"] = json!("true");
        payload["payload_off"] = json!("false");
        if let Some(dc) = device_class {
            payload["device_class"] = json!(dc.as_str());
        }
    }

    if let Some(unit) = &capability.unit_of_measurement {
        payload["unit_of_measurement"] = json!(unit);
    }

    match &capability.kind {
        CapabilityKind::Switch => {
            payload["command_topic"] = json!(command_topic(
                node_id,
                &device.device_id,
                &capability.capability_id
            ));
            payload["payload_on"] = json!("ON");
            payload["payload_off"] = json!("OFF");
        }
        CapabilityKind::Button => {
            payload["command_topic"] = json!(command_topic(
                node_id,
                &device.device_id,
                &capability.capability_id
            ));
            payload["payload_press"] = json!("PRESS");
            // Stateless button does not publish state_topic in HA model.
            payload
                .as_object_mut()
                .expect("json object")
                .remove("state_topic");
        }
        _ => {}
    }

    payload
}

pub fn availability_payload(status: &Availability) -> &'static str {
    match status {
        Availability::Online => "online",
        Availability::Offline => "offline",
        Availability::Degraded => "degraded",
    }
}

pub fn state_payload(state: &StateMessage) -> String {
    match &state.value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}
