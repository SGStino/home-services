use std::{fs::File, io::BufReader, sync::Arc};

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use rustls::{pki_types::CertificateDer, ClientConfig, RootCertStore};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    connect_async,
    Connector,
    tungstenite::{
        client::IntoClientRequest,
        http::{header::AUTHORIZATION, HeaderValue},
        Message,
    },
    MaybeTlsStream,
    WebSocketStream,
};
use tracing::{debug, info};
use uuid::Uuid;

pub type MatterStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
pub enum MatterMessage {
    ServerInfo(Value),
    Event {
        event: String,
        data: Value,
    },
    Success {
        message_id: String,
        result: Value,
    },
    Error {
        message_id: Option<String>,
        error_code: i64,
        details: Option<String>,
    },
    Unknown(Value),
}

#[derive(Deserialize)]
struct RawEventMessage {
    event: String,
    data: Value,
}

#[derive(Deserialize)]
struct RawSuccessMessage {
    message_id: String,
    result: Value,
}

#[derive(Deserialize)]
struct RawErrorMessage {
    #[serde(default)]
    message_id: Option<String>,
    error_code: i64,
    #[serde(default)]
    details: Option<String>,
}

pub async fn connect(url: &str, ca_cert_path: Option<&str>) -> Result<MatterStream> {
    let safe_url = redact_url_for_logs(url);
    let request = build_request(url)?;
    let (mut stream, response) = match ca_cert_path {
        Some(path) => {
            let connector = Connector::Rustls(Arc::new(build_tls_config_with_ca(path)?));
            connect_async_tls_with_config(request, None, false, Some(connector))
                .await
                .with_context(|| {
                    format!(
                        "failed to connect to Matter websocket at {safe_url} using custom CA {path}"
                    )
                })?
        }
        None => connect_async(request)
            .await
            .with_context(|| format!("failed to connect to Matter websocket at {safe_url}"))?,
    };

    info!(url = %safe_url, status = %response.status(), "connected to Matter websocket");

    // The server sends a server-info payload immediately after connect.
    if let Some(initial) = read_next_message(&mut stream).await? {
        debug!(initial = %initial, "received Matter server-info message");
    }

    Ok(stream)
}

fn redact_url_for_logs(url: &str) -> String {
    let Some(scheme_pos) = url.find("://") else {
        return url.to_string();
    };

    let authority_start = scheme_pos + 3;
    let rest = &url[authority_start..];
    let authority_end = rest.find('/').map(|i| authority_start + i).unwrap_or(url.len());
    let authority = &url[authority_start..authority_end];

    if let Some(at_pos) = authority.rfind('@') {
        let host = &authority[at_pos + 1..];
        let mut out = String::with_capacity(url.len());
        out.push_str(&url[..authority_start]);
        out.push_str("***:***@");
        out.push_str(host);
        out.push_str(&url[authority_end..]);
        return out;
    }

    url.to_string()
}

fn build_request(url: &str) -> Result<tokio_tungstenite::tungstenite::handshake::client::Request> {
    let (clean_url, auth_header) = split_userinfo(url);
    let clean_url_for_err = clean_url.clone();
    let mut request = clean_url
        .into_client_request()
        .with_context(|| format!("invalid websocket URL: {clean_url_for_err}"))?;

    if let Some(header_value) = auth_header {
        let hv = HeaderValue::from_str(&header_value)
            .context("failed to construct websocket Authorization header")?;
        request.headers_mut().insert(AUTHORIZATION, hv);
    }

    Ok(request)
}

fn split_userinfo(url: &str) -> (String, Option<String>) {
    let Some(scheme_pos) = url.find("://") else {
        return (url.to_string(), None);
    };

    let authority_start = scheme_pos + 3;
    let rest = &url[authority_start..];
    let authority_end = rest.find('/').map(|i| authority_start + i).unwrap_or(url.len());
    let authority = &url[authority_start..authority_end];

    let Some(at_pos) = authority.rfind('@') else {
        return (url.to_string(), None);
    };

    let userinfo = &authority[..at_pos];
    let host = &authority[at_pos + 1..];
    let mut clean_url = String::with_capacity(url.len());
    clean_url.push_str(&url[..authority_start]);
    clean_url.push_str(host);
    clean_url.push_str(&url[authority_end..]);

    let auth = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(userinfo)
    );

    (clean_url, Some(auth))
}

fn build_tls_config_with_ca(ca_cert_path: &str) -> Result<ClientConfig> {
    let file = File::open(ca_cert_path)
        .with_context(|| format!("failed to open MATTER_TLS_CA_CERT_PATH: {ca_cert_path}"))?;
    let mut reader = BufReader::new(file);

    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to parse PEM certificates from {ca_cert_path}"))?;

    if certs.is_empty() {
        anyhow::bail!("no certificates found in {ca_cert_path}");
    }

    let mut roots = RootCertStore::empty();
    for cert in certs {
        roots
            .add(cert)
            .context("failed to add certificate to rustls root store")?;
    }

    Ok(ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

pub async fn send_start_listening(stream: &mut MatterStream) -> Result<String> {
    let message_id = format!("start-{}", Uuid::new_v4());
    let payload = json!({
        "message_id": message_id,
        "command": "start_listening"
    });

    stream
        .send(Message::Text(payload.to_string()))
        .await
        .context("failed to send start_listening command")?;

    Ok(message_id)
}

pub async fn read_next_message(stream: &mut MatterStream) -> Result<Option<Value>> {
    loop {
        match stream.next().await {
            Some(Ok(Message::Text(text))) => {
                let json = serde_json::from_str::<Value>(&text)
                    .with_context(|| format!("invalid Matter websocket JSON: {text}"))?;
                return Ok(Some(json));
            }
            Some(Ok(Message::Binary(bin))) => {
                let json = serde_json::from_slice::<Value>(&bin)
                    .context("invalid binary Matter websocket JSON")?;
                return Ok(Some(json));
            }
            Some(Ok(Message::Ping(payload))) => {
                stream
                    .send(Message::Pong(payload))
                    .await
                    .context("failed to respond to websocket ping")?;
            }
            Some(Ok(Message::Pong(_))) => {}
            Some(Ok(Message::Frame(_))) => {}
            Some(Ok(Message::Close(_))) => return Ok(None),
            Some(Err(err)) => return Err(anyhow!(err).context("Matter websocket stream error")),
            None => return Ok(None),
        }
    }
}

pub fn parse_message(payload: Value) -> MatterMessage {
    if payload.get("event").is_some() {
        if let Ok(event) = serde_json::from_value::<RawEventMessage>(payload.clone()) {
            return MatterMessage::Event {
                event: event.event,
                data: event.data,
            };
        }
    }

    if payload.get("result").is_some() && payload.get("message_id").is_some() {
        if let Ok(success) = serde_json::from_value::<RawSuccessMessage>(payload.clone()) {
            return MatterMessage::Success {
                message_id: success.message_id,
                result: success.result,
            };
        }
    }

    if payload.get("error_code").is_some() {
        if let Ok(err) = serde_json::from_value::<RawErrorMessage>(payload.clone()) {
            return MatterMessage::Error {
                message_id: err.message_id,
                error_code: err.error_code,
                details: err.details,
            };
        }
    }

    if payload.get("schema_version").is_some() && payload.get("fabric_id").is_some() {
        return MatterMessage::ServerInfo(payload);
    }

    MatterMessage::Unknown(payload)
}
