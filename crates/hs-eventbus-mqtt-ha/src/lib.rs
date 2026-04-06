mod ingest_adapter;
mod command;
mod config;
mod metrics;
mod payloads;
mod publish_adapter;
mod topics;
mod transport;

pub use config::HomeAssistantMqttConfig;
pub use ingest_adapter::HomeAssistantMqttIngestAdapter;
pub use publish_adapter::HomeAssistantMqttPublishAdapter;

// Backward-compatible alias for existing device services.
pub type HomeAssistantMqttAdapter = HomeAssistantMqttPublishAdapter;
