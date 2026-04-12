use std::env;

use hs_eventbus_mqtt_ha::HomeAssistantMqttConfig;

#[derive(Clone)]
pub struct ServiceConfig {
    pub device: DeviceConfig,
    pub tasmota: TasmotaConfig,
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
pub struct TasmotaConfig {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
    pub username: Option<String>,
    pub password: Option<String>,
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
            tasmota: TasmotaConfig::from_env(),
            ha: HomeAssistantMqttConfig::from_env(now_unix_ms),
        }
    }
}

impl TasmotaConfig {
    pub fn from_env() -> Self {
        let host = env_or_default("TASMOTA_HOST", "127.0.0.1");
        let use_tls = env_flag("TASMOTA_USE_TLS");

        let default_port = if use_tls { 443 } else { 80 };
        let port = env::var("TASMOTA_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(default_port);

        let request_timeout_ms = env::var("TASMOTA_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(3_000);

        Self {
            host,
            port,
            use_tls,
            username: env_optional("TASMOTA_USERNAME"),
            password: env_optional("TASMOTA_PASSWORD"),
            request_timeout_ms,
        }
    }

    pub fn base_url(&self) -> String {
        let scheme = if self.use_tls { "https" } else { "http" };
        format!("{scheme}://{}:{}", self.host, self.port)
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

fn env_flag(key: &str) -> bool {
    env::var(key)
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "on"
        })
        .unwrap_or(false)
}
