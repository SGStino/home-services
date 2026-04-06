use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{AvailabilityMessage, DiscoveryMessage, StateMessage};

#[async_trait]
pub trait EventBusAdapter: Send + Sync {
    fn adapter_name(&self) -> &'static str;

    async fn publish_discovery(&self, discovery: &DiscoveryMessage) -> Result<()>;

    async fn publish_state(&self, state: &StateMessage) -> Result<()>;

    async fn publish_availability(&self, availability: &AvailabilityMessage) -> Result<()>;
}
