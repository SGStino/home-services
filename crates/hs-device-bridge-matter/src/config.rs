use std::env;

#[derive(Clone, Debug)]
pub struct BridgeConfig {
    pub matter_ws_url: String,
    pub matter_tls_ca_cert_path: Option<String>,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            matter_ws_url: "ws://127.0.0.1:5580/ws".to_string(),
            matter_tls_ca_cert_path: None,
        }
    }
}

impl BridgeConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(value) = env::var("MATTER_WS_URL") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                config.matter_ws_url = trimmed.to_string();
            }
        }

        if let Ok(value) = env::var("MATTER_TLS_CA_CERT_PATH") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                config.matter_tls_ca_cert_path = Some(trimmed.to_string());
            }
        }

        config
    }
}
