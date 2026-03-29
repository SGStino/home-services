use std::env;

use hs_eventbus_mqtt_ha::HomeAssistantMqttConfig;

#[derive(Clone)]
pub struct ServiceConfig {
    pub device: DeviceConfig,
    pub hs110: Hs110Config,
    pub ha: HomeAssistantMqttConfig,
}

#[derive(Clone)]
pub struct DeviceConfig {
    pub service_id: Option<String>,
    pub device_id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone)]
pub struct Hs110Config {
    pub host: String,
    pub port: u16,
    pub request_timeout_ms: u64,
}

impl ServiceConfig {
    pub fn from_env(now_unix_ms: u64) -> Self {
        let device = DeviceConfig {
            service_id: env_optional("HS_SERVICE_ID"),
            device_id: env_optional("HS_DEVICE_ID"),
            manufacturer: env_optional("HS_DEVICE_MANUFACTURER"),
            model: env_optional("HS_DEVICE_MODEL"),
            name: env_optional("HS_DEVICE_NAME"),
        };

        Self {
            device,
            hs110: Hs110Config::from_env(),
            ha: HomeAssistantMqttConfig::from_env(now_unix_ms),
        }
    }
}

impl Hs110Config {
    pub fn from_env() -> Self {
        let host = env_or_default("HS110_HOST", "127.0.0.1");
        let port = env::var("HS110_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(9999);

        let request_timeout_ms = env::var("HS110_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(2_000);

        Self {
            host,
            port,
            request_timeout_ms,
        }
    }
}

fn env_or_default(key: &str, default_value: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_value.to_string())
}

fn env_optional(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
