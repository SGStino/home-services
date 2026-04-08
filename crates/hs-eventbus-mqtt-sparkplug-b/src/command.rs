use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use hs_device_contracts::{CapabilityKind, CommandMessage};
use prost::Message;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::sparkplug::{payload, DataType, Payload};

#[derive(Clone, Debug)]
pub struct CommandRoute {
    pub device_id: String,
    pub expected_capabilities: HashMap<String, ExpectedCommandType>,
}

pub type CommandRoutes = Arc<RwLock<HashMap<String, CommandRoute>>>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExpectedCommandType {
    Boolean,
    String,
}

pub fn supports_commands(kind: &CapabilityKind) -> bool {
    matches!(kind, CapabilityKind::Switch | CapabilityKind::Button)
}

pub fn expected_command_type(kind: &CapabilityKind) -> Option<ExpectedCommandType> {
    match kind {
        CapabilityKind::Switch => Some(ExpectedCommandType::Boolean),
        CapabilityKind::Button => Some(ExpectedCommandType::String),
        _ => None,
    }
}

pub fn parse_command_messages(route: &CommandRoute, bytes: &[u8]) -> Result<Vec<CommandMessage>> {
    let payload = Payload::decode(bytes)?;
    let mut commands = Vec::new();

    for metric in payload.metrics {
        let Some(metric_name) = metric.name.clone() else {
            continue;
        };
        let Some(expected_type) = route.expected_capabilities.get(&metric_name).copied() else {
            continue;
        };

        let value = match parse_metric_value(&metric.value) {
            Some(value) => value,
            None => continue,
        };

        if !metric_type_matches(expected_type, metric.datatype, &value) {
            continue;
        }

        commands.push(CommandMessage {
            device_id: route.device_id.clone(),
            capability_id: metric_name,
            payload: value,
        });
    }

    if commands.is_empty() {
        return Err(anyhow!(
            "no valid command metrics found in Sparkplug DCMD payload"
        ));
    }

    Ok(commands)
}

fn parse_metric_value(value: &Option<payload::metric::Value>) -> Option<Value> {
    match value {
        Some(payload::metric::Value::BooleanValue(v)) => Some(Value::Bool(*v)),
        Some(payload::metric::Value::StringValue(v)) => Some(Value::String(v.clone())),
        Some(payload::metric::Value::IntValue(v)) => Some(Value::Number((*v).into())),
        Some(payload::metric::Value::LongValue(v)) => Some(Value::Number((*v).into())),
        Some(payload::metric::Value::FloatValue(v)) => {
            serde_json::Number::from_f64(*v as f64).map(Value::Number)
        }
        Some(payload::metric::Value::DoubleValue(v)) => serde_json::Number::from_f64(*v).map(Value::Number),
        _ => None,
    }
}

fn metric_type_matches(
    expected: ExpectedCommandType,
    metric_datatype: Option<u32>,
    value: &Value,
) -> bool {
    let declared = metric_datatype.and_then(DataType::from_u32);
    match expected {
        ExpectedCommandType::Boolean => {
            if let Some(t) = declared {
                if t != DataType::Boolean {
                    return false;
                }
            }
            matches!(value, Value::Bool(_))
        }
        ExpectedCommandType::String => {
            if let Some(t) = declared {
                if t != DataType::String {
                    return false;
                }
            }
            matches!(value, Value::String(_))
        }
    }
}
