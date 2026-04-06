use std::time::Duration;

use hs_device_contracts::Availability;
use rumqttc::{AsyncClient, Event, EventLoop, LastWill, MqttOptions, Packet, QoS};
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

    let lwt_topic = availability_topic(&config.node_id);
    options.set_last_will(LastWill::new(
        lwt_topic,
        availability_payload(&Availability::Offline),
        QoS::AtLeastOnce,
        true,
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
                    let route = routes.read().await.get(&msg.topic).cloned();
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
