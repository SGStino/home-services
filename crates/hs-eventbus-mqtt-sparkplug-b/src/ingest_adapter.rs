use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{
    Availability, AvailabilityMessage, CapabilityDescriptor, CapabilityKind, DeviceDescriptor,
    DiscoveryMessage, StateMessage,
};
use hs_eventbus_api::{DiscoveryKey, EventProcessor, IngestAdapter};
use rumqttc::{Event, Packet, QoS};
use tracing::{debug, error, warn};

use crate::{
    config::SparkplugBConfig,
    payloads::{decode_payload, metric_value_to_json},
    sparkplug::DataType,
    transport::create_client,
};

#[derive(Clone, Debug)]
pub struct SparkplugBMqttIngestAdapter {
    config: SparkplugBConfig,
}

impl SparkplugBMqttIngestAdapter {
    pub fn new(config: SparkplugBConfig) -> Self {
        Self { config }
    }

    pub async fn connect(config: SparkplugBConfig) -> Result<Self> {
        Ok(Self::new(config))
    }
}

#[async_trait]
impl IngestAdapter for SparkplugBMqttIngestAdapter {
    fn adapter_name(&self) -> &'static str {
        "mqtt-sparkplug-b-ingest"
    }

    async fn initialize(&self, processor: Arc<dyn EventProcessor>) -> Result<()> {
        let (client, mut event_loop) = create_client(&self.config);

        let dbirth_filter = format!("spBv1.0/{}/DBIRTH/+/+", self.config.group_id);
        let ddata_filter = format!("spBv1.0/{}/DDATA/+/+", self.config.group_id);
        let ddeath_filter = format!("spBv1.0/{}/DDEATH/+/+", self.config.group_id);
        let state_filter = "spBv1.0/STATE/+";

        client.subscribe(dbirth_filter, QoS::AtLeastOnce).await?;
        client.subscribe(ddata_filter, QoS::AtLeastOnce).await?;
        client.subscribe(ddeath_filter, QoS::AtLeastOnce).await?;
        client.subscribe(state_filter, QoS::AtLeastOnce).await?;

        tokio::spawn(async move {
            loop {
                match event_loop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(msg))) => {
                        if let Some((group_id, edge_node_id, device_id)) = parse_dbirth_topic(&msg.topic) {
                            if msg.payload.is_empty() {
                                processor.on_tombstone(DiscoveryKey::from(msg.topic)).await;
                                continue;
                            }

                            if let Some(discovery) = parse_discovery_message(
                                &group_id,
                                &edge_node_id,
                                &device_id,
                                &msg.payload,
                            ) {
                                processor
                                    .on_discovery(DiscoveryKey::from(msg.topic), discovery)
                                    .await;
                            } else {
                                warn!(topic = %msg.topic, "failed to parse Sparkplug DBIRTH payload");
                            }
                            continue;
                        }

                        if let Some((_, _, device_id)) = parse_ddata_topic(&msg.topic) {
                            match parse_state_messages(&device_id, &msg.payload) {
                                Some(states) => {
                                    for state in states {
                                        processor.on_state(state).await;
                                    }
                                }
                                None => warn!(topic = %msg.topic, "failed to parse Sparkplug DDATA payload"),
                            }
                            continue;
                        }

                        if let Some((_group_id, _edge_node_id, _device_id)) = parse_ddeath_topic(&msg.topic)
                        {
                            processor.on_tombstone(DiscoveryKey::from(msg.topic)).await;
                            continue;
                        }

                        if let Some((edge_node_id, status)) = parse_state_message(&msg.topic, &msg.payload)
                        {
                            processor
                                .on_availability(AvailabilityMessage {
                                    device_id: edge_node_id,
                                    status,
                                    detail: "sparkplug node state".to_string(),
                                })
                                .await;
                            continue;
                        }

                        debug!(topic = %msg.topic, "ignored non-sparkplug MQTT topic");
                    }
                    Ok(_) => {}
                    Err(err) => {
                        error!(error = %err, "sparkplug ingest event loop error");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        });

        Ok(())
    }
}

fn parse_dbirth_topic(topic: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 5 || parts[0] != "spBv1.0" || parts[2] != "DBIRTH" {
        return None;
    }

    Some((
        parts[1].to_string(),
        parts[3].to_string(),
        parts[4].to_string(),
    ))
}

fn parse_ddata_topic(topic: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 5 || parts[0] != "spBv1.0" || parts[2] != "DDATA" {
        return None;
    }

    Some((
        parts[1].to_string(),
        parts[3].to_string(),
        parts[4].to_string(),
    ))
}

fn parse_ddeath_topic(topic: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 5 || parts[0] != "spBv1.0" || parts[2] != "DDEATH" {
        return None;
    }

    Some((
        parts[1].to_string(),
        parts[3].to_string(),
        parts[4].to_string(),
    ))
}

fn parse_state_message(topic: &str, payload: &[u8]) -> Option<(String, Availability)> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 3 || parts[0] != "spBv1.0" || parts[1] != "STATE" {
        return None;
    }

    let status_text = String::from_utf8_lossy(payload).trim().to_ascii_uppercase();
    let status = match status_text.as_str() {
        "ONLINE" => Availability::Online,
        "OFFLINE" => Availability::Offline,
        "DEGRADED" => Availability::Degraded,
        _ => return None,
    };

    Some((parts[2].to_string(), status))
}

fn parse_discovery_message(
    _group_id: &str,
    edge_node_id: &str,
    device_id: &str,
    payload: &[u8],
) -> Option<DiscoveryMessage> {
    let payload = decode_payload(payload)?;
    let metrics = payload.metrics.iter();

    let capabilities = metrics
        .filter_map(parse_metric_capability)
        .collect::<Vec<_>>();

    if capabilities.is_empty() {
        return None;
    }

    Some(DiscoveryMessage {
        device: DeviceDescriptor {
            service_id: edge_node_id.to_string(),
            device_id: device_id.to_string(),
            manufacturer: "unknown".to_string(),
            model: "unknown".to_string(),
            name: device_id.to_string(),
            sw_version: None,
        },
        capabilities,
    })
}

fn parse_metric_capability(metric: &crate::sparkplug::payload::Metric) -> Option<CapabilityDescriptor> {
    let capability_id = metric.name.clone()?;
    let datatype = metric.datatype.and_then(DataType::from_u32);

    let kind = match datatype {
        Some(DataType::Boolean) => CapabilityKind::BinarySensor { device_class: None },
        _ => CapabilityKind::Sensor { device_class: None },
    };

    Some(CapabilityDescriptor {
        capability_id: capability_id.clone(),
        kind,
        friendly_name: capability_id,
        unit_of_measurement: None,
    })
}

fn parse_state_messages(device_id: &str, payload: &[u8]) -> Option<Vec<StateMessage>> {
    let payload = decode_payload(payload)?;
    let observed_ms = payload.timestamp.unwrap_or(0);
    let metrics = payload.metrics;

    let messages = metrics
        .into_iter()
        .filter_map(|metric| {
            let capability_id = metric.name.clone()?;
            let value = metric_value_to_json(&metric)?;

            Some(StateMessage {
                device_id: device_id.to_string(),
                capability_id,
                value,
                observed_ms,
            })
        })
        .collect::<Vec<_>>();

    if messages.is_empty() {
        return None;
    }

    Some(messages)
}
