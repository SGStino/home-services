use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Context, Result};
use esphome_native_api::{
    parser::{message_to_num, parse_proto_message, proto_to_vec, ProtoMessage},
    proto::{
        ButtonCommandRequest, DeviceInfoRequest, HelloRequest, HelloResponse,
        ListEntitiesDoneResponse, ListEntitiesRequest, PingResponse, SubscribeStatesRequest,
        SwitchCommandRequest,
    },
};
use hs_device_contracts::{CapabilityDescriptor, CapabilityKind, CommandMessage, DeviceClass};
use prost::{decode_length_delimiter, encode_length_delimiter};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::{broadcast, mpsc},
};
use tracing::{info, warn};

use crate::{config::EsphomeApiConfig, time::now_unix_ms};

pub struct EsphomeBridge {
    tx: mpsc::Sender<ProtoMessage>,
    rx: broadcast::Receiver<ProtoMessage>,
    capabilities: Vec<CapabilityDescriptor>,
    state_routes: HashMap<u32, StateRoute>,
    command_routes: HashMap<String, CommandRoute>,
}

#[derive(Clone)]
pub struct StateUpdate {
    pub capability_id: String,
    pub value: Value,
    pub observed_ms: u64,
}

#[derive(Clone)]
enum CommandRoute {
    Switch { key: u32, device_id: u32 },
    Button { key: u32, device_id: u32 },
}

#[derive(Clone)]
struct StateRoute {
    capability_id: String,
}

impl EsphomeBridge {
    pub async fn connect(config: &EsphomeApiConfig) -> Result<Self> {
        if config.encryption_key.is_some() {
            return Err(anyhow!(
                "encrypted ESPHome API is not wired yet in hs-service-device-esphome; plaintext API nodes are supported first"
            ));
        }

        info!(host = %config.host, port = config.port, "connecting to ESPHome native API");
        let stream = TcpStream::connect((config.host.as_str(), config.port))
            .await
            .with_context(|| format!("failed connecting to ESPHome API at {}:{}", config.host, config.port))?;

        let (read_half, write_half) = stream.into_split();
        let (tx, write_rx) = mpsc::channel::<ProtoMessage>(64);
        let (broadcast_tx, mut rx) = broadcast::channel::<ProtoMessage>(256);

        tokio::spawn(write_loop(write_half, write_rx));
        tx.send(ProtoMessage::HelloRequest(HelloRequest {
            client_info: config.client_name.clone(),
            api_version_major: 1,
            api_version_minor: 10,
        }))
        .await
        .context("failed to send ESPHome hello request")?;

        let (read_half, hello_frame) = read_half_read_once(read_half).await?;
        let hello = read_plaintext_message(hello_frame)?;
        match hello {
            ProtoMessage::HelloResponse(HelloResponse { api_version_major, api_version_minor, server_info, name }) => {
                info!(api_version_major, api_version_minor, server_info = %server_info, name = %name, "received ESPHome hello response");
            }
            other => {
                return Err(anyhow!("expected HelloResponse from ESPHome node, got {other:?}"));
            }
        }

        spawn_read_loop(read_half, Vec::new(), broadcast_tx.clone(), tx.clone());

        tx.send(ProtoMessage::DeviceInfoRequest(DeviceInfoRequest {}))
            .await
            .context("failed to request ESPHome device info")?;
        info!("ESPHome API session started; requesting entity list");
        tx.send(ProtoMessage::ListEntitiesRequest(ListEntitiesRequest {}))
            .await
            .context("failed to request ESPHome entities")?;

        let mut capabilities = Vec::new();
        let mut seen_capabilities = HashSet::new();
        let mut state_routes = HashMap::new();
        let mut command_routes = HashMap::new();

        loop {
            let message = rx.recv().await.map_err(|error| {
                anyhow!(
                    "ESPHome API closed during entity listing at {}:{}: {}",
                    config.host,
                    config.port,
                    error
                )
            })?;
            match message {
                ProtoMessage::ListEntitiesSensorResponse(entity) => {
                    if !seen_capabilities.insert(entity.object_id.clone()) {
                        continue;
                    }

                    capabilities.push(CapabilityDescriptor {
                        capability_id: entity.object_id.clone(),
                        kind: CapabilityKind::Sensor {
                            device_class: if entity.device_class.is_empty() {
                                None
                            } else {
                                Some(DeviceClass::new(entity.device_class.clone()))
                            },
                        },
                        friendly_name: entity.name.clone(),
                        unit_of_measurement: if entity.unit_of_measurement.is_empty() {
                            None
                        } else {
                            Some(entity.unit_of_measurement.clone())
                        },
                    });

                    state_routes.insert(
                        entity.key,
                        StateRoute {
                            capability_id: entity.object_id,
                        },
                    );
                }
                ProtoMessage::ListEntitiesBinarySensorResponse(entity) => {
                    if !seen_capabilities.insert(entity.object_id.clone()) {
                        continue;
                    }

                    capabilities.push(CapabilityDescriptor {
                        capability_id: entity.object_id.clone(),
                        kind: CapabilityKind::BinarySensor {
                            device_class: if entity.device_class.is_empty() {
                                None
                            } else {
                                Some(DeviceClass::new(entity.device_class.clone()))
                            },
                        },
                        friendly_name: entity.name.clone(),
                        unit_of_measurement: None,
                    });

                    state_routes.insert(
                        entity.key,
                        StateRoute {
                            capability_id: entity.object_id,
                        },
                    );
                }
                ProtoMessage::ListEntitiesTextSensorResponse(entity) => {
                    if !seen_capabilities.insert(entity.object_id.clone()) {
                        continue;
                    }

                    capabilities.push(CapabilityDescriptor {
                        capability_id: entity.object_id.clone(),
                        kind: CapabilityKind::Sensor { device_class: None },
                        friendly_name: entity.name.clone(),
                        unit_of_measurement: None,
                    });

                    state_routes.insert(
                        entity.key,
                        StateRoute {
                            capability_id: entity.object_id,
                        },
                    );
                }
                ProtoMessage::ListEntitiesSwitchResponse(entity) => {
                    if !seen_capabilities.insert(entity.object_id.clone()) {
                        continue;
                    }

                    capabilities.push(CapabilityDescriptor {
                        capability_id: entity.object_id.clone(),
                        kind: CapabilityKind::Switch,
                        friendly_name: entity.name.clone(),
                        unit_of_measurement: None,
                    });

                    state_routes.insert(
                        entity.key,
                        StateRoute {
                            capability_id: entity.object_id.clone(),
                        },
                    );

                    command_routes.insert(
                        entity.object_id,
                        CommandRoute::Switch {
                            key: entity.key,
                            device_id: entity.device_id,
                        },
                    );
                }
                ProtoMessage::ListEntitiesButtonResponse(entity) => {
                    if !seen_capabilities.insert(entity.object_id.clone()) {
                        continue;
                    }

                    capabilities.push(CapabilityDescriptor {
                        capability_id: entity.object_id.clone(),
                        kind: CapabilityKind::Button,
                        friendly_name: entity.name.clone(),
                        unit_of_measurement: None,
                    });

                    command_routes.insert(
                        entity.object_id,
                        CommandRoute::Button {
                            key: entity.key,
                            device_id: entity.device_id,
                        },
                    );
                }
                ProtoMessage::ListEntitiesDoneResponse(ListEntitiesDoneResponse {}) => {
                    break;
                }
                _ => {}
            }
        }

        tx.send(ProtoMessage::SubscribeStatesRequest(SubscribeStatesRequest {}))
            .await
            .context("failed to subscribe to ESPHome state stream")?;
        info!(capability_count = capabilities.len(), "subscribed to ESPHome state stream");

        Ok(Self {
            tx,
            rx,
            capabilities,
            state_routes,
            command_routes,
        })
    }

    pub fn capabilities(&self) -> &[CapabilityDescriptor] {
        &self.capabilities
    }

    pub fn drain_state_updates(&mut self) -> Vec<StateUpdate> {
        let mut updates = Vec::new();
        loop {
            let message = match self.rx.try_recv() {
                Ok(message) => message,
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Closed) => break,
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    warn!(skipped, "lagged while receiving ESPHome state updates");
                    continue;
                }
            };

            match message {
                ProtoMessage::SensorStateResponse(state) => {
                    if state.missing_state {
                        continue;
                    }
                    if let Some(route) = self.state_routes.get(&state.key) {
                        updates.push(StateUpdate {
                            capability_id: route.capability_id.clone(),
                            value: json!(state.state),
                            observed_ms: now_unix_ms(),
                        });
                    }
                }
                ProtoMessage::BinarySensorStateResponse(state) => {
                    if state.missing_state {
                        continue;
                    }
                    if let Some(route) = self.state_routes.get(&state.key) {
                        updates.push(StateUpdate {
                            capability_id: route.capability_id.clone(),
                            value: json!(state.state),
                            observed_ms: now_unix_ms(),
                        });
                    }
                }
                ProtoMessage::TextSensorStateResponse(state) => {
                    if state.missing_state {
                        continue;
                    }
                    if let Some(route) = self.state_routes.get(&state.key) {
                        updates.push(StateUpdate {
                            capability_id: route.capability_id.clone(),
                            value: json!(state.state),
                            observed_ms: now_unix_ms(),
                        });
                    }
                }
                ProtoMessage::SwitchStateResponse(state) => {
                    if let Some(route) = self.state_routes.get(&state.key) {
                        let on_off = if state.state { "ON" } else { "OFF" };
                        updates.push(StateUpdate {
                            capability_id: route.capability_id.clone(),
                            value: json!(on_off),
                            observed_ms: now_unix_ms(),
                        });
                    }
                }
                _ => {}
            }
        }
        updates
    }

    pub async fn forward_command(&self, command: &CommandMessage) -> Result<bool> {
        let Some(route) = self.command_routes.get(&command.capability_id) else {
            return Ok(false);
        };

        match route {
            CommandRoute::Switch { key, device_id } => {
                let state = payload_is_on(&command.payload);
                self.tx
                    .send(ProtoMessage::SwitchCommandRequest(SwitchCommandRequest {
                        key: *key,
                        state,
                        device_id: *device_id,
                    }))
                    .await
                    .context("failed sending switch command to ESPHome")?;
            }
            CommandRoute::Button { key, device_id } => {
                self.tx
                    .send(ProtoMessage::ButtonCommandRequest(ButtonCommandRequest {
                        key: *key,
                        device_id: *device_id,
                    }))
                    .await
                    .context("failed sending button command to ESPHome")?;
            }
        }

        Ok(true)
    }
}

fn payload_is_on(payload: &Value) -> bool {
    match payload {
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_i64() == Some(1),
        Value::String(value) => {
            let value = value.trim().to_ascii_uppercase();
            value == "ON" || value == "1" || value == "TRUE"
        }
        _ => false,
    }
}

async fn write_loop<W>(mut writer: W, mut rx: mpsc::Receiver<ProtoMessage>)
where
    W: AsyncWrite + Unpin,
{
    while let Some(message) = rx.recv().await {
        if let Err(error) = write_plaintext_message(&mut writer, &message).await {
            warn!(error = %error, "failed writing ESPHome message");
            break;
        }
    }
}

fn spawn_read_loop(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    mut pending_frames: Vec<Vec<u8>>,
    broadcast_tx: broadcast::Sender<ProtoMessage>,
    write_tx: mpsc::Sender<ProtoMessage>,
) {
    tokio::spawn(async move {
        loop {
            let message = if let Some(frame) = pending_frames.pop() {
                match read_plaintext_message(frame) {
                    Ok(message) => message,
                    Err(error) => {
                        warn!(error = %error, "failed decoding buffered ESPHome frame");
                        break;
                    }
                }
            } else {
                match read_plaintext_frame(&mut reader).await.and_then(read_plaintext_message) {
                    Ok(message) => message,
                    Err(error) => {
                        warn!(error = %error, "ESPHome read loop stopped");
                        break;
                    }
                }
            };

            if let ProtoMessage::PingRequest(_) = message {
                let _ = write_tx.send(ProtoMessage::PingResponse(PingResponse {})).await;
                continue;
            }

            let _ = broadcast_tx.send(message);
        }
    });
}

async fn read_half_read_once(
    mut reader: tokio::net::tcp::OwnedReadHalf,
) -> Result<(tokio::net::tcp::OwnedReadHalf, Vec<u8>)> {
    let frame = read_plaintext_frame(&mut reader).await?;
    Ok((reader, frame))
}

async fn write_plaintext_message<W>(writer: &mut W, message: &ProtoMessage) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let message_num = message_to_num(message)
        .map_err(|error| anyhow!("failed to encode ESPHome message type: {}", error))?;
    let payload = proto_to_vec(message)
        .map_err(|error| anyhow!("failed to encode ESPHome message payload: {}", error))?;
    let item = [[message_num].as_slice(), payload.as_slice()].concat();

    let mut frame = vec![0u8];
    let mut len_buf = Vec::new();
    encode_length_delimiter(item.len() - 1, &mut len_buf)?;
    frame.extend(len_buf);
    frame.extend(item);

    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_plaintext_frame<R>(reader: &mut R) -> Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut marker = [0u8; 1];
    reader.read_exact(&mut marker).await?;
    if marker[0] != 0 {
        return Err(anyhow!("expected plaintext ESPHome frame marker 0, got {}", marker[0]));
    }

    let mut len_bytes = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).await?;
        len_bytes.push(byte[0]);
        if byte[0] & 0x80 == 0 {
            break;
        }
        if len_bytes.len() > 4 {
            return Err(anyhow!("ESPHome plaintext varint length marker too long"));
        }
    }

    let length = decode_length_delimiter(&len_bytes[..])? + 1;
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;
    Ok(payload)
}

fn read_plaintext_message(frame: Vec<u8>) -> Result<ProtoMessage> {
    let (message_type, payload) = frame
        .split_first()
        .ok_or_else(|| anyhow!("received empty ESPHome frame"))?;
    let message = parse_proto_message(*message_type as usize, payload)
        .map_err(|error| anyhow!("failed parsing ESPHome message type {}: {}", message_type, error))?;
    Ok(message)
}
