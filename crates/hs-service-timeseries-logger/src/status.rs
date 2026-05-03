use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use hs_device_contracts::DiscoveryMessage;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Clone, Debug)]
pub struct StatusHttpConfig {
    pub host: String,
    pub port: u16,
}

impl StatusHttpConfig {
    pub fn from_env() -> Self {
        let host = env::var("STATUS_HTTP_HOST")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "0.0.0.0".to_string());
        let port = env::var("STATUS_HTTP_PORT")
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(8088);

        Self { host, port }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeStatusConfig {
    pub adapter_mode: String,
    pub mqtt_host: String,
    pub mqtt_identity: String,
    pub influx_target: String,
    pub subscriptions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct LoggerStatus {
    inner: Arc<RwLock<StatusInner>>,
}

impl LoggerStatus {
    pub fn new(config: RuntimeStatusConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(StatusInner {
                config,
                tracked_devices: BTreeMap::new(),
                discovery_to_device: HashMap::new(),
            })),
        }
    }

    pub async fn on_discovery(&self, key: &str, event: &DiscoveryMessage) {
        let mut inner = self.inner.write().await;
        let adapter_mode = inner.config.adapter_mode.clone();

        let canonical_id = canonical_device_id(&event.device.service_id, &event.device.device_id);
        if let Some(previous_device_id) = inner
            .discovery_to_device
            .insert(key.to_string(), canonical_id.clone())
        {
            if previous_device_id != canonical_id {
                prune_discovery_key(&mut inner.tracked_devices, &previous_device_id, key);
            }
        }

        let state_source = state_source_for_device(
            &inner.config.adapter_mode,
            &event.device.service_id,
            &event.device.device_id,
        );

        let entry = inner
            .tracked_devices
            .entry(canonical_id)
            .or_insert_with(|| TrackedDevice {
                adapter: adapter_mode,
                node_id: event.device.service_id.clone(),
                device_id: event.device.device_id.clone(),
                capabilities: BTreeSet::new(),
                discovery_keys: BTreeSet::new(),
                state_source,
                availability_topic: event.availability_topic.clone(),
            });

        if entry.availability_topic.is_none() || event.availability_topic.is_some() {
            entry.availability_topic = event.availability_topic.clone();
        }
        entry
            .capabilities
            .extend(event.capabilities.iter().map(|cap| cap.capability_id.clone()));
        entry.discovery_keys.insert(key.to_string());
    }

    pub async fn on_tombstone(&self, key: &str) {
        let mut inner = self.inner.write().await;
        if let Some(canonical_id) = inner.discovery_to_device.remove(key) {
            prune_discovery_key(&mut inner.tracked_devices, &canonical_id, key);
        }
    }

    pub async fn snapshot(&self) -> StatusSnapshot {
        let inner = self.inner.read().await;
        let mut subscriptions = inner.config.subscriptions.clone();
        let mut seen_avail: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for device in inner.tracked_devices.values() {
            if let Some(ref topic) = device.availability_topic {
                if seen_avail.insert(topic.clone()) {
                    subscriptions.push(topic.clone());
                }
            }
        }
        StatusSnapshot {
            config: RuntimeStatusConfig {
                subscriptions,
                ..inner.config.clone()
            },
            tracked_devices: inner
                .tracked_devices
                .values()
                .map(|device| TrackedDeviceView {
                    adapter: device.adapter.clone(),
                    node_id: device.node_id.clone(),
                    device_id: device.device_id.clone(),
                    capability_count: device.capabilities.len(),
                    discovery_keys: device.discovery_keys.iter().cloned().collect(),
                    state_source: device.state_source.clone(),
                    availability_source: device.availability_topic.clone().unwrap_or_default(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct StatusInner {
    config: RuntimeStatusConfig,
    tracked_devices: BTreeMap<String, TrackedDevice>,
    discovery_to_device: HashMap<String, String>,
}

#[derive(Clone, Debug)]
struct TrackedDevice {
    adapter: String,
    node_id: String,
    device_id: String,
    capabilities: BTreeSet<String>,
    discovery_keys: BTreeSet<String>,
    state_source: String,
    availability_topic: Option<String>,
}

#[derive(Clone, Debug)]
pub struct StatusSnapshot {
    pub config: RuntimeStatusConfig,
    pub tracked_devices: Vec<TrackedDeviceView>,
}

#[derive(Clone, Debug)]
pub struct TrackedDeviceView {
    pub adapter: String,
    pub node_id: String,
    pub device_id: String,
    pub capability_count: usize,
    pub discovery_keys: Vec<String>,
    pub state_source: String,
    pub availability_source: String,
}

pub async fn spawn_status_server(status: LoggerStatus, http: StatusHttpConfig) -> Result<()> {
    let app = Router::new()
        .route("/", get(status_html))
        .route("/status", get(status_html))
        .with_state(status);

    let bind_addr = http.bind_addr();
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind status server on {}", bind_addr))?;

    info!(bind = %bind_addr, "status page listening");
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            info!(error = %err, "status server stopped");
        }
    });

    Ok(())
}

async fn status_html(State(status): State<LoggerStatus>) -> Html<String> {
    let snapshot = status.snapshot().await;
    Html(render_html(snapshot))
}

fn render_html(snapshot: StatusSnapshot) -> String {
    let subscriptions = if snapshot.config.subscriptions.is_empty() {
        "-".to_string()
    } else {
        snapshot
            .config
            .subscriptions
            .iter()
            .map(|topic| html_escape(topic))
            .collect::<Vec<_>>()
            .join("<br>")
    };

    let mut rows = String::new();
    for device in &snapshot.tracked_devices {
        let discovery_topics = if device.discovery_keys.is_empty() {
            "-".to_string()
        } else {
            device
                .discovery_keys
                .iter()
                .map(|k| format!("<code>{}</code>", html_escape(k)))
                .collect::<Vec<_>>()
                .join("<br>")
        };

        rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&device.adapter),
            html_escape(&device.node_id),
            html_escape(&device.device_id),
            device.capability_count,
            discovery_topics,
            html_escape(&device.state_source),
            html_escape(&device.availability_source),
        ));
    }

    if rows.is_empty() {
        rows.push_str("<tr><td colspan=\"7\">No tracked devices yet.</td></tr>");
    }

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"refresh\" content=\"10\"><title>Timeseries Logger Status</title><style>body{{font-family:ui-sans-serif,system-ui,-apple-system,Segoe UI,sans-serif;background:#f6f7f8;color:#1f2937;margin:0;padding:24px}}h1{{margin:0 0 12px;font-size:28px}}.card{{background:#fff;border:1px solid #d1d5db;border-radius:12px;padding:16px;margin-bottom:16px}}table{{width:100%;border-collapse:collapse}}th,td{{text-align:left;border-bottom:1px solid #e5e7eb;padding:8px;vertical-align:top}}th{{font-size:12px;text-transform:uppercase;color:#6b7280}}code{{background:#f3f4f6;padding:2px 4px;border-radius:4px}}</style></head><body><h1>Timeseries Logger Status</h1><div class=\"card\"><h2>Runtime</h2><p><strong>Adapter</strong>: {}</p><p><strong>MQTT Host</strong>: {}</p><p><strong>MQTT Identity</strong>: {}</p><p><strong>Influx Target</strong>: {}</p><p><strong>Subscriptions</strong>:<br>{}</p></div><div class=\"card\"><h2>Tracked Devices ({})</h2><table><thead><tr><th>Adapter</th><th>Node</th><th>Device</th><th>Capabilities</th><th>Discovery Topic(s)</th><th>State Source</th><th>Availability Topic</th></tr></thead><tbody>{}</tbody></table></div></body></html>",
        html_escape(&snapshot.config.adapter_mode),
        html_escape(&snapshot.config.mqtt_host),
        html_escape(&snapshot.config.mqtt_identity),
        html_escape(&snapshot.config.influx_target),
        subscriptions,
        snapshot.tracked_devices.len(),
        rows,
    )
}

fn canonical_device_id(node_id: &str, device_id: &str) -> String {
    format!("{}/{}", node_id, device_id)
}

fn state_source_for_device(adapter_mode: &str, node_id: &str, device_id: &str) -> String {
    if adapter_mode == "mqtt-sparkplug-b" {
        format!("spBv1.0/<group>/DDATA/{}/{}", node_id, device_id)
    } else {
        format!("hs/state/{}/{}/+", node_id, device_id)
    }
}

fn prune_discovery_key(
    tracked_devices: &mut BTreeMap<String, TrackedDevice>,
    canonical_id: &str,
    discovery_key: &str,
) {
    if let Some(device) = tracked_devices.get_mut(canonical_id) {
        device.discovery_keys.remove(discovery_key);
        if device.discovery_keys.is_empty() {
            tracked_devices.remove(canonical_id);
        }
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
    .replace('\'', "&#39;")
}
