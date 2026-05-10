use serde_json::Value;

pub fn redact_json(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, child)| {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("token")
                        || lower.contains("secret")
                        || lower.contains("password")
                        || lower.contains("apikey")
                        || lower.contains("auth")
                    {
                        (key, Value::String("<redacted>".to_string()))
                    } else {
                        (key, redact_json(child))
                    }
                })
                .collect::<serde_json::Map<_, _>>(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_json).collect()),
        Value::String(text) if looks_like_jwt(&text) => Value::String("<redacted>".to_string()),
        other => other,
    }
}

pub fn redact_text(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("token")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("apikey")
                || lower.contains("auth")
            {
                "<redacted>".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn looks_like_jwt(value: &str) -> bool {
    let mut parts = value.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(a), Some(b), Some(c), None) if a.len() > 8 && b.len() > 8 && c.len() > 8
    )
}
