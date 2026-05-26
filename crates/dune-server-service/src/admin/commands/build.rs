use serde_json::{json, Map, Value};

use super::{find_command, CommandSpec, FieldKind, FieldSpec, ValidationError};

#[derive(Debug, Clone, Copy)]
pub enum BuildKind {
    /// Inner payload is `{ServerCommand: id, ...validated_values}`.
    Passthrough,
    /// `ServiceBroadcast` has a custom shape (Generic vs ServerShutdown).
    ServiceBroadcast,
}

/// Validate raw form input against a command spec and build the inner JSON
/// payload that goes inside the MQ envelope. Faithful port of
/// `validateAndBuild` from `src/admin/commands.ts`.
pub fn validate_and_build(
    command_id: &str,
    values: &Map<String, Value>,
) -> Result<Value, ValidationError> {
    let spec = find_command(command_id)
        .ok_or_else(|| ValidationError::UnknownCommand(command_id.to_string()))?;

    let mut normalized = Map::new();
    for field in spec.fields {
        let raw = values.get(field.key);
        let is_empty = match raw {
            None => true,
            Some(Value::Null) => true,
            Some(Value::String(s)) if s.is_empty() => true,
            _ => false,
        };
        if is_empty {
            if let Some(default) = default_for(field) {
                normalized.insert(field.key.to_string(), default);
            } else if field.required.unwrap_or(false) {
                return Err(ValidationError::MissingField(field.key.to_string()));
            }
            continue;
        }
        let coerced = coerce(field.kind, raw.unwrap()).ok_or_else(|| {
            ValidationError::WrongType(field.key.to_string(), kind_str(field.kind))
        })?;
        normalized.insert(field.key.to_string(), coerced);
    }

    if matches!(spec.build, BuildKind::ServiceBroadcast) {
        let bt = normalized
            .get("BroadcastType")
            .and_then(|v| v.as_str())
            .unwrap_or("Generic");
        if bt == "Generic"
            && (normalized
                .get("Title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
                || normalized
                    .get("Body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty())
        {
            return Err(ValidationError::BroadcastNeedsTitleAndBody);
        }
    }

    Ok(build(spec, &normalized))
}

pub fn build(spec: &CommandSpec, normalized: &Map<String, Value>) -> Value {
    match spec.build {
        BuildKind::Passthrough => {
            let mut obj = Map::new();
            obj.insert(
                "ServerCommand".to_string(),
                Value::String(spec.id.to_string()),
            );
            for (k, v) in normalized {
                obj.insert(k.clone(), v.clone());
            }
            // The seabass AwardXP server-command handler appears to require
            // `Category` to be present in the payload (otherwise it silently
            // no-ops). The value itself is ignored — every category lands as
            // generic player XP — so we always inject "Combat" so the user
            // doesn't have to see / fill the field.
            if spec.id == "AwardXP" && !obj.contains_key("Category") {
                obj.insert("Category".to_string(), Value::String("Combat".to_string()));
            }
            Value::Object(obj)
        }
        BuildKind::ServiceBroadcast => build_service_broadcast(normalized),
    }
}

fn build_service_broadcast(values: &Map<String, Value>) -> Value {
    let bt = values
        .get("BroadcastType")
        .and_then(|v| v.as_str())
        .unwrap_or("Generic");
    if bt == "ServerShutdown" {
        return json!({
            "ServerCommand": "ServiceBroadcast",
            "BroadcastType": "ServerShutdown",
            "BroadcastPayload": {},
        });
    }
    let title = values
        .get("Title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let body = values
        .get("Body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let duration = values
        .get("BroadcastDuration")
        .and_then(|v| v.as_i64())
        .unwrap_or(30);
    json!({
        "ServerCommand": "ServiceBroadcast",
        "BroadcastType": "Generic",
        "BroadcastPayload": {
            "BroadcastDuration": duration,
            "LocalizedText": [
                {"Key": "en",    "Title": title, "Body": body},
                {"Key": "en-US", "Title": title, "Body": body},
            ]
        }
    })
}

/// The specs file uses const stub defaults so the array can sit in static
/// storage. Real defaults are materialized here at runtime based on field key.
fn default_for(field: &FieldSpec) -> Option<Value> {
    match field.key {
        "Quantity" => Some(json!(1)),
        "Durability" => Some(json!(1.0)),
        "WaterAmount" => Some(json!(1_000_000)),
        "Experience" => Some(json!(1000)),
        "Level" => Some(json!(1)),
        "SkillPoints" => Some(json!(0)),
        "Category" => Some(json!("Combat")),
        "BroadcastType" => Some(json!("Generic")),
        "BroadcastDuration" => Some(json!(30)),
        // TemplateName default removed — the frontend auto-picks the first
        // valid template per the selected vehicle's available list.
        "Persistent" => Some(json!(1.0)),
        _ => None,
    }
}

fn coerce(kind: FieldKind, raw: &Value) -> Option<Value> {
    match kind {
        FieldKind::String | FieldKind::Text | FieldKind::Select => match raw {
            Value::String(s) => Some(Value::String(s.clone())),
            Value::Number(n) => Some(Value::String(n.to_string())),
            Value::Bool(b) => Some(Value::String(b.to_string())),
            _ => None,
        },
        FieldKind::Int => {
            let n = number_value(raw)?;
            if n.fract() == 0.0 && n.is_finite() {
                Some(json!(n as i64))
            } else {
                None
            }
        }
        FieldKind::Float => {
            let n = number_value(raw)?;
            if n.is_finite() {
                Some(json!(n))
            } else {
                None
            }
        }
        FieldKind::Bool => match raw {
            Value::Bool(b) => Some(Value::Bool(*b)),
            Value::String(s) => match s.as_str() {
                "true" | "1" => Some(Value::Bool(true)),
                "false" | "0" => Some(Value::Bool(false)),
                _ => None,
            },
            Value::Number(n) => match n.as_i64() {
                Some(0) => Some(Value::Bool(false)),
                Some(1) => Some(Value::Bool(true)),
                _ => None,
            },
            _ => None,
        },
    }
}

fn number_value(raw: &Value) -> Option<f64> {
    match raw {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn kind_str(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::String => "string",
        FieldKind::Int => "int",
        FieldKind::Float => "float",
        FieldKind::Bool => "bool",
        FieldKind::Select => "select",
        FieldKind::Text => "text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn into_map(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn passthrough_includes_server_command_and_defaults() {
        let raw = into_map(json!({"PlayerId": "P1", "ItemName": "UniqueSda6"}));
        let inner = validate_and_build("AddItemToInventory", &raw).unwrap();
        assert_eq!(inner["ServerCommand"], "AddItemToInventory");
        assert_eq!(inner["PlayerId"], "P1");
        assert_eq!(inner["ItemName"], "UniqueSda6");
        assert_eq!(inner["Quantity"], 1);
        assert_eq!(inner["Durability"], 1.0);
    }

    #[test]
    fn missing_required_field_errors() {
        let raw = into_map(json!({"PlayerId": "P1"}));
        let err = validate_and_build("AddItemToInventory", &raw).unwrap_err();
        assert!(matches!(err, ValidationError::MissingField(ref k) if k == "ItemName"));
    }

    #[test]
    fn unknown_command_errors() {
        let raw = into_map(json!({}));
        let err = validate_and_build("DoesNotExist", &raw).unwrap_err();
        assert!(matches!(err, ValidationError::UnknownCommand(_)));
    }

    #[test]
    fn service_broadcast_generic_requires_title_and_body() {
        let raw = into_map(json!({"BroadcastType": "Generic"}));
        let err = validate_and_build("ServiceBroadcast", &raw).unwrap_err();
        assert!(matches!(err, ValidationError::BroadcastNeedsTitleAndBody));

        let ok_raw = into_map(json!({"Title": "Hi", "Body": "msg", "BroadcastDuration": 15}));
        let inner = validate_and_build("ServiceBroadcast", &ok_raw).unwrap();
        assert_eq!(inner["BroadcastType"], "Generic");
        assert_eq!(inner["BroadcastPayload"]["BroadcastDuration"], 15);
    }

    #[test]
    fn service_broadcast_server_shutdown_uses_empty_payload() {
        let raw = into_map(json!({"BroadcastType": "ServerShutdown"}));
        let inner = validate_and_build("ServiceBroadcast", &raw).unwrap();
        assert_eq!(inner["BroadcastType"], "ServerShutdown");
        assert_eq!(inner["BroadcastPayload"], json!({}));
    }

    #[test]
    fn int_field_rejects_floats() {
        let raw = into_map(json!({"PlayerId": "P", "ItemName": "X", "Quantity": 2.5}));
        let err = validate_and_build("AddItemToInventory", &raw).unwrap_err();
        assert!(matches!(err, ValidationError::WrongType(_, "int")));
    }
}
