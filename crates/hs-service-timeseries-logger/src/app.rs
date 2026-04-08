use std::sync::Arc;

use hs_eventbus_api::IngestAdapter;
use hs_eventbus_mqtt_ha::{HomeAssistantMqttConfig, HomeAssistantMqttIngestAdapter};
use hs_eventbus_mqtt_sparkplug_b::{SparkplugBConfig, SparkplugBMqttIngestAdapter};
use hs_logger_core::{CoreMetadata, LoggerConfig, PointWriter};
use tracing::{info, warn};

use crate::{
    influx_writer::{InfluxHttpConfig, InfluxHttpPointWriter},
    time::now_unix_ms,
    writer::LoggingPointWriter,
};

pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let mode = AdapterMode::from_env();
    let now = now_unix_ms();

    let ingest_adapter: Arc<dyn IngestAdapter> = match mode {
        AdapterMode::HomeAssistant => {
            let config = HomeAssistantMqttConfig::from_env(now);
            Arc::new(HomeAssistantMqttIngestAdapter::connect(config).await?)
        }
        AdapterMode::SparkplugB => {
            let config = SparkplugBConfig::from_env(now);
            Arc::new(SparkplugBMqttIngestAdapter::connect(config).await?)
        }
    };

    let writer = build_point_writer()?;
    let processor = Arc::new(CoreMetadata::new(writer, LoggerConfig::default()));

    ingest_adapter.initialize(processor).await?;

    info!("timeseries logger service started");
    tokio::signal::ctrl_c().await?;
    info!("timeseries logger service stopped");

    Ok(())
}

#[derive(Copy, Clone, Debug)]
enum AdapterMode {
    HomeAssistant,
    SparkplugB,
}

impl AdapterMode {
    fn from_env() -> Self {
        let value = std::env::var("EVENTBUS_ADAPTER")
            .unwrap_or_else(|_| "mqtt-ha".to_string())
            .to_ascii_lowercase();

        match value.as_str() {
            "sparkplug" | "sparkplug-b" | "mqtt-sparkplug-b" => Self::SparkplugB,
            _ => Self::HomeAssistant,
        }
    }
}

fn build_point_writer() -> anyhow::Result<Arc<dyn PointWriter>> {
    if let Some(config) = InfluxHttpConfig::from_env() {
        let writer = InfluxHttpPointWriter::new(config)?;
        info!("using Influx HTTP point writer");
        return Ok(Arc::new(writer));
    }

    warn!("INFLUX_URL/INFLUX_ORG/INFLUX_BUCKET/INFLUX_TOKEN not fully configured; using logging writer");
    Ok(Arc::new(LoggingPointWriter))
}
