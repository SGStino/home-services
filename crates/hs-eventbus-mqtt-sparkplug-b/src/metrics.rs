use opentelemetry::{global, metrics::Counter};
use std::sync::OnceLock;

pub(crate) struct SparkplugMetrics {
    /// Number of MQTT publish calls, labelled by `topic` and `outcome` ("ok" / "error").
    pub(crate) publishes_total: Counter<u64>,
}

pub(crate) fn sparkplug_metrics() -> &'static SparkplugMetrics {
    static METRICS: OnceLock<SparkplugMetrics> = OnceLock::new();
    METRICS.get_or_init(|| {
        let meter = global::meter("hs-eventbus-mqtt-sparkplug-b");
        SparkplugMetrics {
            publishes_total: meter
                .u64_counter("mqtt_sparkplug_publishes_total")
                .with_description(
                    "Count of MQTT publish calls from the Sparkplug B adapter, per topic and outcome",
                )
                .build(),
        }
    })
}
