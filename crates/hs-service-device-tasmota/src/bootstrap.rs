use hs_device_contracts::{sensor_class, CapabilityDescriptor, CapabilityKind, DeviceClass};

pub fn tasmota_capabilities() -> Vec<CapabilityDescriptor> {
    vec![
        CapabilityDescriptor {
            capability_id: "power".to_string(),
            kind: CapabilityKind::Switch,
            friendly_name: "Power".to_string(),
            unit_of_measurement: None,
        },
        CapabilityDescriptor {
            capability_id: "power_w".to_string(),
            kind: CapabilityKind::Sensor {
                device_class: Some(DeviceClass::from(sensor_class::POWER)),
            },
            friendly_name: "Power".to_string(),
            unit_of_measurement: Some("W".to_string()),
        },
        CapabilityDescriptor {
            capability_id: "voltage_v".to_string(),
            kind: CapabilityKind::Sensor {
                device_class: Some(DeviceClass::from(sensor_class::VOLTAGE)),
            },
            friendly_name: "Voltage".to_string(),
            unit_of_measurement: Some("V".to_string()),
        },
        CapabilityDescriptor {
            capability_id: "current_a".to_string(),
            kind: CapabilityKind::Sensor {
                device_class: Some(DeviceClass::from(sensor_class::CURRENT)),
            },
            friendly_name: "Current".to_string(),
            unit_of_measurement: Some("A".to_string()),
        },
        CapabilityDescriptor {
            capability_id: "energy_total_kwh".to_string(),
            kind: CapabilityKind::Sensor {
                device_class: Some(DeviceClass::from(sensor_class::ENERGY)),
            },
            friendly_name: "Energy Total".to_string(),
            unit_of_measurement: Some("kWh".to_string()),
        },
    ]
}
