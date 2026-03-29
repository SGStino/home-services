use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use crate::{config::Hs110Config, tplink_protocol};

const SYSINFO_QUERY: &str = r#"{"system":{"get_sysinfo":{}}}"#;
const EMETER_QUERY: &str = r#"{"emeter":{"get_realtime":{}}}"#;

#[derive(Clone)]
pub struct Hs110Client {
    host: String,
    port: u16,
    request_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct Hs110Snapshot {
    pub relay_on: bool,
    pub power_w: Option<f64>,
    pub voltage_v: Option<f64>,
    pub current_a: Option<f64>,
    pub energy_total_kwh: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Hs110SysInfo {
    pub alias: String,
    pub model: String,
    pub device_id: String,
    pub mac: String,
    pub relay_on: bool,
}

impl Hs110Client {
    pub fn new(config: &Hs110Config) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            request_timeout: Duration::from_millis(config.request_timeout_ms),
        }
    }

    pub async fn snapshot(&self) -> Result<Hs110Snapshot> {
        let sysinfo = self.sysinfo().await?;

        let emeter = self.send_json(EMETER_QUERY).await?;

        let power_w = find_number(&emeter, &["emeter", "get_realtime", "power"])
            .or_else(|| find_number(&emeter, &["emeter", "get_realtime", "power_mw"]).map(|v| v / 1_000.0));

        let voltage_v = find_number(&emeter, &["emeter", "get_realtime", "voltage"])
            .or_else(|| find_number(&emeter, &["emeter", "get_realtime", "voltage_mv"]).map(|v| v / 1_000.0));

        let current_a = find_number(&emeter, &["emeter", "get_realtime", "current"])
            .or_else(|| find_number(&emeter, &["emeter", "get_realtime", "current_ma"]).map(|v| v / 1_000.0));

        let energy_total_kwh = find_number(&emeter, &["emeter", "get_realtime", "total"])
            .or_else(|| find_number(&emeter, &["emeter", "get_realtime", "total_wh"]).map(|v| v / 1_000.0));

        Ok(Hs110Snapshot {
            relay_on: sysinfo.relay_on,
            power_w,
            voltage_v,
            current_a,
            energy_total_kwh,
        })
    }

    pub async fn sysinfo(&self) -> Result<Hs110SysInfo> {
        let payload = self.send_json(SYSINFO_QUERY).await?;

        let relay_state = find_i64(&payload, &["system", "get_sysinfo", "relay_state"])
            .context("missing relay_state in get_sysinfo")?;

        Ok(Hs110SysInfo {
            alias: find_string(&payload, &["system", "get_sysinfo", "alias"]).unwrap_or_default(),
            model: find_string(&payload, &["system", "get_sysinfo", "model"]).unwrap_or_else(|| "HS110".to_string()),
            device_id: find_string(&payload, &["system", "get_sysinfo", "deviceId"]).unwrap_or_default(),
            mac: find_string(&payload, &["system", "get_sysinfo", "mac"]).unwrap_or_default(),
            relay_on: relay_state == 1,
        })
    }

    pub async fn set_power(&self, on: bool) -> Result<()> {
        let state = if on { 1 } else { 0 };
        let request = format!(
            "{{\"system\":{{\"set_relay_state\":{{\"state\":{state}}}}}}}"
        );
        let response = self.send_json(&request).await?;

        let err_code = find_number(&response, &["system", "set_relay_state", "err_code"])
            .ok_or_else(|| anyhow!("missing err_code in set_relay_state response"))? as i64;

        if err_code != 0 {
            return Err(anyhow!("set_relay_state failed with err_code={err_code}"));
        }

        Ok(())
    }

    async fn send_json(&self, payload: &str) -> Result<Value> {
        let address = format!("{}:{}", self.host, self.port);
        let frame = tplink_protocol::frame_request(payload);

        let mut stream = timeout(self.request_timeout, TcpStream::connect(&address))
            .await
            .with_context(|| format!("timed out connecting to HS110 at {address}"))??;

        timeout(self.request_timeout, stream.write_all(&frame))
            .await
            .context("timed out sending HS110 request")??;

        let mut header = [0u8; 4];
        timeout(self.request_timeout, stream.read_exact(&mut header))
            .await
            .context("timed out reading HS110 response header")??;

        let body_len = u32::from_be_bytes(header) as usize;
        let mut body = vec![0u8; body_len];
        timeout(self.request_timeout, stream.read_exact(&mut body))
            .await
            .context("timed out reading HS110 response body")??;

        let mut full_frame = Vec::with_capacity(4 + body_len);
        full_frame.extend_from_slice(&header);
        full_frame.extend_from_slice(&body);

        let plaintext = tplink_protocol::parse_response_payload(&full_frame)
            .ok_or_else(|| anyhow!("invalid HS110 framed response"))?;

        serde_json::from_slice::<Value>(&plaintext)
            .with_context(|| format!("invalid HS110 JSON response: {}", String::from_utf8_lossy(&plaintext)))
    }
}

fn find_number(value: &Value, path: &[&str]) -> Option<f64> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    match current {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn find_i64(value: &Value, path: &[&str]) -> Option<i64> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    match current {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse::<i64>().ok(),
        _ => None,
    }
}

fn find_string(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    match current {
        Value::String(text) => Some(text.trim().to_string()),
        _ => None,
    }
}
