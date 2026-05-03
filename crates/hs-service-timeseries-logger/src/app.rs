use std::sync::Arc;

use hs_eventbus_api::{EventProcessor, IngestAdapter};
use hs_eventbus_mqtt_ha::{HomeAssistantMqttConfig, HomeAssistantMqttIngestAdapter};
use hs_eventbus_mqtt_sparkplug_b::{SparkplugBConfig, SparkplugBMqttIngestAdapter};
use hs_logger_core::{CoreMetadata, LoggerConfig, PointWriter};
use tracing::{info, warn};

use crate::{
    influx_writer::{InfluxHttpConfig, InfluxHttpPointWriter},
    status::{spawn_status_server, LoggerStatus, RuntimeStatusConfig, StatusHttpConfig},
    time::now_unix_ms,
    tracking_processor::TrackingEventProcessor,
    writer::LoggingPointWriter,
};

pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let mode = AdapterMode::from_env();
    let now = now_unix_ms();
    let status_http = StatusHttpConfig::from_env();

    let ingest = build_ingest_adapter(mode, now).await?;

    let (writer, influx_target) = build_point_writer()?;
    let status = LoggerStatus::new(RuntimeStatusConfig {
        adapter_mode: ingest.adapter_mode.clone(),
        mqtt_host: ingest.mqtt_host.clone(),
        mqtt_identity: ingest.mqtt_identity.clone(),
        influx_target,
        subscriptions: ingest.subscriptions.clone(),
    });
    spawn_status_server(status.clone(), status_http.clone()).await?;

    let core_processor: Arc<dyn EventProcessor> = Arc::new(CoreMetadata::new(writer, LoggerConfig::default()));
    let processor: Arc<dyn EventProcessor> = Arc::new(TrackingEventProcessor::new(core_processor, status));

    ingest.ingest_adapter.initialize(processor).await?;

    info!(
        status_bind = %status_http.bind_addr(),
        adapter = %ingest.adapter_mode,
        "timeseries logger service started"
    );
    tokio::signal::ctrl_c().await?;
    info!("timeseries logger service stopped");

    Ok(())
}

struct IngestRuntime {
    ingest_adapter: Arc<dyn IngestAdapter>,
    adapter_mode: String,
    mqtt_host: String,
    mqtt_identity: String,
    subscriptions: Vec<String>,
}

async fn build_ingest_adapter(mode: AdapterMode, now: u64) -> anyhow::Result<IngestRuntime> {
    match mode {
        AdapterMode::HomeAssistant => {
            let config = HomeAssistantMqttConfig::from_env(now);
            let subscriptions = vec![
                format!("{}/+/+/+/config", config.discovery_prefix),
                "hs/state/+/+/+".to_string(),
            ];

            Ok(IngestRuntime {
                ingest_adapter: Arc::new(HomeAssistantMqttIngestAdapter::connect(config.clone()).await?),
                adapter_mode: "mqtt-ha".to_string(),
                mqtt_host: config.broker_host,
                mqtt_identity: format!(
                    "node={} client={} session={}",
                    config.node_id, config.client_id, config.availability_session
                ),
                subscriptions,
            })
        }
        AdapterMode::SparkplugB => {
            let config = SparkplugBConfig::from_env(now);
            let group = sanitize_topic_component(&config.group_id);
            let subscriptions = vec![
                format!("spBv1.0/{}/DBIRTH/+/+", group),
                format!("spBv1.0/{}/DDATA/+/+", group),
                format!("spBv1.0/{}/DDEATH/+/+", group),
                format!("spBv1.0/{}/NBIRTH/+", group),
                "spBv1.0/STATE/+".to_string(),
            ];

            Ok(IngestRuntime {
                ingest_adapter: Arc::new(SparkplugBMqttIngestAdapter::connect(config.clone()).await?),
                adapter_mode: "mqtt-sparkplug-b".to_string(),
                mqtt_host: config.broker_host,
                mqtt_identity: format!(
                    "group={} edge_node={} client={}",
                    config.group_id, config.edge_node_id, config.client_id
                ),
                subscriptions,
            })
        }
    }
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

fn build_point_writer() -> anyhow::Result<(Arc<dyn PointWriter>, String)> {
    if let Some(config) = InfluxHttpConfig::from_env() {
        let target = format!(
            "enabled (url={} org={} bucket={})",
            config.base_url, config.org, config.bucket
        );
        let writer = InfluxHttpPointWriter::new(config)?;
        info!("using Influx HTTP point writer");
        return Ok((Arc::new(writer), target));
    }

    warn!("INFLUX_URL/INFLUX_ORG/INFLUX_BUCKET/INFLUX_TOKEN not fully configured; using logging writer");
    Ok((
        Arc::new(LoggingPointWriter),
        "disabled (logging writer)".to_string(),
    ))
}

fn sanitize_topic_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}
