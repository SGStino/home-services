use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use hs_device_contracts::{AvailabilityMessage, DiscoveryMessage, StateMessage};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiscoveryKey(pub Arc<str>);

impl DiscoveryKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DiscoveryKey {
    fn from(value: String) -> Self {
        Self(Arc::<str>::from(value))
    }
}

impl From<&str> for DiscoveryKey {
    fn from(value: &str) -> Self {
        Self(Arc::<str>::from(value))
    }
}

 
/// Observer interface implemented by consumers (e.g. CoreMetadata in the logger).
/// The ingest adapter calls these methods as events arrive from the bus.
#[async_trait]
pub trait EventProcessor: Send + Sync {
    async fn on_discovery(&self, key: DiscoveryKey, event: DiscoveryMessage);
    async fn on_tombstone(&self, key: DiscoveryKey);
    async fn on_state(&self, state: StateMessage);
    async fn on_availability(&self, availability: AvailabilityMessage);
}

/// Observable interface implemented by ingest adapters (e.g. HAMQTTIngestAdapter).
/// Calling `initialize` wires the adapter to push events into `processor`.
#[async_trait]
pub trait IngestAdapter: Send + Sync {
    fn adapter_name(&self) -> &'static str;

    async fn initialize(&self, processor: Arc<dyn EventProcessor>) -> Result<()>;
}
