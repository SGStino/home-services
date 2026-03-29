use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Availability {
    Online,
    Offline,
    Degraded,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvailabilityMessage {
    pub device_id: String,
    pub status: Availability,
    pub detail: String,
}
