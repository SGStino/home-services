use serde::{Deserialize, Serialize};

use crate::capability::CapabilityDescriptor;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceDescriptor {
    pub service_id: String,
    pub device_id: String,
    pub manufacturer: String,
    pub model: String,
    pub name: String,
    pub sw_version: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub device: DeviceDescriptor,
    pub capabilities: Vec<CapabilityDescriptor>,
    #[serde(default)]
    pub availability_topic: Option<String>,
}
