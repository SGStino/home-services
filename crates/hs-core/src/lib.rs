use anyhow::Result;
use hs_contracts::{AvailabilityMessage, CapabilityDescriptor, DeviceDescriptor, DiscoveryMessage, StateMessage};
use hs_eventbus_api::EventBusAdapter;
use opentelemetry::{global, metrics::{Counter, Histogram}, KeyValue};
use std::{sync::OnceLock, time::Instant};
use tracing::{info, info_span};

pub mod telemetry;

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
        let span = info_span!(
            "hs_core.publish_discovery",
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %device.device_id,
        );
        let _enter = span.enter();

        let started_at = Instant::now();
        let metrics = runtime_metrics();
        let adapter_name = self.adapter.adapter_name();

        info!(
            service_id = %self.service_id,
            adapter = adapter_name,
            device_id = %device.device_id,
            "publishing discovery"
        );

        let result = self
            .adapter
            .publish_discovery(&DiscoveryMessage {
                device,
                capabilities,
            })
            .await;

        let outcome = if result.is_ok() { "ok" } else { "error" };
        let attrs = [
            KeyValue::new("service.id", self.service_id.clone()),
            KeyValue::new("adapter", adapter_name.to_owned()),
            KeyValue::new("message.kind", "discovery"),
            KeyValue::new("outcome", outcome),
        ];
        metrics.publish_events.add(1, &attrs);
        metrics
            .publish_latency_seconds
            .record(started_at.elapsed().as_secs_f64(), &attrs);

        result
    }

    pub async fn publish_availability(&self, availability: AvailabilityMessage) -> Result<()> {
        let span = info_span!(
            "hs_core.publish_availability",
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %availability.device_id,
        );
        let _enter = span.enter();

        let started_at = Instant::now();
        let metrics = runtime_metrics();
        let adapter_name = self.adapter.adapter_name();

        info!(
            service_id = %self.service_id,
            adapter = adapter_name,
            device_id = %availability.device_id,
            "publishing availability"
        );

        let result = self.adapter.publish_availability(&availability).await;

        let outcome = if result.is_ok() { "ok" } else { "error" };
        let attrs = [
            KeyValue::new("service.id", self.service_id.clone()),
            KeyValue::new("adapter", adapter_name.to_owned()),
            KeyValue::new("message.kind", "availability"),
            KeyValue::new("outcome", outcome),
        ];
        metrics.publish_events.add(1, &attrs);
        metrics
            .publish_latency_seconds
            .record(started_at.elapsed().as_secs_f64(), &attrs);

        result
    }

    pub async fn publish_state(&self, state: StateMessage) -> Result<()> {
        let span = info_span!(
            "hs_core.publish_state",
            service_id = %self.service_id,
            adapter = self.adapter.adapter_name(),
            device_id = %state.device_id,
            capability_id = %state.capability_id,
        );
        let _enter = span.enter();

        let started_at = Instant::now();
        let metrics = runtime_metrics();
        let adapter_name = self.adapter.adapter_name();

        info!(
            service_id = %self.service_id,
            adapter = adapter_name,
            device_id = %state.device_id,
            capability_id = %state.capability_id,
            "publishing state"
        );

        let result = self.adapter.publish_state(&state).await;

        let outcome = if result.is_ok() { "ok" } else { "error" };
        let attrs = [
            KeyValue::new("service.id", self.service_id.clone()),
            KeyValue::new("adapter", adapter_name.to_owned()),
            KeyValue::new("message.kind", "state"),
            KeyValue::new("outcome", outcome),
        ];
        metrics.publish_events.add(1, &attrs);
        metrics
            .publish_latency_seconds
            .record(started_at.elapsed().as_secs_f64(), &attrs);

        result
    }
}

struct RuntimeMetrics {
    publish_events: Counter<u64>,
    publish_latency_seconds: Histogram<f64>,
}

fn runtime_metrics() -> &'static RuntimeMetrics {
    static METRICS: OnceLock<RuntimeMetrics> = OnceLock::new();
    METRICS.get_or_init(|| {
        let meter = global::meter("hs-core");
        RuntimeMetrics {
            publish_events: meter
                .u64_counter("hs_core_publish_events_total")
                .with_description("Count of publish calls from hs-core grouped by message type and outcome")
                .build(),
            publish_latency_seconds: meter
                .f64_histogram("hs_core_publish_latency_seconds")
                .with_description("Latency of publish calls from hs-core")
                .with_unit("s")
                .build(),
        }
    })
}
