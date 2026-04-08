use std::time::Duration;

use hs_device_contracts::Availability;
use rumqttc::{AsyncClient, Event, EventLoop, LastWill, MqttOptions, Packet, QoS};
use tokio::sync::broadcast;
use tracing::{error, warn};

use crate::{
    command::{parse_command_messages, CommandRoutes},
    config::SparkplugBConfig,
    payloads::availability_payload,
    topics::state_topic,
};

pub fn build_mqtt_options(config: &SparkplugBConfig) -> MqttOptions {
    let mut options = MqttOptions::new(
        config.client_id.clone(),
        config.broker_host.clone(),
        config.broker_port,
    );
    options.set_keep_alive(Duration::from_secs(10));

    let lwt_topic = state_topic(&config.edge_node_id);
    options.set_last_will(LastWill::new(
        lwt_topic,
        availability_payload(&Availability::Offline),
        QoS::AtLeastOnce,
        true,
    ));

    options
}

pub fn create_client(config: &SparkplugBConfig) -> (AsyncClient, EventLoop) {
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
                        match parse_command_messages(&route, &msg.payload) {
                            Ok(commands) => {
                                for command in commands {
                                    if commands_tx.send(command).is_err() {
                                        warn!("command received but no active command receiver");
                                    }
                                }
                            }
                            Err(err) => {
                                warn!(error = %err, topic = %msg.topic, "invalid Sparkplug DCMD payload");
                            }
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
