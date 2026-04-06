use std::sync::Arc;

use hs_eventbus_api::IngestAdapter;
use hs_eventbus_mqtt_ha::{HomeAssistantMqttConfig, HomeAssistantMqttIngestAdapter};
use hs_logger_core::{CoreMetadata, LoggerConfig};
use tracing::info;

use crate::{time::now_unix_ms, writer::LoggingPointWriter};

pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let config = HomeAssistantMqttConfig::from_env(now_unix_ms());
    let ingest_adapter = HomeAssistantMqttIngestAdapter::connect(config).await?;

    let writer = Arc::new(LoggingPointWriter);
    let processor = Arc::new(CoreMetadata::new(writer, LoggerConfig::default()));

    ingest_adapter.initialize(processor).await?;

    info!("timeseries logger service started");
    tokio::signal::ctrl_c().await?;
    info!("timeseries logger service stopped");

    Ok(())
}
