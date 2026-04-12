use std::{collections::HashMap, time::SystemTime};

use anyhow::{bail, Context, Result};
use hs_device_contracts::{Availability, AvailabilityMessage};
use hs_eventbus_api::EventBusAdapter;
use hs_eventbus_mqtt_sparkplug_b::{SparkplugBConfig, SparkplugBMqttAdapter};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::{
    config::BridgeConfig,
    mapper::NodeSnapshot,
    matter_ws::{connect, parse_message, read_next_message, send_start_listening, MatterMessage},
};

pub async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let bridge_config = BridgeConfig::from_env();
    let mqtt_config = SparkplugBConfig::from_env(now_unix_ms());
    let adapter = SparkplugBMqttAdapter::connect(mqtt_config).await?;

    let mut socket = connect(
        &bridge_config.matter_ws_url,
        bridge_config.matter_tls_ca_cert_path.as_deref(),
    )
    .await?;
    let start_message_id = send_start_listening(&mut socket).await?;

    let mut nodes: HashMap<u64, NodeSnapshot> = HashMap::new();

    while let Some(raw) = read_next_message(&mut socket).await? {
        match parse_message(raw) {
            MatterMessage::ServerInfo(info_payload) => {
                info!(server_info = %info_payload, "Matter server info received");
            }
            MatterMessage::Success { message_id, result } => {
                if message_id == start_message_id {
                    handle_start_snapshot(&adapter, &mut nodes, result).await?;
                    info!(count = nodes.len(), "initial Matter node snapshot processed");
                } else {
                    debug!(message_id = %message_id, "ignoring unrelated success response");
                }
            }
            MatterMessage::Event { event, data } => {
                handle_event(&adapter, &mut nodes, &event, data).await?;
            }
            MatterMessage::Error {
                message_id,
                error_code,
                details,
            } => {
                warn!(
                    message_id = ?message_id,
                    error_code,
                    details = ?details,
                    "Matter server returned error response"
                );
            }
            MatterMessage::Unknown(payload) => {
                debug!(payload = %payload, "ignoring unknown Matter websocket payload");
            }
        }
    }

    info!("Matter websocket closed");
    Ok(())
}

async fn handle_start_snapshot(
    adapter: &SparkplugBMqttAdapter,
    nodes: &mut HashMap<u64, NodeSnapshot>,
    result: Value,
) -> Result<()> {
    let list = result
        .as_array()
        .context("start_listening response result is not an array")?;

    for item in list {
        let Some(snapshot) = NodeSnapshot::from_value(item) else {
            continue;
        };

        publish_full_snapshot(adapter, &snapshot).await?;
        nodes.insert(snapshot.node_id, snapshot);
    }

    Ok(())
}

async fn handle_event(
    adapter: &SparkplugBMqttAdapter,
    nodes: &mut HashMap<u64, NodeSnapshot>,
    event: &str,
    data: Value,
) -> Result<()> {
    match event {
        "node_added" | "node_updated" => {
            if let Some(snapshot) = NodeSnapshot::from_value(&data) {
                publish_full_snapshot(adapter, &snapshot).await?;
                nodes.insert(snapshot.node_id, snapshot);
            }
        }
        "node_removed" => {
            let Some(node_id) = data.as_u64() else {
                bail!("node_removed event did not include numeric node id");
            };

            if let Some(previous) = nodes.remove(&node_id) {
                adapter
                    .publish_availability(&AvailabilityMessage {
                        device_id: previous.device_id(),
                        status: Availability::Offline,
                        detail: "node removed from Matter server".to_string(),
                    })
                    .await?;
            }
        }
        "attribute_updated" => {
            let Some((node_id, path, value)) = parse_attribute_update(&data) else {
                return Ok(());
            };

            if let Some(node) = nodes.get_mut(&node_id) {
                let capability_before = node
                    .state_for_attribute(&path, value.clone(), now_unix_ms())
                    .is_some();

                node.apply_attribute_update(path.clone(), value.clone());

                if !capability_before {
                    adapter.publish_discovery(&node.discovery()).await?;
                }

                if let Some(state) = node.state_for_attribute(&path, value, now_unix_ms()) {
                    adapter.publish_state(&state).await?;
                }
            }
        }
        "server_shutdown" => {
            warn!("Matter server reported shutdown event");
        }
        _ => {
            debug!(event = %event, "ignoring unsupported Matter event");
        }
    }

    Ok(())
}

async fn publish_full_snapshot(adapter: &SparkplugBMqttAdapter, snapshot: &NodeSnapshot) -> Result<()> {
    adapter.publish_discovery(&snapshot.discovery()).await?;
    adapter.publish_availability(&snapshot.availability_message()).await?;

    for state in snapshot.state_messages(now_unix_ms()) {
        adapter.publish_state(&state).await?;
    }

    info!(
        node_id = snapshot.node_id,
        capabilities = snapshot.capabilities().len(),
        "published Matter snapshot to Sparkplug"
    );
    Ok(())
}

fn parse_attribute_update(data: &Value) -> Option<(u64, String, Value)> {
    let list = data.as_array()?;
    if list.len() != 3 {
        return None;
    }

    let node_id = list[0].as_u64()?;
    let path = list[1].as_str()?.to_string();
    let value = list[2].clone();

    Some((node_id, path, value))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
