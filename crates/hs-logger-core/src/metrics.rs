use opentelemetry::{
    global,
    metrics::{Counter, Histogram, Meter},
};
use std::sync::OnceLock;

pub(crate) struct LoggerMetrics {
    pub(crate) ingest_events_total: Counter<u64>,
    pub(crate) dropped_events_total: Counter<u64>,
    pub(crate) write_batches_total: Counter<u64>,
    pub(crate) write_points_total: Counter<u64>,
    pub(crate) write_latency_seconds: Histogram<f64>,
}

pub(crate) fn logger_metrics() -> &'static LoggerMetrics {
    static METRICS: OnceLock<LoggerMetrics> = OnceLock::new();
    METRICS.get_or_init(|| {
        let meter = global::meter("hs-logger-core");
        build_logger_metrics(&meter)
    })
}

fn build_logger_metrics(meter: &Meter) -> LoggerMetrics {
    LoggerMetrics {
        ingest_events_total: meter
            .u64_counter("hs_logger_ingest_events_total")
            .with_description("Count of ingest events received by logger core")
            .build(),
        dropped_events_total: meter
            .u64_counter("hs_logger_dropped_events_total")
            .with_description("Count of logger events dropped by policy or missing metadata")
            .build(),
        write_batches_total: meter
            .u64_counter("hs_logger_write_batches_total")
            .with_description("Count of point-writer batch attempts grouped by outcome")
            .build(),
        write_points_total: meter
            .u64_counter("hs_logger_write_points_total")
            .with_description("Count of points passed to writer grouped by outcome")
            .build(),
        write_latency_seconds: meter
            .f64_histogram("hs_logger_write_latency_seconds")
            .with_description("Write latency of point-writer batch calls")
            .with_unit("s")
            .build(),
    }
}
