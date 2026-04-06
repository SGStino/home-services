use hs_device_contracts::{
    sensor_class, CapabilityDescriptor, CapabilityKind, DeviceClass, DeviceDescriptor,
};

pub fn demo_device() -> DeviceDescriptor {
    DeviceDescriptor {
        service_id: "device-demo-living-room-node".to_string(),
        device_id: "living-room-node-01".to_string(),
        manufacturer: "Home Services".to_string(),
        model: "demo-sensor-switch-button".to_string(),
        name: "Living Room Demo Node".to_string(),
        sw_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

pub fn demo_capabilities() -> Vec<CapabilityDescriptor> {
    vec![
        CapabilityDescriptor {
            capability_id: "temperature".to_string(),
            kind: CapabilityKind::Sensor {
                device_class: Some(DeviceClass::from(sensor_class::TEMPERATURE)),
            },
            friendly_name: "Temperature".to_string(),
            unit_of_measurement: Some("°C".to_string()),
        },
        CapabilityDescriptor {
            capability_id: "power".to_string(),
            kind: CapabilityKind::Switch,
            friendly_name: "Power Switch".to_string(),
            unit_of_measurement: None,
        },
        CapabilityDescriptor {
            capability_id: "shutdown".to_string(),
            kind: CapabilityKind::Button,
            friendly_name: "Shutdown Button".to_string(),
            unit_of_measurement: None,
        },
    ]
}
