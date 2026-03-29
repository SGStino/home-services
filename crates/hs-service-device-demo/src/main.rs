use std::{env, time::{Duration, SystemTime, UNIX_EPOCH}};

use hs_contracts::{
    sensor_class, Availability, AvailabilityMessage, CapabilityDescriptor, CapabilityKind,
    DeviceClass, DeviceDescriptor, StateMessage,
};
use hs_core::{telemetry, DeviceRuntime};
use hs_eventbus_mqtt_ha::{HomeAssistantMqttAdapter, HomeAssistantMqttConfig};
use serde_json::json;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let telemetry_guard = telemetry::init(env!("CARGO_PKG_NAME"))?;

    let device = DeviceDescriptor {
        service_id: "device-demo-living-room-node".to_string(),
        device_id: "living-room-node-01".to_string(),
        manufacturer: "Home Services".to_string(),
        model: "demo-sensor-switch-button".to_string(),
        name: "Living Room Demo Node".to_string(),
        sw_version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };

    let capabilities = vec![
        CapabilityDescriptor {
            capability_id: "temperature".to_string(),
            kind: CapabilityKind::Sensor {
                device_class: Some(DeviceClass::from(sensor_class::TEMPERATURE)),
            },
            friendly_name: "Temperature".to_string(),
            unit_of_measurement: Some("°C".to_string()),
        },
        CapabilityDescriptor {
            capability_id: "power".to_string(),
            kind: CapabilityKind::Switch,
            friendly_name: "Power Switch".to_string(),
            unit_of_measurement: None,
        },
        CapabilityDescriptor {
            capability_id: "shutdown".to_string(),
            kind: CapabilityKind::Button,
            friendly_name: "Shutdown Button".to_string(),
            unit_of_measurement: None,
        },
    ];

    let broker_host = env::var("MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let node_id = env::var("MQTT_NODE_ID").unwrap_or_else(|_| "hs-node-dev".to_string());

    let adapter = HomeAssistantMqttAdapter::connect(HomeAssistantMqttConfig {
        broker_host,
        broker_port: 1883,
        client_id: format!("{}-{}", device.service_id, now_unix_ms()),
        discovery_prefix: "homeassistant".to_string(),
        node_id,
        lwt_device_id: device.device_id.clone(),
    })
    .await?;

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

fn command_is_on(payload: &serde_json::Value) -> bool {
    match payload {
        serde_json::Value::String(s) => {
            let s = s.trim().to_ascii_uppercase();
            s == "ON" || s == "1" || s == "TRUE"
        }
        serde_json::Value::Bool(v) => *v,
        serde_json::Value::Number(n) => n.as_i64() == Some(1),
        _ => false,
    }
}

fn command_is_off(payload: &serde_json::Value) -> bool {
    match payload {
        serde_json::Value::String(s) => {
            let s = s.trim().to_ascii_uppercase();
            s == "OFF" || s == "0" || s == "FALSE"
        }
        serde_json::Value::Bool(v) => !*v,
        serde_json::Value::Number(n) => n.as_i64() == Some(0),
        _ => false,
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
