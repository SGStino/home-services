use std::env;

use hs_contracts::DeviceDescriptor;
use hs_eventbus_mqtt_ha::HomeAssistantMqttConfig;

#[derive(Clone)]
pub struct ServiceConfig {
    pub device: DeviceDescriptor,
    pub esphome: EsphomeApiConfig,
    pub ha: HomeAssistantMqttConfig,
}

#[derive(Clone)]
pub struct EsphomeApiConfig {
    pub host: String,
    pub port: u16,
    pub client_name: String,
    pub encryption_key: Option<String>,
}

impl ServiceConfig {
    pub fn from_env(now_unix_ms: u64) -> Self {
        let service_id = env_or_default("HS_SERVICE_ID", "device-esphome-bridge");
        let device_id = env_or_default("HS_DEVICE_ID", "esphome-device-01");
        let manufacturer = env_or_default("HS_DEVICE_MANUFACTURER", "ESPHome");
        let model = env_or_default("HS_DEVICE_MODEL", "esphome-native-api-node");
        let name = env_or_default("HS_DEVICE_NAME", "ESPHome Device");

        let device = DeviceDescriptor {
            service_id,
            device_id,
            manufacturer,
            model,
            name,
            sw_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        };

        Self {
            device,
            esphome: EsphomeApiConfig::from_env(now_unix_ms),
            ha: HomeAssistantMqttConfig::from_env(now_unix_ms),
        }
    }
}

impl EsphomeApiConfig {
    pub fn from_env(now_unix_ms: u64) -> Self {
        let host = env::var("ESPHOME_API_HOST")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "127.0.0.1".to_string());

        let port = env::var("ESPHOME_API_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(6053);

        let client_name = env::var("ESPHOME_API_CLIENT_NAME")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("hs-esphome-client-{now_unix_ms}"));

        let encryption_key = env::var("ESPHOME_API_ENCRYPTION_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Self {
            host,
            port,
            client_name,
            encryption_key,
        }
    }
}

fn env_or_default(key: &str, default_value: &str) -> String {
    env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_value.to_string())
}
