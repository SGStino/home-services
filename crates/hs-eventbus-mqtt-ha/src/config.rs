use std::env;

#[derive(Clone, Debug)]
pub struct HomeAssistantMqttConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id: String,
    pub availability_session: String,
    pub availability_message_expiry_secs: Option<u32>,
    pub discovery_prefix: String,
    pub node_id: String,
}

impl Default for HomeAssistantMqttConfig {
    fn default() -> Self {
        Self {
            broker_host: "127.0.0.1".to_string(),
            broker_port: 1883,
            client_id: "hs-device-demo".to_string(),
            availability_session: "hs-device-demo".to_string(),
            availability_message_expiry_secs: None,
            discovery_prefix: "homeassistant".to_string(),
            node_id: "hs-node-dev".to_string(),
        }
    }
}

impl HomeAssistantMqttConfig {
    pub fn from_env(now_unix_ms: u64) -> Self {
        let broker_host = env::var("MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let node_id = env::var("MQTT_NODE_ID").unwrap_or_else(|_| "hs-node-dev".to_string());
        let client_id = env::var("MQTT_CLIENT_ID")
            .unwrap_or_else(|_| format!("hs-adapter-{}-{}", node_id, now_unix_ms));
        let availability_session = env::var("MQTT_AVAILABILITY_SESSION")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| client_id.clone());
        let availability_message_expiry_secs = env::var("MQTT_AVAILABILITY_EXPIRY_SECS")
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|value| *value > 0);

        Self {
            broker_host,
            broker_port: 1883,
            client_id,
            availability_session,
            availability_message_expiry_secs,
            discovery_prefix: "homeassistant".to_string(),
            node_id,
        }
    }
}
