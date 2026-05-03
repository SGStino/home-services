use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{CommandMessage, DeviceDescriptor, StateMessage};
use hs_device_core::{
    run_device_service, DeviceRuntime, DeviceServiceBehavior, ServiceDirective, StateFilter,
};
use hs_eventbus_mqtt_ha::HomeAssistantMqttAdapter;
use serde_json::json;
use tracing::{info, warn};

use crate::{
    bootstrap::tasmota_capabilities,
    command_payload::{command_is_off, command_is_on},
    config::{DeviceConfig, ServiceConfig},
    tasmota_client::{TasmotaClient, TasmotaStatus},
    time::now_unix_ms,
};

const TASMOTA_TELEMETRY_THRESHOLDS: &[(&str, f64)] = &[
    ("power_w", 0.001),
    ("voltage_v", 0.001),
    ("current_a", 0.001),
    ("energy_total_kwh", 0.001),
];

const FORCE_EMIT_AFTER_SILENCE_MS: u64 = 5 * 60 * 1_000;

pub async fn run() -> Result<()> {
    let config = ServiceConfig::from_env(now_unix_ms());
    let client = TasmotaClient::new(&config.tasmota)?;
    let status = client.status().await?;
    let device = resolve_device_descriptor(&config.device, &status);

    info!(
        host = %config.tasmota.host,
        module = ?status.module,
        device_name = ?status.device_name,
        device_id = %device.device_id,
        service_id = %device.service_id,
        "resolved Tasmota device metadata"
    );

    let capabilities = tasmota_capabilities();
    let adapter = HomeAssistantMqttAdapter::connect(config.ha).await?;
    let commands = adapter
        .subscribe_device_commands(&device, &capabilities)
        .await?;

    let behavior = TasmotaBehavior {
        client,
        state_filter: StateFilter::with_numeric_thresholds(TASMOTA_TELEMETRY_THRESHOLDS)
            .with_force_emit_after_silence_ms(FORCE_EMIT_AFTER_SILENCE_MS),
    };

    run_device_service(
        env!("CARGO_PKG_NAME"),
        device.service_id.clone(),
        device,
        capabilities,
        adapter,
        commands,
        behavior,
    )
    .await
}

fn resolve_device_descriptor(config: &DeviceConfig, status: &TasmotaStatus) -> DeviceDescriptor {
    let fallback_name = status
        .device_name
        .clone()
        .or_else(|| status.module.clone())
        .unwrap_or_else(|| "Tasmota Device".to_string());

    let fallback_device_id = default_device_id(status);
    let device_id = config
        .device_id
        .clone()
        .unwrap_or_else(|| fallback_device_id.clone());

    DeviceDescriptor {
        service_id: config
            .service_id
            .clone()
            .unwrap_or_else(|| format!("device-{device_id}")),
        device_id,
        manufacturer: config
            .manufacturer
            .clone()
            .unwrap_or_else(|| "Tasmota".to_string()),
        model: config
            .model
            .clone()
            .or_else(|| status.module.clone())
            .unwrap_or_else(|| "Unknown Model".to_string()),
        name: config.name.clone().unwrap_or(fallback_name),
        sw_version: status.firmware.clone(),
    }
}

fn default_device_id(status: &TasmotaStatus) -> String {
    if let Some(mac) = &status.mac {
        let normalized: String = mac
            .chars()
            .filter(|ch| ch.is_ascii_hexdigit())
            .collect::<String>()
            .to_ascii_lowercase();

        if normalized.len() >= 12 {
            return format!("tasmota-{}", &normalized[..12]);
        }
    }

    if let Some(name) = &status.device_name {
        let slug: String = name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
            .collect();
        let compact = slug.trim_matches('-').to_string();
        if !compact.is_empty() {
            return format!("tasmota-{compact}");
        }
    }

    "tasmota-unknown".to_string()
}

struct TasmotaBehavior {
    client: TasmotaClient,
    state_filter: StateFilter,
}

#[async_trait]
impl DeviceServiceBehavior<HomeAssistantMqttAdapter> for TasmotaBehavior {
    fn tick_interval(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn startup_detail(&self) -> &'static str {
        "tasmota service started"
    }

    async fn initial_states(&mut self, device: &DeviceDescriptor) -> Result<Vec<StateMessage>> {
        let status = self.client.status().await?;
        let states = states_from_status(device, status, now_unix_ms());
        self.state_filter.seed_from_states(&states);
        Ok(states)
    }

    async fn on_tick(
        &mut self,
        runtime: &DeviceRuntime<HomeAssistantMqttAdapter>,
        device: &DeviceDescriptor,
    ) -> Result<()> {
        match self.client.status().await {
            Ok(status) => {
                for state in states_from_status(device, status, now_unix_ms()) {
                    if self.state_filter.should_publish_and_remember(&state) {
                        runtime.publish_state(state).await?;
                    }
                }
            }
            Err(error) => {
                warn!(error = %error, "failed to poll Tasmota status");
            }
        }
        Ok(())
    }

    async fn on_command(
        &mut self,
        runtime: &DeviceRuntime<HomeAssistantMqttAdapter>,
        device: &DeviceDescriptor,
        command: CommandMessage,
    ) -> Result<ServiceDirective> {
        if command.capability_id != "power" {
            return Ok(ServiceDirective::Continue);
        }

        if command_is_on(&command.payload) {
            self.client.set_power(true).await?;
            info!("set Tasmota relay to ON");
        } else if command_is_off(&command.payload) {
            self.client.set_power(false).await?;
            info!("set Tasmota relay to OFF");
        } else {
            warn!(payload = %command.payload, "received unsupported power command payload");
            return Ok(ServiceDirective::Continue);
        }

        let status = self.client.status().await?;
        for state in states_from_status(device, status, now_unix_ms()) {
            if self.state_filter.should_publish_and_remember(&state) {
                runtime.publish_state(state).await?;
            }
        }
        Ok(ServiceDirective::Continue)
    }
}

fn states_from_status(
    device: &DeviceDescriptor,
    status: TasmotaStatus,
    observed_ms: u64,
) -> Vec<StateMessage> {
    let mut states = vec![StateMessage {
        device_id: device.device_id.clone(),
        capability_id: "power".to_string(),
        value: json!(if status.relay_on { "ON" } else { "OFF" }),
        observed_ms,
    }];

    if let Some(power_w) = status.power_w {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "power_w".to_string(),
            value: json!(power_w),
            observed_ms,
        });
    }

    if let Some(voltage_v) = status.voltage_v {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "voltage_v".to_string(),
            value: json!(voltage_v),
            observed_ms,
        });
    }

    if let Some(current_a) = status.current_a {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "current_a".to_string(),
            value: json!(current_a),
            observed_ms,
        });
    }

    if let Some(energy_total_kwh) = status.energy_total_kwh {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "energy_total_kwh".to_string(),
            value: json!(energy_total_kwh),
            observed_ms,
        });
    }

    states
}
