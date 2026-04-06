use std::env;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use hs_logger_core::{DataPoint, DataPointField, DataPointList, PointWriter};
use reqwest::Client;

#[derive(Clone, Debug)]
pub struct InfluxHttpConfig {
    pub base_url: String,
    pub org: String,
    pub bucket: String,
    pub token: String,
}

impl InfluxHttpConfig {
    pub fn from_env() -> Option<Self> {
        let base_url = env::var("INFLUX_URL").ok()?.trim().to_string();
        if base_url.is_empty() {
            return None;
        }

        let org = env::var("INFLUX_ORG").ok()?.trim().to_string();
        let bucket = env::var("INFLUX_BUCKET").ok()?.trim().to_string();
        let token = env::var("INFLUX_TOKEN").ok()?.trim().to_string();

        if org.is_empty() || bucket.is_empty() || token.is_empty() {
            return None;
        }

        Some(Self {
            base_url,
            org,
            bucket,
            token,
        })
    }
}

pub struct InfluxHttpPointWriter {
    client: Client,
    write_url: String,
    auth_header: String,
}

impl InfluxHttpPointWriter {
    pub fn new(config: InfluxHttpConfig) -> Result<Self> {
        let mut url = reqwest::Url::parse(config.base_url.trim_end_matches('/'))
            .with_context(|| format!("invalid INFLUX_URL: {}", config.base_url))?;
        url.set_path("/api/v2/write");
        url.query_pairs_mut()
            .append_pair("org", &config.org)
            .append_pair("bucket", &config.bucket)
            .append_pair("precision", "ms");

        Ok(Self {
            client: Client::new(),
            write_url: url.to_string(),
            auth_header: format!("Token {}", config.token),
        })
    }
}

#[async_trait]
impl PointWriter for InfluxHttpPointWriter {
    async fn write_points(&self, point_list: DataPointList) -> Result<()> {
        let body = point_list
            .iter()
            .filter_map(to_line_protocol)
            .collect::<Vec<_>>()
            .join("\n");

        if body.is_empty() {
            return Ok(());
        }

        let response = self
            .client
            .post(&self.write_url)
            .header("Authorization", &self.auth_header)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(body)
            .send()
            .await
            .context("failed to send Influx write request")?;

        if !response.status().is_success() {
            let status = response.status();
            let response_body = response.text().await.unwrap_or_default();
            bail!(
                "influx write failed with status {}: {}",
                status,
                response_body
            );
        }

        Ok(())
    }
}

fn to_line_protocol(point: &DataPoint) -> Option<String> {
    if point.fields.is_empty() {
        return None;
    }

    let measurement = escape_measurement(&point.measurement);
    let mut tags = String::new();
    for (key, value) in &point.tags {
        tags.push(',');
        tags.push_str(&escape_tag(key));
        tags.push('=');
        tags.push_str(&escape_tag(value));
    }

    let mut field_items = Vec::with_capacity(point.fields.len());
    for (key, value) in &point.fields {
        let rendered = match value {
            DataPointField::Number(v) if v.is_finite() => v.to_string(),
            DataPointField::Number(_) => continue,
            DataPointField::Bool(v) => v.to_string(),
            DataPointField::Text(v) => format!("\"{}\"", escape_field_string(v)),
        };
        field_items.push(format!("{}={}", escape_field_key(key), rendered));
    }

    if field_items.is_empty() {
        return None;
    }

    Some(format!(
        "{}{} {} {}",
        measurement,
        tags,
        field_items.join(","),
        point.observed_ms
    ))
}

fn escape_measurement(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(' ', "\\ ")
}

fn escape_tag(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(' ', "\\ ")
        .replace('=', "\\=")
}

fn escape_field_key(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(' ', "\\ ")
        .replace('=', "\\=")
}

fn escape_field_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
