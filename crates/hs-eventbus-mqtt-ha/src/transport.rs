use std::time::Duration;

use hs_device_contracts::Availability;
use rumqttc::v5::mqttbytes::v5::{LastWill, LastWillProperties, Packet};
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::{AsyncClient, Event, EventLoop, MqttOptions};
use tokio::sync::broadcast;
use tracing::{error, warn};

use crate::{
    command::{into_command_message, parse_command_payload, CommandRoutes},
    config::HomeAssistantMqttConfig,
    payloads::availability_payload,
    topics::availability_topic,
};

pub fn build_mqtt_options(config: &HomeAssistantMqttConfig) -> MqttOptions {
    let mut options = MqttOptions::new(
        config.client_id.clone(),
        config.broker_host.clone(),
        config.broker_port,
    );
    options.set_keep_alive(Duration::from_secs(10));

    let lwt_topic = availability_topic(
        &config.node_id,
        &config.client_id,
        &config.availability_session,
    );
    let lwt_properties = config
        .availability_message_expiry_secs
        .map(|expiry_secs| LastWillProperties {
            delay_interval: None,
            payload_format_indicator: None,
            message_expiry_interval: Some(expiry_secs),
            content_type: None,
            response_topic: None,
            correlation_data: None,
            user_properties: Vec::new(),
        });
    options.set_last_will(LastWill::new(
        lwt_topic,
        availability_payload(&Availability::Offline),
        QoS::AtLeastOnce,
        true,
        lwt_properties,
    ));

    options
}

pub fn create_client(config: &HomeAssistantMqttConfig) -> (AsyncClient, EventLoop) {
    AsyncClient::new(build_mqtt_options(config), 50)
}

pub fn spawn_command_loop(
    mut event_loop: EventLoop,
    routes: CommandRoutes,
    commands_tx: broadcast::Sender<hs_device_contracts::CommandMessage>,
) {
    tokio::spawn(async move {
        loop {
            match event_loop.poll().await {
                Ok(Event::Incoming(Packet::Publish(msg))) => {
                        let topic = String::from_utf8_lossy(&msg.topic);
                        let route = routes.read().await.get(topic.as_ref()).cloned();
                    if let Some(route) = route {
                        let payload = parse_command_payload(&msg.payload);
                        let command = into_command_message(route, payload);

                        if commands_tx.send(command).is_err() {
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
}
