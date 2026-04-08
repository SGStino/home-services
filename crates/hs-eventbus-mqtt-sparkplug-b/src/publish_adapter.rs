use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{
    AvailabilityMessage, CapabilityDescriptor, CommandMessage, DeviceDescriptor, DiscoveryMessage,
    StateMessage,
};
use hs_eventbus_api::{CommandSubscriber, EventBusAdapter};
use opentelemetry::KeyValue;
use rumqttc::{AsyncClient, QoS};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info};

use crate::{
    command::{expected_command_type, supports_commands, CommandRoute, CommandRoutes},
    config::SparkplugBConfig,
    metrics::sparkplug_metrics,
    payloads::{availability_payload, discovery_payload, state_payload},
    topics::{dbirth_topic, dcmd_topic, ddata_topic, state_topic},
    transport::{create_client, spawn_command_loop},
};

#[derive(Clone)]
pub struct SparkplugBMqttPublishAdapter {
    config: SparkplugBConfig,
    client: AsyncClient,
    command_routes: CommandRoutes,
    commands_tx: broadcast::Sender<CommandMessage>,
    sequence_by_device: Arc<RwLock<HashMap<String, u64>>>,
}

impl SparkplugBMqttPublishAdapter {
    pub async fn connect(config: SparkplugBConfig) -> Result<Self> {
        let (client, event_loop) = create_client(&config);
        let routes: CommandRoutes =
            std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let (commands_tx, _) = broadcast::channel(128);

        spawn_command_loop(
            event_loop,
            std::sync::Arc::clone(&routes),
            commands_tx.clone(),
        );

        Ok(Self {
            config,
            client,
            command_routes: routes,
            commands_tx,
            sequence_by_device: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn subscribe_device_commands(
        &self,
        device: &DeviceDescriptor,
        capabilities: &[CapabilityDescriptor],
    ) -> Result<broadcast::Receiver<CommandMessage>> {
        for capability in capabilities {
            if !supports_commands(&capability.kind) {
                continue;
            }

            let Some(expected_type) = expected_command_type(&capability.kind) else {
                continue;
            };

            let topic = dcmd_topic(
                &self.config.group_id,
                &self.config.edge_node_id,
                &device.device_id,
            );
            self.client
                .subscribe(topic.clone(), QoS::AtLeastOnce)
                .await?;
            let mut routes = self.command_routes.write().await;
            let route = routes.entry(topic).or_insert_with(|| CommandRoute {
                device_id: device.device_id.clone(),
                expected_capabilities: HashMap::new(),
            });
            route
                .expected_capabilities
                .insert(capability.capability_id.clone(), expected_type);
        }

        Ok(self.commands_tx.subscribe())
    }
}

#[async_trait]
impl CommandSubscriber for SparkplugBMqttPublishAdapter {
    async fn subscribe_device_commands(
        &self,
        device: &DeviceDescriptor,
        capabilities: &[CapabilityDescriptor],
    ) -> Result<broadcast::Receiver<CommandMessage>> {
        self.subscribe_device_commands(device, capabilities).await
    }
}

#[async_trait]
impl EventBusAdapter for SparkplugBMqttPublishAdapter {
    fn adapter_name(&self) -> &'static str {
        "mqtt-sparkplug-b"
    }

    async fn publish_discovery(&self, discovery: &DiscoveryMessage) -> Result<()> {
        let seq = self.next_seq(&discovery.device.device_id).await;
        let topic = dbirth_topic(
            &self.config.group_id,
            &self.config.edge_node_id,
            &discovery.device.device_id,
        );
        let now_ms = current_unix_ms();
        let payload = discovery_payload(discovery, seq, now_ms);

        let result = self
            .client
            .publish(topic.clone(), QoS::AtLeastOnce, true, payload)
            .await;

        let outcome = if result.is_ok() { "ok" } else { "error" };
        sparkplug_metrics().publishes_total.add(
            1,
            &[KeyValue::new("topic", topic.clone()), KeyValue::new("outcome", outcome)],
        );
        result?;

        info!(
            topic = %topic,
            device_id = %discovery.device.device_id,
            "published Sparkplug DBIRTH"
        );

        Ok(())
    }

    async fn publish_state(&self, state: &StateMessage) -> Result<()> {
        let seq = self.next_seq(&state.device_id).await;
        let topic = ddata_topic(
            &self.config.group_id,
            &self.config.edge_node_id,
            &state.device_id,
        );
        let payload = state_payload(state, seq);

        let result = self
            .client
            .publish(topic.clone(), QoS::AtLeastOnce, false, payload)
            .await;

        let outcome = if result.is_ok() { "ok" } else { "error" };
        sparkplug_metrics().publishes_total.add(
            1,
            &[KeyValue::new("topic", topic.clone()), KeyValue::new("outcome", outcome)],
        );
        result?;

        debug!(topic = %topic, "published Sparkplug DDATA update");
        Ok(())
    }

    async fn publish_availability(&self, availability: &AvailabilityMessage) -> Result<()> {
        let topic = state_topic(&self.config.edge_node_id);
        let payload = availability_payload(&availability.status);

        let result = self
            .client
            .publish(topic.clone(), QoS::AtLeastOnce, true, payload)
            .await;

        let outcome = if result.is_ok() { "ok" } else { "error" };
        sparkplug_metrics().publishes_total.add(
            1,
            &[KeyValue::new("topic", topic.clone()), KeyValue::new("outcome", outcome)],
        );
        result?;

        info!(
            topic = %topic,
            status = %payload,
            device_id = %availability.device_id,
            "published Sparkplug node state"
        );
        Ok(())
    }
}

impl SparkplugBMqttPublishAdapter {
    async fn next_seq(&self, device_id: &str) -> u64 {
        let mut guard = self.sequence_by_device.write().await;
        let next = guard.entry(device_id.to_string()).or_insert(0);
        let current = *next;
        *next = (*next + 1) % 256;
        current
    }
}

fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
