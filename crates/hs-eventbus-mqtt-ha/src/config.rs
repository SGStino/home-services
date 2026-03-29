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
