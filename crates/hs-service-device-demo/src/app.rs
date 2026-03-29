use std::time::Duration;

use hs_contracts::{Availability, AvailabilityMessage, StateMessage};
use hs_core::{telemetry, DeviceRuntime};
use hs_eventbus_mqtt_ha::HomeAssistantMqttAdapter;
use serde_json::json;
use tracing::info;

use crate::{
    bootstrap::{demo_capabilities, demo_device, mqtt_config},
    command_payload::{command_is_off, command_is_on},
    time::now_unix_ms,
};

pub async fn run() -> anyhow::Result<()> {
    let telemetry_guard = telemetry::init(env!("CARGO_PKG_NAME"))?;

    let device = demo_device();
    let capabilities = demo_capabilities();
    let config = mqtt_config(&device, now_unix_ms());

    let adapter = HomeAssistantMqttAdapter::connect(config).await?;

    let mut commands = adapter
        .subscribe_device_commands(&device, &capabilities)
        .await?;

    let runtime = DeviceRuntime::new(device.service_id.clone(), adapter);

    runtime
        .announce_device(device.clone(), capabilities)
        .await?;

    runtime
        .publish_availability(AvailabilityMessage {
            device_id: device.device_id.clone(),
            status: Availability::Online,
            detail: "demo device service started".to_string(),
        })
        .await?;

    runtime
        .publish_state(StateMessage {
            device_id: device.device_id.clone(),
            capability_id: "power".to_string(),
            value: json!("OFF"),
            observed_at_unix_ms: now_unix_ms(),
        })
        .await?;

    info!("demo device service started; waiting for switch/button commands");

    let mut switch_on = false;
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    let mut tick_index: u64 = 0;

    loop {
        // Keep telemetry guard alive for the entire runtime so OTEL exporters stay active.
        let _ = &telemetry_guard;

        tokio::select! {
            _ = tick.tick() => {
                if !switch_on {
                    continue;
                }

                tick_index = tick_index.saturating_add(1);
                let temperature = 20.0 + ((tick_index % 10) as f64 * 0.35);
                runtime
                    .publish_state(StateMessage {
                        device_id: device.device_id.clone(),
                        capability_id: "temperature".to_string(),
                        value: json!(temperature),
                        observed_at_unix_ms: now_unix_ms(),
                    })
                    .await?;
            }
            cmd = commands.recv() => {
                let cmd = match cmd {
                    Ok(c) => c,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };

                if cmd.device_id != device.device_id {
                    continue;
                }

                match cmd.capability_id.as_str() {
                    "power" => {
                        if command_is_on(&cmd.payload) {
                            switch_on = true;
                            runtime.publish_state(StateMessage {
                                device_id: device.device_id.clone(),
                                capability_id: "power".to_string(),
                                value: json!("ON"),
                                observed_at_unix_ms: now_unix_ms(),
                            }).await?;
                            info!("switch turned ON");
                        } else if command_is_off(&cmd.payload) {
                            switch_on = false;
                            runtime.publish_state(StateMessage {
                                device_id: device.device_id.clone(),
                                capability_id: "power".to_string(),
                                value: json!("OFF"),
                                observed_at_unix_ms: now_unix_ms(),
                            }).await?;
                            info!("switch turned OFF");
                        }
                    }
                    "shutdown" => {
                        info!("shutdown button pressed; stopping service");
                        runtime.publish_availability(AvailabilityMessage {
                            device_id: device.device_id.clone(),
                            status: Availability::Offline,
                            detail: "shutdown requested by button command".to_string(),
                        }).await?;
                        break;
                    }
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("received ctrl-c; stopping service");
                runtime.publish_availability(AvailabilityMessage {
                    device_id: device.device_id.clone(),
                    status: Availability::Offline,
                    detail: "service stopped by ctrl-c".to_string(),
                }).await?;
                break;
            }
        }
    }

    info!("demo device service exiting");
    Ok(())
}
