use anyhow::Result;
use hs_device_contracts::{
    AvailabilityMessage, CapabilityDescriptor, DeviceDescriptor, DiscoveryMessage, StateMessage,
};
use hs_eventbus_api::EventBusAdapter;
use opentelemetry::KeyValue;
use std::time::Instant;
use tracing::{debug, info, info_span, Instrument};

use crate::runtime_metrics::runtime_metrics;

pub struct DeviceRuntime<A>
where
    A: EventBusAdapter,
{
    adapter: A,
    service_id: String,
}

impl<A> DeviceRuntime<A>
where
    A: EventBusAdapter,
{
    pub fn new(service_id: impl Into<String>, adapter: A) -> Self {
        Self {
            adapter,
            service_id: service_id.into(),
        }
    }

    pub async fn announce_device(
        &self,
        device: DeviceDescriptor,
        capabilities: Vec<CapabilityDescriptor>,
    ) -> Result<()> {
        let publish_span = info_span!(
            "hs_core.publish_discovery",
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %device.device_id,
        );

        info!(
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %device.device_id,
            "publishing discovery"
        );

        let started_at = Instant::now();
        let adapter_name = self.adapter.adapter_name();
        let result = self
            .adapter
            .publish_discovery(&DiscoveryMessage {
                device,
                capabilities,
                availability_topic: None,
            })
            .instrument(publish_span)
            .await;

        self.record_publish("discovery", adapter_name, started_at, &result);
        result
    }

    pub async fn publish_availability(&self, availability: AvailabilityMessage) -> Result<()> {
        let publish_span = info_span!(
            "hs_core.publish_availability",
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %availability.device_id,
        );

        info!(
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %availability.device_id,
            "publishing availability"
        );

        let started_at = Instant::now();
        let adapter_name = self.adapter.adapter_name();
        let result = self
            .adapter
            .publish_availability(&availability)
            .instrument(publish_span)
            .await;

        self.record_publish("availability", adapter_name, started_at, &result);
        result
    }

    pub async fn publish_state(&self, state: StateMessage) -> Result<()> {
        let publish_span = info_span!(
            "hs_core.publish_state",
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %state.device_id,
            capability_id = %state.capability_id,
        );

        debug!(
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %state.device_id,
            capability_id = %state.capability_id,
            "publishing state"
        );

        let started_at = Instant::now();
        let adapter_name = self.adapter.adapter_name();
        let result = self
            .adapter
            .publish_state(&state)
            .instrument(publish_span)
            .await;

        self.record_publish("state", adapter_name, started_at, &result);
        result
    }

    fn record_publish(
        &self,
        message_kind: &'static str,
        adapter_name: &str,
        started_at: Instant,
        result: &Result<()>,
    ) {
        let metrics = runtime_metrics();

        let outcome = if result.is_ok() { "ok" } else { "error" };
        let attrs = [
            KeyValue::new("service.id", self.service_id.clone()),
            KeyValue::new("adapter", adapter_name.to_owned()),
            KeyValue::new("message.kind", message_kind),
            KeyValue::new("outcome", outcome),
        ];

        metrics.publish_events.add(1, &attrs);
        metrics
            .publish_latency_seconds
            .record(started_at.elapsed().as_secs_f64(), &attrs);
    }
}
