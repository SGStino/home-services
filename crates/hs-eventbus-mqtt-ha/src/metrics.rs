use opentelemetry::{global, metrics::Counter};
use std::sync::OnceLock;

pub(crate) struct MqttMetrics {
    /// Number of MQTT publish calls, labelled by `topic` and `outcome` ("ok" / "error").
    pub(crate) publishes_total: Counter<u64>,
}

pub(crate) fn mqtt_metrics() -> &'static MqttMetrics {
    static METRICS: OnceLock<MqttMetrics> = OnceLock::new();
    METRICS.get_or_init(|| {
        let meter = global::meter("hs-eventbus-mqtt-ha");
        MqttMetrics {
            publishes_total: meter
                .u64_counter("mqtt_ha_publishes_total")
                .with_description(
                    "Count of MQTT publish calls from the Home Assistant adapter, per topic and outcome",
                )
                .build(),
        }
    })
}
