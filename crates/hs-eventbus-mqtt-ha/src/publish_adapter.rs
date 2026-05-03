use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{
    AvailabilityMessage, CapabilityDescriptor, CommandMessage, DeviceDescriptor, DiscoveryMessage,
    StateMessage,
};
use hs_eventbus_api::{CommandSubscriber, EventBusAdapter};
use opentelemetry::KeyValue;
use rumqttc::v5::mqttbytes::v5::PublishProperties;
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::AsyncClient;
use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::{
    metrics::mqtt_metrics,
    command::{supports_commands, CommandRoute, CommandRoutes},
    config::HomeAssistantMqttConfig,
    payloads::{availability_payload, discovery_payload, state_payload},
    topics::{availability_topic, command_topic, config_topic, state_topic},
    transport::{create_client, spawn_command_loop},
};

#[derive(Clone)]
pub struct HomeAssistantMqttPublishAdapter {
    config: HomeAssistantMqttConfig,
    client: AsyncClient,
    command_routes: CommandRoutes,
    commands_tx: broadcast::Sender<CommandMessage>,
}

impl HomeAssistantMqttPublishAdapter {
    pub async fn connect(config: HomeAssistantMqttConfig) -> Result<Self> {
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

            let topic = command_topic(
                &self.config.node_id,
                &device.device_id,
                &capability.capability_id,
            );
            self.client
                .subscribe(topic.clone(), QoS::AtLeastOnce)
                .await?;
            self.command_routes.write().await.insert(
                topic,
                CommandRoute {
                    device_id: device.device_id.clone(),
                    capability_id: capability.capability_id.clone(),
                },
            );
        }

        Ok(self.commands_tx.subscribe())
    }
}

#[async_trait]
impl CommandSubscriber for HomeAssistantMqttPublishAdapter {
    async fn subscribe_device_commands(
        &self,
        device: &DeviceDescriptor,
        capabilities: &[CapabilityDescriptor],
    ) -> Result<broadcast::Receiver<CommandMessage>> {
        for capability in capabilities {
            if !supports_commands(&capability.kind) {
                continue;
            }

            let topic = command_topic(
                &self.config.node_id,
                &device.device_id,
                &capability.capability_id,
            );
            self.client
                .subscribe(topic.clone(), QoS::AtLeastOnce)
                .await?;
            self.command_routes.write().await.insert(
                topic,
                CommandRoute {
                    device_id: device.device_id.clone(),
                    capability_id: capability.capability_id.clone(),
                },
            );
        }

        Ok(self.commands_tx.subscribe())
    }
}

#[async_trait]
impl EventBusAdapter for HomeAssistantMqttPublishAdapter {
    fn adapter_name(&self) -> &'static str {
        "mqtt-home-assistant"
    }

    async fn publish_discovery(&self, discovery: &DiscoveryMessage) -> Result<()> {
        let availability_topic = availability_topic(
            &self.config.node_id,
            &self.config.client_id,
            &self.config.availability_session,
        );

        for capability in &discovery.capabilities {
            let topic = config_topic(
                &self.config.discovery_prefix,
                &self.config.node_id,
                capability,
                &discovery.device.device_id,
            );
            let payload = discovery_payload(
                discovery,
                capability,
                &self.config.node_id,
                &availability_topic,
            );

            let result = self
                .client
                .publish(topic.clone(), QoS::AtLeastOnce, true, payload.to_string())
                .await;

            let outcome = if result.is_ok() { "ok" } else { "error" };
            mqtt_metrics().publishes_total.add(
                1,
                &[KeyValue::new("topic", topic.clone()), KeyValue::new("outcome", outcome)],
            );
            result?;

            info!(
                topic = %topic,
                device_id = %discovery.device.device_id,
                capability_id = %capability.capability_id,
                "published Home Assistant discovery config"
            );
        }

        Ok(())
    }

    async fn publish_state(&self, state: &StateMessage) -> Result<()> {
        let topic = state_topic(&self.config.node_id, &state.device_id, &state.capability_id);
        let payload = state_payload(state);

        let result = self
            .client
            .publish(topic.clone(), QoS::AtLeastOnce, false, payload)
            .await;

        let outcome = if result.is_ok() { "ok" } else { "error" };
        mqtt_metrics().publishes_total.add(
            1,
            &[KeyValue::new("topic", topic.clone()), KeyValue::new("outcome", outcome)],
        );
        result?;

        debug!(topic = %topic, "published state update");
        Ok(())
    }

    async fn publish_availability(&self, availability: &AvailabilityMessage) -> Result<()> {
        let topic = availability_topic(
            &self.config.node_id,
            &self.config.client_id,
            &self.config.availability_session,
        );
        let payload = availability_payload(&availability.status);

        let result = if let Some(expiry_secs) = self.config.availability_message_expiry_secs {
            let properties = PublishProperties {
                message_expiry_interval: Some(expiry_secs),
                ..PublishProperties::default()
            };
            self.client
                .publish_with_properties(topic.clone(), QoS::AtLeastOnce, true, payload, properties)
                .await
        } else {
            self.client
                .publish(topic.clone(), QoS::AtLeastOnce, true, payload)
                .await
        };

        let outcome = if result.is_ok() { "ok" } else { "error" };
        mqtt_metrics().publishes_total.add(
            1,
            &[KeyValue::new("topic", topic.clone()), KeyValue::new("outcome", outcome)],
        );
        result?;

        info!(topic = %topic, status = %payload, device_id = %availability.device_id, "published availability");
        Ok(())
    }
}
