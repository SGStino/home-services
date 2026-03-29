use std::env;

use hs_contracts::DeviceDescriptor;

#[derive(Clone, Debug)]
pub struct HomeAssistantMqttConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id: String,
    pub discovery_prefix: String,
    pub node_id: String,
    pub lwt_device_id: String,
}

impl Default for HomeAssistantMqttConfig {
    fn default() -> Self {
        Self {
            broker_host: "127.0.0.1".to_string(),
            broker_port: 1883,
            client_id: "hs-device-demo".to_string(),
            discovery_prefix: "homeassistant".to_string(),
            node_id: "hs-node-dev".to_string(),
            lwt_device_id: "device-demo".to_string(),
        }
    }
}

impl HomeAssistantMqttConfig {
    pub fn from_env_for_device(device: &DeviceDescriptor, now_unix_ms: u64) -> Self {
        let broker_host = env::var("MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let node_id = env::var("MQTT_NODE_ID").unwrap_or_else(|_| "hs-node-dev".to_string());

        Self {
            broker_host,
            broker_port: 1883,
            client_id: format!("{}-{}", device.service_id, now_unix_ms),
            discovery_prefix: "homeassistant".to_string(),
            node_id,
            lwt_device_id: device.device_id.clone(),
        }
    }
}
