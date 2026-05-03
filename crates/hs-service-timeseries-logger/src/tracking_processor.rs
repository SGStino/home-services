use std::sync::Arc;

use async_trait::async_trait;
use hs_device_contracts::{AvailabilityMessage, DiscoveryMessage, StateMessage};
use hs_eventbus_api::{DiscoveryKey, EventProcessor};

use crate::status::LoggerStatus;

#[derive(Clone)]
pub struct TrackingEventProcessor {
    inner: Arc<dyn EventProcessor>,
    status: LoggerStatus,
}

impl TrackingEventProcessor {
    pub fn new(inner: Arc<dyn EventProcessor>, status: LoggerStatus) -> Self {
        Self { inner, status }
    }
}

#[async_trait]
impl EventProcessor for TrackingEventProcessor {
    async fn on_discovery(&self, key: DiscoveryKey, event: DiscoveryMessage) {
        self.status.on_discovery(key.as_str(), &event).await;
        self.inner.on_discovery(key, event).await;
    }

    async fn on_tombstone(&self, key: DiscoveryKey) {
        self.status.on_tombstone(key.as_str()).await;
        self.inner.on_tombstone(key).await;
    }

    async fn on_state(&self, state: StateMessage) {
        self.inner.on_state(state).await;
    }

    async fn on_availability(&self, availability: AvailabilityMessage) {
        self.inner.on_availability(availability).await;
    }
}
