use std::env;

#[derive(Clone, Debug)]
pub struct SparkplugBConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id: String,
    pub group_id: String,
    pub edge_node_id: String,
}

impl Default for SparkplugBConfig {
    fn default() -> Self {
        Self {
            broker_host: "127.0.0.1".to_string(),
            broker_port: 1883,
            client_id: "hs-sparkplug".to_string(),
            group_id: "home-services".to_string(),
            edge_node_id: "hs-node-dev".to_string(),
        }
    }
}

impl SparkplugBConfig {
    pub fn from_env(now_unix_ms: u64) -> Self {
        let broker_host = env::var("MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let group_id = env::var("SPARKPLUG_GROUP_ID").unwrap_or_else(|_| "home-services".to_string());
        let edge_node_id =
            env::var("SPARKPLUG_EDGE_NODE_ID").unwrap_or_else(|_| "hs-node-dev".to_string());
        let client_id = env::var("MQTT_CLIENT_ID")
            .unwrap_or_else(|_| format!("hs-sparkplug-{}-{}", edge_node_id, now_unix_ms));

        Self {
            broker_host,
            broker_port: 1883,
            client_id,
            group_id,
            edge_node_id,
        }
    }
}
