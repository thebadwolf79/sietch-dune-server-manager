//! Region patch operation building and shell quoting helpers.

use serde_json::{json, Value};

use crate::{errors::failure, models::CommandResult};

pub(super) fn validate_region(region: &str) -> CommandResult<()> {
    match region {
        "Asia" | "Europe" | "North America" | "Oceania" | "South America" => Ok(()),
        _ => Err(failure(
            "Region must be Asia, Europe, North America, Oceania, or South America",
        )),
    }
}

pub(super) fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn region_patch_operations(value: &Value, region: &str) -> CommandResult<Vec<Value>> {
    let mut operations = Vec::new();
    collect_region_patch_operations(value, &mut Vec::new(), region, &mut operations);
    if operations.is_empty() {
        return Err(failure("No battlegroup region fields were found to patch"));
    }
    Ok(operations)
}

fn collect_region_patch_operations(
    value: &Value,
    path: &mut Vec<String>,
    region: &str,
    operations: &mut Vec<Value>,
) {
    match value {
        Value::Object(map) => {
            if map
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name == "BATTLEGROUP_REGION_NAME")
                && map.get("value").is_some()
            {
                let mut value_path = path.clone();
                value_path.push("value".to_string());
                operations.push(replace_operation(&value_path, json!(region)));
            }

            for (key, child) in map {
                path.push(key.clone());
                if key == "dataCenter" && child.is_string() {
                    operations.push(replace_operation(path, json!(region)));
                }
                collect_region_patch_operations(child, path, region, operations);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push(index.to_string());
                if child
                    .as_str()
                    .is_some_and(|text| text.starts_with("-FarmRegion="))
                {
                    operations.push(replace_operation(
                        path,
                        json!(format!("-FarmRegion={region}")),
                    ));
                }
                collect_region_patch_operations(child, path, region, operations);
                path.pop();
            }
        }
        _ => {}
    }
}

fn replace_operation(path: &[String], value: Value) -> Value {
    json!({
        "op": "replace",
        "path": json_pointer(path),
        "value": value,
    })
}

fn json_pointer(path: &[String]) -> String {
    format!(
        "/{}",
        path.iter()
            .map(|item| item.replace('~', "~0").replace('/', "~1"))
            .collect::<Vec<_>>()
            .join("/")
    )
}
