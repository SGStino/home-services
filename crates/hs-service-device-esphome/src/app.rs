use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{CommandMessage, DeviceDescriptor, StateMessage};
use hs_device_core::{run_device_service, DeviceRuntime, DeviceServiceBehavior, ServiceDirective};
use hs_eventbus_mqtt_ha::HomeAssistantMqttAdapter;
use tracing::{info, warn};

use crate::{
    config::ServiceConfig,
    esphome::EsphomeBridge,
    time::now_unix_ms,
};

pub async fn run() -> Result<()> {
    let config = ServiceConfig::from_env(now_unix_ms());

    let bridge = EsphomeBridge::connect(&config.esphome).await?;
    let capabilities = bridge.capabilities().to_vec();

    let adapter = HomeAssistantMqttAdapter::connect(config.ha).await?;
    let commands = adapter
        .subscribe_device_commands(&config.device, &capabilities)
        .await?;

    let behavior = EsphomeBehavior { bridge };

    run_device_service(
        env!("CARGO_PKG_NAME"),
        config.device.service_id.clone(),
        config.device,
        capabilities,
        adapter,
        commands,
        behavior,
    )
    .await
}

struct EsphomeBehavior {
    bridge: EsphomeBridge,
}

#[async_trait]
impl DeviceServiceBehavior<HomeAssistantMqttAdapter> for EsphomeBehavior {
    fn tick_interval(&self) -> Duration {
        Duration::from_millis(250)
    }

    fn startup_detail(&self) -> &'static str {
        "esphome native api bridge service started"
    }

    async fn initial_states(&mut self, _device: &DeviceDescriptor) -> Result<Vec<StateMessage>> {
        Ok(Vec::new())
    }

    async fn on_tick(
        &mut self,
        runtime: &DeviceRuntime<HomeAssistantMqttAdapter>,
        device: &DeviceDescriptor,
    ) -> Result<()> {
        for update in self.bridge.drain_state_updates() {
            runtime
                .publish_state(StateMessage {
                    device_id: device.device_id.clone(),
                    capability_id: update.capability_id,
                    value: update.value,
                    observed_ms: update.observed_ms,
                })
                .await?;
        }
        Ok(())
    }

    async fn on_command(
        &mut self,
        _runtime: &DeviceRuntime<HomeAssistantMqttAdapter>,
        _device: &DeviceDescriptor,
        command: CommandMessage,
    ) -> Result<ServiceDirective> {
        if self.bridge.forward_command(&command).await? {
            info!(
                capability_id = %command.capability_id,
                "forwarded Home Assistant command to ESPHome topic"
            );
        } else {
            warn!(
                capability_id = %command.capability_id,
                "received command for capability without ESPHome command mapping"
            );
        }
        Ok(ServiceDirective::Continue)
    }
}
