mod command;
mod config;
mod ingest_adapter;
mod metrics;
mod payloads;
mod publish_adapter;
mod sparkplug;
mod topics;
mod transport;

pub use config::SparkplugBConfig;
pub use ingest_adapter::SparkplugBMqttIngestAdapter;
pub use publish_adapter::SparkplugBMqttPublishAdapter;

// Backward-compatible alias pattern used by other adapters.
pub type SparkplugBMqttAdapter = SparkplugBMqttPublishAdapter;
