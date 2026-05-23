use serde_json::{json, Value};

use crate::{errors::failure, models::CommandResult};

pub(super) fn battlegroup_image_patch_operations(
    value: &Value,
    new_version: &str,
) -> CommandResult<Vec<Value>> {
    let mut operations = Vec::new();
    collect_battlegroup_image_patch_operations(
        value,
        &mut Vec::new(),
        new_version,
        &mut operations,
    );
    if operations.is_empty() {
        return Err(failure("No battlegroup server images were found to patch"));
    }
    Ok(operations)
}

fn collect_battlegroup_image_patch_operations(
    value: &Value,
    path: &mut Vec<String>,
    new_version: &str,
    operations: &mut Vec<Value>,
) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                path.push(key.clone());
                if key == "image" {
                    if let Some(updated) = child
                        .as_str()
                        .and_then(|image| revised_seabass_server_image(image, new_version))
                    {
                        operations.push(replace_operation(path, json!(updated)));
                    }
                }
                collect_battlegroup_image_patch_operations(child, path, new_version, operations);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                path.push(index.to_string());
                collect_battlegroup_image_patch_operations(child, path, new_version, operations);
                path.pop();
            }
        }
        _ => {}
    }
}

fn revised_seabass_server_image(image: &str, new_version: &str) -> Option<String> {
    let file = image.rsplit('/').next().unwrap_or(image);
    if !file.starts_with("seabass-server") {
        return None;
    }
    let (prefix, _) = image.rsplit_once(':')?;
    Some(format!("{prefix}:{new_version}"))
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
