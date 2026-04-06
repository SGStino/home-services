use hs_device_contracts::{CapabilityDescriptor, CapabilityKind};

pub fn command_topic(node_id: &str, device_id: &str, capability_id: &str) -> String {
    format!(
        "hs/command/{}/{}/{}",
        sanitize(node_id),
        sanitize(device_id),
        sanitize(capability_id)
    )
}

pub fn state_topic(node_id: &str, device_id: &str, capability_id: &str) -> String {
    format!(
        "hs/state/{}/{}/{}",
        sanitize(node_id),
        sanitize(device_id),
        sanitize(capability_id)
    )
}

pub fn availability_topic(node_id: &str) -> String {
    format!("hs/availability/{}", sanitize(node_id))
}

pub fn config_topic(
    discovery_prefix: &str,
    node_id: &str,
    capability: &CapabilityDescriptor,
    device_id: &str,
) -> String {
    format!(
        "{}/{}/{}/{}/config",
        discovery_prefix,
        component_name(capability),
        sanitize(node_id),
        object_id(device_id, &capability.capability_id)
    )
}

pub fn component_name(capability: &CapabilityDescriptor) -> &'static str {
    match capability.kind {
        CapabilityKind::Sensor { .. } => "sensor",
        CapabilityKind::BinarySensor { .. } => "binary_sensor",
        CapabilityKind::Switch => "switch",
        CapabilityKind::Button => "button",
        CapabilityKind::Light { .. } => "light",
        CapabilityKind::Number { .. } => "number",
        CapabilityKind::Select { .. } => "select",
        CapabilityKind::Cover => "cover",
        CapabilityKind::Climate => "climate",
    }
}

pub fn object_id(device_id: &str, capability_id: &str) -> String {
    format!("{}_{}", sanitize(device_id), sanitize(capability_id))
}

pub fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}
