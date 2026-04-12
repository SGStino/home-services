use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

use crate::config::TasmotaConfig;

#[derive(Clone, Debug)]
pub struct TasmotaStatus {
    pub relay_on: bool,
    pub power_w: Option<f64>,
    pub voltage_v: Option<f64>,
    pub current_a: Option<f64>,
    pub energy_total_kwh: Option<f64>,
    pub device_name: Option<String>,
    pub module: Option<String>,
    pub mac: Option<String>,
    pub firmware: Option<String>,
}

pub struct TasmotaClient {
    http: Client,
    base_url: String,
    username: Option<String>,
    password: Option<String>,
}

impl TasmotaClient {
    pub fn new(config: &TasmotaConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .build()
            .context("failed to build Tasmota HTTP client")?;

        Ok(Self {
            http,
            base_url: config.base_url(),
            username: config.username.clone(),
            password: config.password.clone(),
        })
    }

    pub async fn status(&self) -> Result<TasmotaStatus> {
        let response = self.command_json("Status 0").await?;
        Ok(parse_status(&response))
    }

    pub async fn set_power(&self, on: bool) -> Result<()> {
        let command = if on { "Power On" } else { "Power Off" };
        let _ = self.command_json(command).await?;
        Ok(())
    }

    async fn command_json(&self, command: &str) -> Result<Value> {
        let url = format!("{}/cm", self.base_url);
        let mut request = self.http.get(&url).query(&[("cmnd", command)]);

        if let Some(username) = &self.username {
            request = request.basic_auth(username, self.password.as_ref());
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("request to Tasmota command endpoint failed: {command}"))?
            .error_for_status()
            .with_context(|| format!("Tasmota returned non-success for command: {command}"))?;

        response
            .json::<Value>()
            .await
            .with_context(|| format!("failed to parse Tasmota JSON response for command: {command}"))
    }
}

fn parse_status(payload: &Value) -> TasmotaStatus {
    let relay = power_state(payload).unwrap_or(false);
    let energy = payload
        .pointer("/StatusSNS/ENERGY")
        .or_else(|| payload.pointer("/StatusSTS/ENERGY"));

    TasmotaStatus {
        relay_on: relay,
        power_w: energy.and_then(|value| read_number(value, "Power")),
        voltage_v: energy.and_then(|value| read_number(value, "Voltage")),
        current_a: energy.and_then(|value| read_number(value, "Current")),
        energy_total_kwh: energy.and_then(|value| read_number(value, "Total")),
        device_name: payload
            .pointer("/Status/DeviceName")
            .and_then(Value::as_str)
            .map(str::to_string),
        module: payload
            .pointer("/Status/Module")
            .and_then(Value::as_str)
            .map(str::to_string),
        mac: payload
            .pointer("/StatusNET/Mac")
            .and_then(Value::as_str)
            .map(str::to_string),
        firmware: payload
            .pointer("/StatusFWR/Version")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn power_state(payload: &Value) -> Option<bool> {
    let raw = payload
        .pointer("/StatusSTS/POWER")
        .or_else(|| payload.get("POWER"))
        .or_else(|| payload.pointer("/Status/Power"));

    match raw {
        Some(Value::String(s)) => {
            let v = s.trim().to_ascii_uppercase();
            if v == "ON" || v == "1" {
                Some(true)
            } else if v == "OFF" || v == "0" {
                Some(false)
            } else {
                None
            }
        }
        Some(Value::Number(n)) => n.as_i64().map(|v| v == 1),
        Some(Value::Bool(v)) => Some(*v),
        _ => None,
    }
}

fn read_number(object: &Value, key: &str) -> Option<f64> {
    let value = object.get(key)?;
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}
