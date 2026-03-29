use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use hs_contracts::{CommandMessage, DeviceDescriptor, StateMessage};
use hs_core::{run_device_service, DeviceRuntime, DeviceServiceBehavior, ServiceDirective};
use hs_eventbus_mqtt_ha::HomeAssistantMqttAdapter;
use serde_json::json;
use tracing::{info, warn};

use crate::{
    bootstrap::hs110_capabilities,
    command_payload::{command_is_off, command_is_on},
    config::{DeviceConfig, ServiceConfig},
    hs110_client::{Hs110Client, Hs110Snapshot, Hs110SysInfo},
    time::now_unix_ms,
};

pub async fn run() -> Result<()> {
    let config = ServiceConfig::from_env(now_unix_ms());
    let client = Hs110Client::new(&config.hs110);
    let sysinfo = client.sysinfo().await?;
    let device = resolve_device_descriptor(&config.device, &sysinfo);

    info!(
        host = %config.hs110.host,
        model = %sysinfo.model,
        alias = %sysinfo.alias,
        device_id = %device.device_id,
        service_id = %device.service_id,
        "resolved HS110 device metadata"
    );

    let capabilities = hs110_capabilities();

    let adapter = HomeAssistantMqttAdapter::connect(config.ha).await?;
    let commands = adapter
        .subscribe_device_commands(&device, &capabilities)
        .await?;

    let behavior = Hs110Behavior { client };

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

fn resolve_device_descriptor(config: &DeviceConfig, sysinfo: &Hs110SysInfo) -> DeviceDescriptor {
    let fallback_name = if !sysinfo.alias.trim().is_empty() {
        sysinfo.alias.trim().to_string()
    } else if !sysinfo.model.trim().is_empty() {
        sysinfo.model.trim().to_string()
    } else {
        "TP-Link HS110".to_string()
    };

    let fallback_device_id = default_device_id(sysinfo);

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
            .unwrap_or_else(|| "TP-Link".to_string()),
        model: config
            .model
            .clone()
            .unwrap_or_else(|| sysinfo.model.clone()),
        name: config.name.clone().unwrap_or(fallback_name),
        sw_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

fn default_device_id(sysinfo: &Hs110SysInfo) -> String {
    if let Some(mac) = compact_mac(&sysinfo.mac) {
        return format!("tplink-hs110-{mac}");
    }

    let candidate: String = sysinfo
        .device_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(12)
        .collect::<String>()
        .to_ascii_lowercase();

    if !candidate.is_empty() {
        return format!("tplink-hs110-{candidate}");
    }

    "tplink-hs110-unknown".to_string()
}

fn compact_mac(mac: &str) -> Option<String> {
    let normalized: String = mac
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_lowercase();

    if normalized.len() >= 12 {
        Some(normalized)
    } else {
        None
    }
}

struct Hs110Behavior {
    client: Hs110Client,
}

#[async_trait]
impl DeviceServiceBehavior<HomeAssistantMqttAdapter> for Hs110Behavior {
    fn tick_interval(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn startup_detail(&self) -> &'static str {
        "tp-link hs110 service started"
    }

    async fn initial_states(&mut self, device: &DeviceDescriptor) -> Result<Vec<StateMessage>> {
        let snapshot = self.client.snapshot().await?;
        Ok(states_from_snapshot(device, snapshot, now_unix_ms()))
    }

    async fn on_tick(
        &mut self,
        runtime: &DeviceRuntime<HomeAssistantMqttAdapter>,
        device: &DeviceDescriptor,
    ) -> Result<()> {
        match self.client.snapshot().await {
            Ok(snapshot) => {
                for state in states_from_snapshot(device, snapshot, now_unix_ms()) {
                    runtime.publish_state(state).await?;
                }
            }
            Err(error) => {
                warn!(error = %error, "failed to poll HS110 state");
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
            info!("set HS110 relay to ON");
        } else if command_is_off(&command.payload) {
            self.client.set_power(false).await?;
            info!("set HS110 relay to OFF");
        } else {
            warn!(payload = %command.payload, "received unsupported power command payload");
            return Ok(ServiceDirective::Continue);
        }

        let snapshot = self.client.snapshot().await?;
        for state in states_from_snapshot(device, snapshot, now_unix_ms()) {
            runtime.publish_state(state).await?;
        }

        Ok(ServiceDirective::Continue)
    }
}

fn states_from_snapshot(
    device: &DeviceDescriptor,
    snapshot: Hs110Snapshot,
    observed_ms: u64,
) -> Vec<StateMessage> {
    let mut states = vec![StateMessage {
        device_id: device.device_id.clone(),
        capability_id: "power".to_string(),
        value: json!(if snapshot.relay_on { "ON" } else { "OFF" }),
        observed_ms,
    }];

    if let Some(power_w) = snapshot.power_w {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "power_w".to_string(),
            value: json!(power_w),
            observed_ms,
        });
    }

    if let Some(voltage_v) = snapshot.voltage_v {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "voltage_v".to_string(),
            value: json!(voltage_v),
            observed_ms,
        });
    }

    if let Some(current_a) = snapshot.current_a {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "current_a".to_string(),
            value: json!(current_a),
            observed_ms,
        });
    }

    if let Some(energy_total_kwh) = snapshot.energy_total_kwh {
        states.push(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "energy_total_kwh".to_string(),
            value: json!(energy_total_kwh),
            observed_ms,
        });
    }

    states
}
