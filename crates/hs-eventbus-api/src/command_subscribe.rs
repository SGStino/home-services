use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{CapabilityDescriptor, CommandMessage, DeviceDescriptor};
use tokio::sync::broadcast;

#[async_trait]
pub trait CommandSubscriber: Send + Sync {
    async fn subscribe_device_commands(
        &self,
        device: &DeviceDescriptor,
        capabilities: &[CapabilityDescriptor],
    ) -> Result<broadcast::Receiver<CommandMessage>>;
}
