use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateMessage {
    pub device_id: String,
    pub capability_id: String,
    pub value: Value,
    #[serde(alias = "observed_at_unix_ms")]
    pub observed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandMessage {
    pub device_id: String,
    pub capability_id: String,
    pub payload: Value,
}
