use anyhow::{Context, Result};
use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{
    LogExporter, MetricExporter, SpanExporter, WithExportConfig, WithHttpConfig,
};
use opentelemetry_sdk::{
    logs::SdkLoggerProvider,
    metrics::{periodic_reader_with_async_runtime::PeriodicReader, SdkMeterProvider},
    runtime::Tokio,
    trace::SdkTracerProvider,
    Resource,
};
use std::{env, time::Duration};
use tracing::dispatcher;
use tracing_subscriber::{
    filter::filter_fn, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

const DEFAULT_LOG_FILTER: &str = "info";
const DEFAULT_OTLP_ENDPOINT: &str = "http://127.0.0.1:4318";
const DEFAULT_OTEL_METRIC_EXPORT_INTERVAL_SECS: u64 = 15;
const DEFAULT_OTEL_LOGS_ENABLED: bool = true;

pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

impl TelemetryGuard {
    pub fn no_otel() -> Self {
        Self {
            tracer_provider: None,
            meter_provider: None,
            logger_provider: None,
        }
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(logger_provider) = self.logger_provider.take() {
            let _ = logger_provider.shutdown();
        }

        if let Some(meter_provider) = self.meter_provider.take() {
            let _ = meter_provider.shutdown();
        }

        if let Some(tracer_provider) = self.tracer_provider.take() {
            let _ = tracer_provider.shutdown();
        }
    }
}

pub fn init(service_name: &str) -> Result<TelemetryGuard> {
    // Allow callers to initialize tracing early (for pre-runtime startup logs)
    // without failing when core runtime wiring invokes init again.
    if dispatcher::has_been_set() {
        return Ok(TelemetryGuard::no_otel());
    }

    let cfg = TelemetryConfig::from_env(service_name);
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_file(true);

    if !cfg.enabled {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .context("failed to initialize tracing subscriber")?;
        return Ok(TelemetryGuard::no_otel());
    }

    let resource = Resource::builder_empty()
        .with_attributes(vec![
            KeyValue::new("service.name", cfg.service_name.clone()),
            KeyValue::new("service.namespace", cfg.service_namespace.clone()),
            KeyValue::new("service.version", cfg.service_version.clone()),
            KeyValue::new("deployment.environment", cfg.environment.clone()),
        ])
        .build();

    let span_exporter = SpanExporter::builder()
        .with_http()
        .with_http_client(reqwest::Client::new())
        .with_endpoint(signal_endpoint(&cfg.otlp_endpoint, "traces"))
        .build()
        .context("failed to build OTLP span exporter")?;

    let tracer_provider = SdkTracerProvider::builder()
        .with_simple_exporter(span_exporter)
        .with_resource(resource.clone())
        .build();

    let tracer = tracer_provider.tracer(cfg.service_name.clone());

    let logger_provider = if cfg.logs_enabled {
        let log_exporter = LogExporter::builder()
            .with_http()
            .with_http_client(reqwest::Client::new())
            .with_endpoint(signal_endpoint(&cfg.otlp_endpoint, "logs"))
            .build()
            .context("failed to build OTLP log exporter")?;

        Some(
            SdkLoggerProvider::builder()
                .with_simple_exporter(log_exporter)
                .with_resource(resource.clone())
                .build(),
        )
    } else {
        None
    };

    let metric_exporter = MetricExporter::builder()
        .with_http()
        .with_http_client(reqwest::Client::new())
        .with_endpoint(signal_endpoint(&cfg.otlp_endpoint, "metrics"))
        .build()
        .context("failed to build OTLP metric exporter")?;

    let metric_reader = PeriodicReader::builder(metric_exporter, Tokio)
        .with_interval(Duration::from_secs(cfg.metric_export_interval_secs))
        .build();

    let meter_provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(metric_reader)
        .build();

    global::set_meter_provider(meter_provider.clone());
    global::set_tracer_provider(tracer_provider.clone());

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(tracing_opentelemetry::layer().with_tracer(tracer));

    if let Some(ref provider) = logger_provider {
        // Prevent recursion from OpenTelemetry SDK internal diagnostics being bridged back into OTLP logs.
        let otel_log_layer =
            OpenTelemetryTracingBridge::new(provider).with_filter(filter_fn(|metadata| {
                !metadata.target().starts_with("opentelemetry")
            }));

        registry
            .with(otel_log_layer)
            .try_init()
            .context("failed to initialize tracing subscriber with OpenTelemetry layers")?;
    } else {
        registry
            .try_init()
            .context("failed to initialize tracing subscriber with OpenTelemetry layer")?;
    }

    Ok(TelemetryGuard {
        tracer_provider: Some(tracer_provider),
        meter_provider: Some(meter_provider),
        logger_provider,
    })
}

struct TelemetryConfig {
    enabled: bool,
    logs_enabled: bool,
    otlp_endpoint: String,
    service_name: String,
    service_namespace: String,
    service_version: String,
    environment: String,
    metric_export_interval_secs: u64,
}

impl TelemetryConfig {
    fn from_env(default_service_name: &str) -> Self {
        let enabled = env_bool("HS_OTEL_ENABLED", false);
        let logs_enabled = env_bool("HS_OTEL_LOGS_ENABLED", DEFAULT_OTEL_LOGS_ENABLED);
        let otlp_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_OTLP_ENDPOINT.to_string());

        let service_name = env::var("OTEL_SERVICE_NAME")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_service_name.to_string());

        let service_namespace = env::var("OTEL_SERVICE_NAMESPACE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "home-services".to_string());

        let service_version = env::var("OTEL_SERVICE_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

        let environment = env::var("HS_ENV")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "dev".to_string());

        let metric_export_interval_secs = env::var("HS_OTEL_METRIC_EXPORT_INTERVAL_SECS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .unwrap_or(DEFAULT_OTEL_METRIC_EXPORT_INTERVAL_SECS)
            .max(5);

        Self {
            enabled,
            logs_enabled,
            otlp_endpoint,
            service_name,
            service_namespace,
            service_version,
            environment,
            metric_export_interval_secs,
        }
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn signal_endpoint(base: &str, signal: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.ends_with(&format!("/v1/{signal}")) {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1/{signal}")
    }
}
