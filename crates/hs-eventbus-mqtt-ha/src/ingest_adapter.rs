use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{
    Availability, AvailabilityMessage, CapabilityDescriptor, CapabilityKind, DeviceClass,
    DeviceDescriptor, DiscoveryMessage, LightFeatures, NumberConfig, StateMessage,
};
use hs_eventbus_api::{DiscoveryKey, EventProcessor, IngestAdapter};
use rumqttc::{AsyncClient, Event, Packet, QoS};
use serde_json::Value;
use tracing::{debug, error, warn};

use crate::{config::HomeAssistantMqttConfig, transport::create_client};

#[derive(Clone, Debug)]
pub struct HomeAssistantMqttIngestAdapter {
    config: HomeAssistantMqttConfig,
}

impl HomeAssistantMqttIngestAdapter {
    pub fn new(config: HomeAssistantMqttConfig) -> Self {
        Self { config }
    }

    pub async fn connect(config: HomeAssistantMqttConfig) -> Result<Self> {
        Ok(Self::new(config))
    }
}

#[async_trait]
impl IngestAdapter for HomeAssistantMqttIngestAdapter {
    fn adapter_name(&self) -> &'static str {
        "mqtt-home-assistant-ingest"
    }

    async fn initialize(&self, processor: Arc<dyn EventProcessor>) -> Result<()> {
        let (client, mut event_loop) = create_client(&self.config);

        let discovery_filter = format!("{}/+/+/+/config", self.config.discovery_prefix);
        client.subscribe(discovery_filter, QoS::AtLeastOnce).await?;
        client.subscribe("hs/state/+/+/+", QoS::AtLeastOnce).await?;

        let discovery_prefix = self.config.discovery_prefix.clone();
        let command_client = client.clone();
        tokio::spawn(async move {
            let mut discovery_availability_topics: HashMap<DiscoveryKey, String> = HashMap::new();
            let mut availability_topic_ref_count: HashMap<String, usize> = HashMap::new();
            let mut availability_topic_node_id: HashMap<String, String> = HashMap::new();

            loop {
                match event_loop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(msg))) => {
                        if let Some((component, node_id, object_id, discovery_key)) =
                            parse_discovery_topic(&discovery_prefix, &msg.topic)
                        {
                            if msg.payload.is_empty() {
                                if let Some(topic) = discovery_availability_topics.remove(&discovery_key)
                                {
                                    unsubscribe_availability_topic(
                                        &command_client,
                                        &mut availability_topic_ref_count,
                                        &mut availability_topic_node_id,
                                        &topic,
                                    )
                                    .await;
                                }
                                processor.on_tombstone(discovery_key).await;
                                continue;
                            }

                            let availability_topic = parse_discovery_availability_topic(&msg.payload);
                            reconcile_availability_subscription(
                                &command_client,
                                &discovery_key,
                                &node_id,
                                availability_topic,
                                &mut discovery_availability_topics,
                                &mut availability_topic_ref_count,
                                &mut availability_topic_node_id,
                            )
                            .await;

                            if let Some(discovery) =
                                parse_discovery_message(&component, &node_id, &object_id, &msg.payload)
                            {
                                processor.on_discovery(discovery_key, discovery).await;
                            } else {
                                warn!(topic = %msg.topic, "failed to parse discovery payload");
                            }
                            continue;
                        }

                        if let Some((_, device_id, capability_id)) = parse_state_topic(&msg.topic) {
                            if let Some(state) =
                                parse_state_message(&device_id, &capability_id, &msg.payload)
                            {
                                processor.on_state(state).await;
                            } else {
                                warn!(topic = %msg.topic, "failed to parse state payload");
                            }
                            continue;
                        }

                        if let Some(node_id) = availability_topic_node_id.get(&msg.topic) {
                            let Some(status) = parse_availability_status(&msg.payload) else {
                                warn!(topic = %msg.topic, "failed to parse availability payload");
                                continue;
                            };

                            // Availability topic identity comes from discovery payload mapping.
                            processor
                                .on_availability(AvailabilityMessage {
                                    device_id: node_id.clone(),
                                    status,
                                    detail: "node-scoped availability".to_string(),
                                })
                                .await;
                            continue;
                        }

                        debug!(topic = %msg.topic, "ignored non-ingest MQTT topic");
                    }
                    Ok(_) => {}
                    Err(err) => {
                        error!(error = %err, "mqtt ingest event loop error");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        });

        Ok(())
    }
}

fn parse_discovery_topic(
    discovery_prefix: &str,
    topic: &str,
) -> Option<(String, String, String, DiscoveryKey)> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 5 {
        return None;
    }

    if parts[0] != discovery_prefix || parts[4] != "config" {
        return None;
    }

    Some((
        parts[1].to_string(),
        parts[2].to_string(),
        parts[3].to_string(),
        DiscoveryKey::from(topic),
    ))
}

fn parse_state_topic(topic: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 5 || parts[0] != "hs" || parts[1] != "state" {
        return None;
    }

    Some((
        parts[2].to_string(),
        parts[3].to_string(),
        parts[4].to_string(),
    ))
}

fn parse_availability_status(payload: &[u8]) -> Option<Availability> {
    let status_text = String::from_utf8_lossy(payload).trim().to_ascii_lowercase();
    Some(match status_text.as_str() {
        "online" => Availability::Online,
        "offline" => Availability::Offline,
        "degraded" => Availability::Degraded,
        _ => return None,
    })
}

fn parse_discovery_availability_topic(payload: &[u8]) -> Option<String> {
    let payload: Value = serde_json::from_slice(payload).ok()?;
    let topic = payload.get("availability_topic")?.as_str()?.trim();
    if topic.is_empty() {
        return None;
    }

    Some(topic.to_string())
}

async fn reconcile_availability_subscription(
    client: &AsyncClient,
    discovery_key: &DiscoveryKey,
    node_id: &str,
    next_topic: Option<String>,
    discovery_availability_topics: &mut HashMap<DiscoveryKey, String>,
    availability_topic_ref_count: &mut HashMap<String, usize>,
    availability_topic_node_id: &mut HashMap<String, String>,
) {
    let current_topic = discovery_availability_topics.get(discovery_key).cloned();
    if current_topic == next_topic {
        return;
    }

    let Some(next_topic) = next_topic else {
        if let Some(previous_topic) = discovery_availability_topics.remove(discovery_key) {
            unsubscribe_availability_topic(
                client,
                availability_topic_ref_count,
                availability_topic_node_id,
                &previous_topic,
            )
            .await;
        }
        return;
    };

    if !subscribe_availability_topic(
        client,
        availability_topic_ref_count,
        availability_topic_node_id,
        &next_topic,
        node_id,
    )
    .await
    {
        warn!(
            discovery_key = ?discovery_key,
            topic = %next_topic,
            "failed to subscribe availability topic; keeping previous subscription"
        );
        return;
    }

    if let Some(previous_topic) = discovery_availability_topics
        .insert(discovery_key.clone(), next_topic)
    {
        unsubscribe_availability_topic(
            client,
            availability_topic_ref_count,
            availability_topic_node_id,
            &previous_topic,
        )
        .await;
    }
}

async fn subscribe_availability_topic(
    client: &AsyncClient,
    availability_topic_ref_count: &mut HashMap<String, usize>,
    availability_topic_node_id: &mut HashMap<String, String>,
    topic: &str,
    node_id: &str,
) -> bool {
    let ref_count = availability_topic_ref_count.get(topic).copied().unwrap_or(0);
    if ref_count == 0
        && client
            .subscribe(topic.to_string(), QoS::AtLeastOnce)
            .await
            .is_err()
    {
        return false;
    }

    if ref_count == 0 {
        availability_topic_node_id.insert(topic.to_string(), node_id.to_string());
    } else if availability_topic_node_id
        .get(topic)
        .is_some_and(|mapped_node_id| mapped_node_id != node_id)
    {
        warn!(
            topic = %topic,
            existing_node_id = %availability_topic_node_id.get(topic).cloned().unwrap_or_default(),
            new_node_id = %node_id,
            "availability topic reused across node ids; retaining original node mapping"
        );
    }

    availability_topic_ref_count.insert(topic.to_string(), ref_count + 1);
    true
}

async fn unsubscribe_availability_topic(
    client: &AsyncClient,
    availability_topic_ref_count: &mut HashMap<String, usize>,
    availability_topic_node_id: &mut HashMap<String, String>,
    topic: &str,
) {
    let Some(ref_count) = availability_topic_ref_count.get(topic).copied() else {
        return;
    };

    if ref_count > 1 {
        availability_topic_ref_count.insert(topic.to_string(), ref_count - 1);
        return;
    }

    availability_topic_ref_count.remove(topic);
    availability_topic_node_id.remove(topic);
    if let Err(err) = client.unsubscribe(topic.to_string()).await {
        warn!(error = %err, topic = %topic, "failed to unsubscribe availability topic");
    }
}

fn parse_state_message(device_id: &str, capability_id: &str, payload: &[u8]) -> Option<StateMessage> {
    let value: Value = serde_json::from_slice(payload).ok()?;
    let observed_ms = value
        .get("ts")
        .and_then(Value::as_u64)
        .or_else(|| value.get("observed_at_unix_ms").and_then(Value::as_u64))
        .unwrap_or(0);

    Some(StateMessage {
        device_id: device_id.to_string(),
        capability_id: capability_id.to_string(),
        value: value.get("value").cloned().unwrap_or(Value::Null),
        observed_ms,
    })
}

fn parse_discovery_message(
    component: &str,
    node_id: &str,
    object_id: &str,
    payload: &[u8],
) -> Option<DiscoveryMessage> {
    let payload: Value = serde_json::from_slice(payload).ok()?;

    let (topic_device_id, topic_capability_id) = parse_state_topic(
        payload
            .get("state_topic")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .map(|(_, device_id, capability_id)| (device_id, capability_id))
    .or_else(|| split_object_id(object_id))?;

    let device = payload.get("device")?;
    let descriptor = DeviceDescriptor {
        service_id: node_id.to_string(),
        device_id: topic_device_id,
        manufacturer: device
            .get("manufacturer")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        model: device
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        name: device
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        sw_version: device
            .get("sw_version")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    };

    let capability = CapabilityDescriptor {
        capability_id: topic_capability_id.clone(),
        kind: capability_kind_from_component(component, &payload),
        friendly_name: payload
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&topic_capability_id)
            .to_string(),
        unit_of_measurement: payload
            .get("unit_of_measurement")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    };

    Some(DiscoveryMessage {
        device: descriptor,
        capabilities: vec![capability],
    })
}

fn capability_kind_from_component(component: &str, payload: &Value) -> CapabilityKind {
    match component {
        "sensor" => CapabilityKind::Sensor {
            device_class: payload
                .get("device_class")
                .and_then(Value::as_str)
                .map(DeviceClass::from),
        },
        "binary_sensor" => CapabilityKind::BinarySensor {
            device_class: payload
                .get("device_class")
                .and_then(Value::as_str)
                .map(DeviceClass::from),
        },
        "switch" => CapabilityKind::Switch,
        "button" => CapabilityKind::Button,
        "light" => CapabilityKind::Light {
            features: LightFeatures::on_off_only(),
        },
        "number" => CapabilityKind::Number {
            config: NumberConfig {
                min: payload.get("min").and_then(Value::as_f64).unwrap_or(0.0),
                max: payload.get("max").and_then(Value::as_f64).unwrap_or(100.0),
                step: payload.get("step").and_then(Value::as_f64).unwrap_or(1.0),
                unit_of_measurement: payload
                    .get("unit_of_measurement")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            },
        },
        "select" => CapabilityKind::Select {
            options: payload
                .get("options")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        },
        "cover" => CapabilityKind::Cover,
        "climate" => CapabilityKind::Climate,
        _ => CapabilityKind::Sensor { device_class: None },
    }
}

fn split_object_id(object_id: &str) -> Option<(String, String)> {
    let (device_id, capability_id) = object_id.rsplit_once('_')?;
    Some((device_id.to_string(), capability_id.to_string()))
}
