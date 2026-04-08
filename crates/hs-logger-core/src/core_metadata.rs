use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{
    AvailabilityMessage, CapabilityDescriptor, CapabilityKind, DiscoveryMessage, StateMessage,
};
use hs_eventbus_api::{DiscoveryKey, EventProcessor};
use opentelemetry::KeyValue;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::datapoint::{DataPoint, DataPointField};
use crate::metrics::logger_metrics;
use crate::{LoggerConfig, PointWriter};

#[derive(Clone)]
pub struct CoreMetadata {
    writer: Arc<dyn PointWriter>,
    config: LoggerConfig,
    entities: Arc<RwLock<HashMap<(String, String), EntityMetadata>>>,
    discovery_entities: Arc<RwLock<HashMap<DiscoveryKey, Vec<(String, String)>>>>,
}

#[derive(Clone, Debug)]
struct EntityMetadata {
    metadata: BTreeMap<String, String>,
    capability_kind: CapabilityKind,
}

impl CoreMetadata {
    pub fn new(writer: Arc<dyn PointWriter>, config: LoggerConfig) -> Self {
        Self {
            writer,
            config,
            entities: Arc::new(RwLock::new(HashMap::new())),
            discovery_entities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn state_index_key(device_id: &str, capability_id: &str) -> (String, String) {
        (device_id.to_string(), capability_id.to_string())
    }

    fn metadata_for_capability(
        discovery: &DiscoveryMessage,
        capability: &CapabilityDescriptor,
    ) -> BTreeMap<String, String> {
        let mut metadata = BTreeMap::new();
        metadata.insert("node_id".to_string(), discovery.device.service_id.clone());
        metadata.insert("device_id".to_string(), discovery.device.device_id.clone());
        metadata.insert("capability_id".to_string(), capability.capability_id.clone());
        metadata.insert(
            "capability_kind".to_string(),
            capability_kind_name(&capability.kind).to_string(),
        );

        if let Some(device_class) = capability_device_class(&capability.kind) {
            metadata.insert("device_class".to_string(), device_class);
        }

        metadata.insert(
            "manufacturer".to_string(),
            discovery.device.manufacturer.clone(),
        );
        metadata.insert("model".to_string(), discovery.device.model.clone());

        if let Some(unit) = &capability.unit_of_measurement {
            metadata.insert("unit".to_string(), unit.clone());
        }

        metadata
    }

    fn filter_tags(&self, metadata: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        metadata
            .iter()
            .filter(|(k, _)| self.config.should_log_metadata_key(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    async fn write_state_point(&self, metadata: &EntityMetadata, state: StateMessage) -> Result<()> {
        let measurement = measurement_for_capability(&metadata.capability_kind).to_string();
        let fields = fields_from_state_value(&state.value, &metadata.capability_kind);

        if fields.is_empty() {
            logger_metrics().dropped_events_total.add(
                1,
                &[
                    KeyValue::new("event.kind", "state"),
                    KeyValue::new("reason", "unsupported_fields"),
                ],
            );
            debug!(
                device_id = %state.device_id,
                capability_id = %state.capability_id,
                "dropping state sample with no supported point fields"
            );
            return Ok(());
        }

        let point = DataPoint {
            measurement,
            tags: self.filter_tags(&metadata.metadata),
            fields,
            observed_ms: state.observed_ms,
        };

        let point_count = 1;
        let started_at = Instant::now();
        let result = self.writer.write_points(vec![point]).await;
        let elapsed_seconds = started_at.elapsed().as_secs_f64();

        let outcome = if result.is_ok() { "ok" } else { "error" };
        logger_metrics()
            .write_batches_total
            .add(1, &[KeyValue::new("outcome", outcome)]);
        logger_metrics().write_points_total.add(
            point_count,
            &[KeyValue::new("outcome", outcome)],
        );
        logger_metrics().write_latency_seconds.record(
            elapsed_seconds,
            &[KeyValue::new("outcome", outcome)],
        );

        result
    }

    async fn write_availability_points(&self, availability: AvailabilityMessage) -> Result<()> {
        let entities = self.entities.read().await;
        let matching: Vec<EntityMetadata> = entities
            .values()
            .filter(|entity| {
                entity
                    .metadata
                    .get("node_id")
                    .is_some_and(|node_id| node_id == &availability.device_id)
                    || entity
                        .metadata
                        .get("device_id")
                        .is_some_and(|device_id| device_id == &availability.device_id)
            })
            .cloned()
            .collect();

        drop(entities);

        if matching.is_empty() {
            logger_metrics().dropped_events_total.add(
                1,
                &[
                    KeyValue::new("event.kind", "availability"),
                    KeyValue::new("reason", "missing_metadata"),
                ],
            );
            debug!(
                availability_id = %availability.device_id,
                "dropping availability sample without active metadata"
            );
            return Ok(());
        }

        let point_count = matching.len() as u64;
        let observed_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let points = matching
            .into_iter()
            .map(|entity| {
                DataPoint {
                    measurement: "availability".to_string(),
                    tags: self.filter_tags(&entity.metadata),
                    fields: BTreeMap::from([(
                        "value".to_string(),
                        DataPointField::Number(availability_code(&availability.status) as f64),
                    )]),
                    observed_ms,
                }
            })
            .collect();

        let started_at = Instant::now();
        let result = self.writer.write_points(points).await;
        let elapsed_seconds = started_at.elapsed().as_secs_f64();
        let outcome = if result.is_ok() { "ok" } else { "error" };
        logger_metrics()
            .write_batches_total
            .add(1, &[KeyValue::new("outcome", outcome)]);
        logger_metrics().write_points_total.add(
            point_count,
            &[KeyValue::new("outcome", outcome)],
        );
        logger_metrics().write_latency_seconds.record(
            elapsed_seconds,
            &[KeyValue::new("outcome", outcome)],
        );

        result
    }
}

#[async_trait]
impl EventProcessor for CoreMetadata {
    async fn on_discovery(&self, key: DiscoveryKey, event: DiscoveryMessage) {
        logger_metrics().ingest_events_total.add(
            1,
            &[KeyValue::new("event.kind", "discovery")],
        );

        if event.capabilities.is_empty() {
            logger_metrics().dropped_events_total.add(
                1,
                &[
                    KeyValue::new("event.kind", "discovery"),
                    KeyValue::new("reason", "no_capabilities"),
                ],
            );
            warn!("discovery event ignored because it had no capabilities");
            return;
        }

        let mut entities = self.entities.write().await;
        let mut discovery_entities = self.discovery_entities.write().await;

        if let Some(previous_keys) = discovery_entities.remove(&key) {
            for state_key in previous_keys {
                entities.remove(&state_key);
            }
        }

        let mut discovered_state_keys = Vec::with_capacity(event.capabilities.len());
        for capability in &event.capabilities {
            let entity = EntityMetadata {
                metadata: Self::metadata_for_capability(&event, capability),
                capability_kind: capability.kind.clone(),
            };

            let state_key = Self::state_index_key(&event.device.device_id, &capability.capability_id);
            entities.insert(state_key.clone(), entity);
            discovered_state_keys.push(state_key);
        }

        discovery_entities.insert(key, discovered_state_keys);
    }

    async fn on_tombstone(&self, key: DiscoveryKey) {
        logger_metrics().ingest_events_total.add(
            1,
            &[KeyValue::new("event.kind", "tombstone")],
        );

        let mut entities = self.entities.write().await;
        let mut discovery_entities = self.discovery_entities.write().await;
        if let Some(state_keys) = discovery_entities.remove(&key) {
            for state_key in state_keys {
                entities.remove(&state_key);
            }
        }
    }

    async fn on_state(&self, state: StateMessage) {
        logger_metrics().ingest_events_total.add(
            1,
            &[KeyValue::new("event.kind", "state")],
        );

        let index_key = Self::state_index_key(&state.device_id, &state.capability_id);
        let metadata = { self.entities.read().await.get(&index_key).cloned() };
        let Some(metadata) = metadata else {
            logger_metrics().dropped_events_total.add(
                1,
                &[
                    KeyValue::new("event.kind", "state"),
                    KeyValue::new("reason", "missing_metadata"),
                ],
            );
            debug!(
                device_id = %state.device_id,
                capability_id = %state.capability_id,
                "dropping state sample without active discovery metadata"
            );
            return;
        };

        if let Err(err) = self.write_state_point(&metadata, state).await {
            warn!(error = %err, "failed to write state data point");
        }
    }

    async fn on_availability(&self, availability: AvailabilityMessage) {
        logger_metrics().ingest_events_total.add(
            1,
            &[KeyValue::new("event.kind", "availability")],
        );

        if let Err(err) = self.write_availability_points(availability).await {
            warn!(error = %err, "failed to write availability data point");
        }
    }
}

fn capability_kind_name(kind: &CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Sensor { .. } => "sensor",
        CapabilityKind::BinarySensor { .. } => "binary_sensor",
        CapabilityKind::Switch => "switch",
        CapabilityKind::Button => "button",
        CapabilityKind::Light { .. } => "light",
        CapabilityKind::Number { .. } => "number",
        CapabilityKind::Select { .. } => "select",
        CapabilityKind::Cover => "cover",
        CapabilityKind::Climate => "climate",
    }
}

fn capability_device_class(kind: &CapabilityKind) -> Option<String> {
    match kind {
        CapabilityKind::Sensor {
            device_class: Some(class),
        } => Some(class.as_str().to_string()),
        CapabilityKind::BinarySensor {
            device_class: Some(class),
        } => Some(class.as_str().to_string()),
        _ => None,
    }
}

fn measurement_for_capability(kind: &CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Switch => "switch",
        CapabilityKind::Button => "switch",
        _ => "sensor",
    }
}

fn fields_from_state_value(
    value: &Value,
    kind: &CapabilityKind,
) -> BTreeMap<String, DataPointField> {
    let mut fields = BTreeMap::new();

    match kind {
        CapabilityKind::Switch | CapabilityKind::Button => {
            if let Some(v) = parse_bool_like(value) {
                fields.insert("value".to_string(), DataPointField::Bool(v));
                return fields;
            }
        }
        _ => {}
    }

    if let Some(num) = value.as_f64() {
        fields.insert("value".to_string(), DataPointField::Number(num));
        return fields;
    }

    if let Some(v) = parse_bool_like(value) {
        fields.insert("value".to_string(), DataPointField::Bool(v));
        return fields;
    }

    if let Some(text) = value.as_str() {
        fields.insert("value".to_string(), DataPointField::Text(text.to_string()));
    }

    fields
}

fn parse_bool_like(value: &Value) -> Option<bool> {
    if let Some(v) = value.as_bool() {
        return Some(v);
    }

    let text = value.as_str()?.trim().to_ascii_lowercase();
    match text.as_str() {
        "on" | "true" | "1" | "press" => Some(true),
        "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn availability_code(status: &hs_device_contracts::Availability) -> u8 {
    match status {
        hs_device_contracts::Availability::Offline => 0,
        hs_device_contracts::Availability::Degraded => 1,
        hs_device_contracts::Availability::Online => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use hs_device_contracts::{
        CapabilityDescriptor, CapabilityKind, DeviceDescriptor, DiscoveryMessage, StateMessage,
    };
    use hs_eventbus_api::{DiscoveryKey, EventProcessor};
    use serde_json::json;
    use tokio::sync::RwLock;

    use crate::{DataPoint, LoggerConfig, PointWriter};

    use super::CoreMetadata;

    #[derive(Default)]
    struct CaptureWriter {
        points: Arc<RwLock<Vec<DataPoint>>>,
    }

    #[async_trait]
    impl PointWriter for CaptureWriter {
        async fn write_points(&self, point_list: Vec<DataPoint>) -> Result<()> {
            self.points.write().await.extend(point_list);
            Ok(())
        }
    }

    fn test_discovery(capability_ids: &[&str]) -> DiscoveryMessage {
        DiscoveryMessage {
            device: DeviceDescriptor {
                service_id: "service-1".to_string(),
                device_id: "device-1".to_string(),
                manufacturer: "test".to_string(),
                model: "model".to_string(),
                name: "name".to_string(),
                sw_version: None,
            },
            capabilities: capability_ids
                .iter()
                .map(|id| CapabilityDescriptor {
                    capability_id: (*id).to_string(),
                    kind: if *id == "power" {
                        CapabilityKind::Switch
                    } else {
                        CapabilityKind::Sensor { device_class: None }
                    },
                    friendly_name: (*id).to_string(),
                    unit_of_measurement: None,
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn discovery_with_multiple_capabilities_writes_each_state() {
        let writer = Arc::new(CaptureWriter::default());
        let core = CoreMetadata::new(writer.clone(), LoggerConfig::default());

        core.on_discovery(
            DiscoveryKey::from("key-1"),
            test_discovery(&["power", "temperature"]),
        )
        .await;

        core.on_state(StateMessage {
            device_id: "device-1".to_string(),
            capability_id: "power".to_string(),
            value: json!("ON"),
            observed_ms: 1,
        })
        .await;

        core.on_state(StateMessage {
            device_id: "device-1".to_string(),
            capability_id: "temperature".to_string(),
            value: json!(21.5),
            observed_ms: 2,
        })
        .await;

        let points = writer.points.read().await;
        assert_eq!(points.len(), 2);

        let capability_ids: Vec<String> = points
            .iter()
            .filter_map(|point| point.tags.get("capability_id").cloned())
            .collect();
        assert!(capability_ids.contains(&"power".to_string()));
        assert!(capability_ids.contains(&"temperature".to_string()));
    }

    #[tokio::test]
    async fn tombstone_removes_all_capabilities_for_discovery_key() {
        let writer = Arc::new(CaptureWriter::default());
        let core = CoreMetadata::new(writer.clone(), LoggerConfig::default());
        let discovery_key = DiscoveryKey::from("key-1");

        core.on_discovery(discovery_key.clone(), test_discovery(&["power", "temperature"]))
            .await;
        core.on_tombstone(discovery_key).await;

        core.on_state(StateMessage {
            device_id: "device-1".to_string(),
            capability_id: "power".to_string(),
            value: json!("ON"),
            observed_ms: 1,
        })
        .await;

        core.on_state(StateMessage {
            device_id: "device-1".to_string(),
            capability_id: "temperature".to_string(),
            value: json!(21.5),
            observed_ms: 2,
        })
        .await;

        let points = writer.points.read().await;
        assert!(points.is_empty());
    }
}
