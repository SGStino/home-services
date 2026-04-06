use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{
    Availability, AvailabilityMessage, CapabilityDescriptor, CommandMessage, DeviceDescriptor,
    StateMessage,
};
use hs_eventbus_api::EventBusAdapter;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::{telemetry, DeviceRuntime};

pub enum ServiceDirective {
    Continue,
    Stop { detail: String },
}

#[async_trait]
pub trait DeviceServiceBehavior<A>
where
    A: EventBusAdapter,
{
    fn tick_interval(&self) -> Duration {
        Duration::from_secs(1)
    }

    fn startup_detail(&self) -> &'static str {
        "device service started"
    }

    async fn initial_states(&mut self, _device: &DeviceDescriptor) -> Result<Vec<StateMessage>> {
        Ok(Vec::new())
    }

    async fn on_tick(&mut self, runtime: &DeviceRuntime<A>, device: &DeviceDescriptor)
    -> Result<()>;

    async fn on_command(
        &mut self,
        runtime: &DeviceRuntime<A>,
        device: &DeviceDescriptor,
        command: CommandMessage,
    ) -> Result<ServiceDirective>;
}

pub async fn run_device_service<A, B>(
    telemetry_service_name: &str,
    service_id: String,
    device: DeviceDescriptor,
    capabilities: Vec<CapabilityDescriptor>,
    adapter: A,
    mut commands: broadcast::Receiver<CommandMessage>,
    mut behavior: B,
) -> Result<()>
where
    A: EventBusAdapter,
    B: DeviceServiceBehavior<A> + Send,
{
    let _telemetry_guard = telemetry::init(telemetry_service_name)?;

    let runtime = DeviceRuntime::new(service_id, adapter);

    runtime
        .announce_device(device.clone(), capabilities)
        .await?;

    runtime
        .publish_availability(AvailabilityMessage {
            device_id: device.device_id.clone(),
            status: Availability::Online,
            detail: behavior.startup_detail().to_string(),
        })
        .await?;

    for state in behavior.initial_states(&device).await? {
        runtime.publish_state(state).await?;
    }

    info!(device_id = %device.device_id, "device service started");

    let mut tick = tokio::time::interval(behavior.tick_interval());
    let shutdown_detail = loop {
        tokio::select! {
            _ = tick.tick() => {
                behavior.on_tick(&runtime, &device).await?;
            }
            cmd = commands.recv() => {
                let cmd = match cmd {
                    Ok(c) => c,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(skipped, "command receiver lagged");
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break "command stream closed".to_string();
                    }
                };

                if cmd.device_id != device.device_id {
                    continue;
                }

                match behavior.on_command(&runtime, &device, cmd).await? {
                    ServiceDirective::Continue => {}
                    ServiceDirective::Stop { detail } => {
                        break detail;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break "service stopped by ctrl-c".to_string();
            }
        }
    };

    runtime
        .publish_availability(AvailabilityMessage {
            device_id: device.device_id.clone(),
            status: Availability::Offline,
            detail: shutdown_detail,
        })
        .await?;

    info!(device_id = %device.device_id, "device service exiting");
    Ok(())
}
