use opentelemetry::{
    global,
    metrics::{Counter, Histogram},
};
use std::sync::OnceLock;

pub(crate) struct RuntimeMetrics {
    pub(crate) publish_events: Counter<u64>,
    pub(crate) publish_latency_seconds: Histogram<f64>,
}

pub(crate) fn runtime_metrics() -> &'static RuntimeMetrics {
    static METRICS: OnceLock<RuntimeMetrics> = OnceLock::new();
    METRICS.get_or_init(|| {
        let meter = global::meter("hs-core");
        RuntimeMetrics {
            publish_events: meter
                .u64_counter("hs_core_publish_events_total")
                .with_description(
                    "Count of publish calls from hs-core grouped by message type and outcome",
                )
                .build(),
            publish_latency_seconds: meter
                .f64_histogram("hs_core_publish_latency_seconds")
                .with_description("Latency of publish calls from hs-core")
                .with_unit("s")
                .build(),
        }
    })
}
