pub fn command_is_on(payload: &serde_json::Value) -> bool {
    match payload {
        serde_json::Value::String(s) => {
            let s = s.trim().to_ascii_uppercase();
            s == "ON" || s == "1" || s == "TRUE"
        }
        serde_json::Value::Bool(v) => *v,
        serde_json::Value::Number(n) => n.as_i64() == Some(1),
        _ => false,
    }
}

pub fn command_is_off(payload: &serde_json::Value) -> bool {
    match payload {
        serde_json::Value::String(s) => {
            let s = s.trim().to_ascii_uppercase();
            s == "OFF" || s == "0" || s == "FALSE"
        }
        serde_json::Value::Bool(v) => !*v,
        serde_json::Value::Number(n) => n.as_i64() == Some(0),
        _ => false,
    }
}
