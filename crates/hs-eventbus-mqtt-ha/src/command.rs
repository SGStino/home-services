use std::{collections::HashMap, sync::Arc};

use hs_device_contracts::{CapabilityKind, CommandMessage};
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct CommandRoute {
    pub device_id: String,
    pub capability_id: String,
}

pub type CommandRoutes = Arc<RwLock<HashMap<String, CommandRoute>>>;

pub fn supports_commands(kind: &CapabilityKind) -> bool {
    matches!(kind, CapabilityKind::Switch | CapabilityKind::Button)
}

pub fn parse_command_payload(bytes: &[u8]) -> serde_json::Value {
    let payload_text = String::from_utf8(bytes.to_vec()).unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&payload_text)
        .unwrap_or(serde_json::Value::String(payload_text))
}

pub fn into_command_message(route: CommandRoute, payload: serde_json::Value) -> CommandMessage {
    CommandMessage {
        device_id: route.device_id,
        capability_id: route.capability_id,
        payload,
    }
}
