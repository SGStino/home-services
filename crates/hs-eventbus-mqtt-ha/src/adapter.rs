use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use hs_contracts::{
    Availability, AvailabilityMessage, CapabilityDescriptor, CapabilityKind, CommandMessage,
    DeviceDescriptor, DiscoveryMessage, StateMessage,
};
use hs_eventbus_api::EventBusAdapter;
use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Packet, QoS};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use crate::{
    config::HomeAssistantMqttConfig,
    payloads::{availability_payload, discovery_payload, state_payload},
    topics::{availability_topic, command_topic, config_topic, state_topic},
};

#[derive(Clone)]
pub struct HomeAssistantMqttAdapter {
    config: HomeAssistantMqttConfig,
    client: AsyncClient,
    command_routes: Arc<RwLock<HashMap<String, (String, String)>>>,
    commands_tx: broadcast::Sender<CommandMessage>,
}

impl HomeAssistantMqttAdapter {
    pub async fn connect(config: HomeAssistantMqttConfig) -> Result<Self> {
        let mut options = MqttOptions::new(
            config.client_id.clone(),
            config.broker_host.clone(),
            config.broker_port,
        );
        options.set_keep_alive(Duration::from_secs(10));

        let lwt_topic = availability_topic(&config.node_id, &config.lwt_device_id);
        options.set_last_will(LastWill::new(
            lwt_topic,
            availability_payload(&Availability::Offline),
            QoS::AtLeastOnce,
            true,
        ));

        let (client, mut event_loop) = AsyncClient::new(options, 50);
        let routes: Arc<RwLock<HashMap<String, (String, String)>>> = Arc::new(RwLock::new(HashMap::new()));
        let routes_for_loop = Arc::clone(&routes);
        let (commands_tx, _) = broadcast::channel(128);
        let commands_tx_for_loop = commands_tx.clone();

        tokio::spawn(async move {
            loop {
                match event_loop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(msg))) => {
                        let topic = msg.topic.clone();
                        let route = routes_for_loop.read().await.get(&topic).cloned();
                        if let Some((device_id, capability_id)) = route {
                            let payload_text = String::from_utf8(msg.payload.to_vec())
                                .unwrap_or_else(|_| String::new());
                            let payload = serde_json::from_str::<serde_json::Value>(&payload_text)
                                .unwrap_or_else(|_| serde_json::Value::String(payload_text));
                            let command = CommandMessage {
                                device_id,
                                capability_id,
                                payload,
                            };

                            if commands_tx_for_loop.send(command).is_err() {
                                warn!("command received but no active command receiver");
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(err) => {
                        error!(error = %err, "mqtt event loop error");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });

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
            let supports_commands = matches!(capability.kind, CapabilityKind::Switch | CapabilityKind::Button);
            if !supports_commands {
                continue;
            }

            let topic = command_topic(&self.config.node_id, &device.device_id, &capability.capability_id);
            self.client.subscribe(topic.clone(), QoS::AtLeastOnce).await?;
            self.command_routes.write().await.insert(
                topic,
                (device.device_id.clone(), capability.capability_id.clone()),
            );
        }

        Ok(self.commands_tx.subscribe())
    }
}

#[async_trait]
impl EventBusAdapter for HomeAssistantMqttAdapter {
    fn adapter_name(&self) -> &'static str {
        "mqtt-home-assistant"
    }

    async fn publish_discovery(&self, discovery: &DiscoveryMessage) -> Result<()> {
        for capability in &discovery.capabilities {
            let topic = config_topic(
                &self.config.discovery_prefix,
                &self.config.node_id,
                capability,
                &discovery.device.device_id,
            );
            let payload = discovery_payload(discovery, capability, &self.config.node_id);

            self.client
                .publish(topic.clone(), QoS::AtLeastOnce, true, payload.to_string())
                .await?;

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

        self.client
            .publish(topic.clone(), QoS::AtLeastOnce, false, payload)
            .await?;

        debug!(topic = %topic, "published state update");
        Ok(())
    }

    async fn publish_availability(&self, availability: &AvailabilityMessage) -> Result<()> {
        let topic = availability_topic(&self.config.node_id, &availability.device_id);
        let payload = availability_payload(&availability.status);

        self.client
            .publish(topic.clone(), QoS::AtLeastOnce, true, payload)
            .await?;

        info!(topic = %topic, status = %payload, "published availability");
        Ok(())
    }
}
