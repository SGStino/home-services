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
    let state_topic = state_topic(node_id, &device.device_id, &capability.capability_id);
    let mut payload = json!({
        "name": capability.friendly_name,
        "unique_id": format!(
            "{}_{}_{}",
            node_id,
            device.device_id,
            capability.capability_id
        ),
        "state_topic": state_topic,
        "availability_topic": availability_topic(node_id),
        "payload_available": "online",
        "payload_not_available": "offline",
        "value_template": "{{ value_json.value }}",
        "json_attributes_topic": state_topic,
        "json_attributes_template": "{{ {'ts': value_json.ts} | tojson }}",
        "device": {
            "identifiers": [format!("hs_{}", device.device_id)],
            "name": device.name,
            "manufacturer": device.manufacturer,
            "model": device.model,
            "sw_version": device.sw_version,
        }
    });

    if let CapabilityKind::Sensor { device_class: Some(dc) } = &capability.kind {
        payload["device_class"] = json!(dc.as_str());
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
            let payload = payload.as_object_mut().expect("json object");
            // Stateless button does not publish state or timestamp metadata in HA.
            payload.remove("state_topic");
            payload.remove("value_template");
            payload.remove("json_attributes_topic");
            payload.remove("json_attributes_template");
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
    json!({
        "value": state.value,
        "ts": state.observed_ms,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{discovery_payload, state_payload};
    use hs_contracts::{
        sensor_class, CapabilityDescriptor, CapabilityKind, DeviceClass, DeviceDescriptor,
        DiscoveryMessage, StateMessage,
    };
    use serde_json::json;

    #[test]
    fn state_payload_wraps_value_with_timestamp() {
        let payload = state_payload(&StateMessage {
            device_id: "living-room-node-01".to_string(),
            capability_id: "temperature".to_string(),
            value: json!(21.5),
            observed_ms: 1_742_230_400_123,
        });

        assert_eq!(
            payload,
            json!({
                "value": 21.5,
                "ts": 1_742_230_400_123u64,
            })
            .to_string()
        );
    }

    #[test]
    fn discovery_payload_extracts_value_and_exposes_timestamp_attributes() {
        let discovery = DiscoveryMessage {
            device: DeviceDescriptor {
                service_id: "device-demo-living-room-node".to_string(),
                device_id: "living-room-node-01".to_string(),
                manufacturer: "Home Services".to_string(),
                model: "demo-sensor-switch-button".to_string(),
                name: "Living Room Demo Node".to_string(),
                sw_version: Some("0.1.0".to_string()),
            },
            capabilities: Vec::new(),
        };
        let capability = CapabilityDescriptor {
            capability_id: "temperature".to_string(),
            kind: CapabilityKind::Sensor {
                device_class: Some(DeviceClass::from(sensor_class::TEMPERATURE)),
            },
            friendly_name: "Temperature".to_string(),
            unit_of_measurement: Some("°C".to_string()),
        };

        let payload = discovery_payload(&discovery, &capability, "hs-node-dev");

        assert_eq!(payload["value_template"], "{{ value_json.value }}");
        assert_eq!(
            payload["json_attributes_topic"],
            "hs/state/hs_node_dev/living_room_node_01/temperature"
        );
        assert_eq!(
            payload["json_attributes_template"],
            "{{ {'ts': value_json.ts} | tojson }}"
        );
    }

    #[test]
    fn button_discovery_removes_state_templates() {
        let discovery = DiscoveryMessage {
            device: DeviceDescriptor {
                service_id: "device-demo-living-room-node".to_string(),
                device_id: "living-room-node-01".to_string(),
                manufacturer: "Home Services".to_string(),
                model: "demo-sensor-switch-button".to_string(),
                name: "Living Room Demo Node".to_string(),
                sw_version: Some("0.1.0".to_string()),
            },
            capabilities: Vec::new(),
        };
        let capability = CapabilityDescriptor {
            capability_id: "shutdown".to_string(),
            kind: CapabilityKind::Button,
            friendly_name: "Shutdown Button".to_string(),
            unit_of_measurement: None,
        };

        let payload = discovery_payload(&discovery, &capability, "hs-node-dev");

        assert!(payload.get("state_topic").is_none());
        assert!(payload.get("value_template").is_none());
        assert!(payload.get("json_attributes_topic").is_none());
        assert!(payload.get("json_attributes_template").is_none());
    }
}
