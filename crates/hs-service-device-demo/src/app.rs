use std::time::Duration;

use async_trait::async_trait;
use hs_contracts::{CommandMessage, DeviceDescriptor, StateMessage};
use hs_core::{run_device_service, DeviceRuntime, DeviceServiceBehavior, ServiceDirective};
use hs_eventbus_mqtt_ha::{HomeAssistantMqttAdapter, HomeAssistantMqttConfig};
use serde_json::json;
use tracing::info;

use crate::{
    bootstrap::{demo_capabilities, demo_device},
    command_payload::{command_is_off, command_is_on},
    time::now_unix_ms,
};

pub async fn run() -> anyhow::Result<()> {
    let device = demo_device();
    let capabilities = demo_capabilities();
    let config = HomeAssistantMqttConfig::from_env_for_device(&device, now_unix_ms());

    let adapter = HomeAssistantMqttAdapter::connect(config).await?;

    let commands = adapter
        .subscribe_device_commands(&device, &capabilities)
        .await?;

    run_device_service(
        env!("CARGO_PKG_NAME"),
        device.service_id.clone(),
        device,
        capabilities,
        adapter,
        commands,
        DemoBehavior::default(),
    )
    .await
}

#[derive(Default)]
struct DemoBehavior {
    switch_on: bool,
    tick_index: u64,
}

#[async_trait]
impl DeviceServiceBehavior<HomeAssistantMqttAdapter> for DemoBehavior {
    fn tick_interval(&self) -> Duration {
        Duration::from_secs(2)
    }

    fn startup_detail(&self) -> &'static str {
        "demo device service started"
    }

    async fn initial_states(&mut self, device: &DeviceDescriptor) -> anyhow::Result<Vec<StateMessage>> {
        Ok(vec![StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "power".to_string(),
            value: json!("OFF"),
            observed_at_unix_ms: now_unix_ms(),
        }])
    }

    async fn on_tick(
        &mut self,
        runtime: &DeviceRuntime<HomeAssistantMqttAdapter>,
        device: &DeviceDescriptor,
    ) -> anyhow::Result<()> {
        if !self.switch_on {
            return Ok(());
        }

        self.tick_index = self.tick_index.saturating_add(1);
        let temperature = 20.0 + ((self.tick_index % 10) as f64 * 0.35);
        runtime
            .publish_state(StateMessage {
                device_id: device.device_id.clone(),
                capability_id: "temperature".to_string(),
                value: json!(temperature),
                observed_at_unix_ms: now_unix_ms(),
            })
            .await
    }

    async fn on_command(
        &mut self,
        runtime: &DeviceRuntime<HomeAssistantMqttAdapter>,
        device: &DeviceDescriptor,
        command: CommandMessage,
    ) -> anyhow::Result<ServiceDirective> {
        match command.capability_id.as_str() {
            "power" => {
                if command_is_on(&command.payload) {
                    self.switch_on = true;
                    runtime
                        .publish_state(StateMessage {
                            device_id: device.device_id.clone(),
                            capability_id: "power".to_string(),
                            value: json!("ON"),
                            observed_at_unix_ms: now_unix_ms(),
                        })
                        .await?;
                    info!("switch turned ON");
                } else if command_is_off(&command.payload) {
                    self.switch_on = false;
                    runtime
                        .publish_state(StateMessage {
                            device_id: device.device_id.clone(),
                            capability_id: "power".to_string(),
                            value: json!("OFF"),
                            observed_at_unix_ms: now_unix_ms(),
                        })
                        .await?;
                    info!("switch turned OFF");
                }
                Ok(ServiceDirective::Continue)
            }
            "shutdown" => {
                info!("shutdown button pressed; stopping service");
                Ok(ServiceDirective::Stop {
                    detail: "shutdown requested by button command".to_string(),
                })
            }
            _ => Ok(ServiceDirective::Continue),
        }
    }
}
