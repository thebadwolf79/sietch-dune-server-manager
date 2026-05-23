//! Shared shell-quoting and JSON descent helpers.

use serde_json::Value;

use crate::{errors::failure, models::CommandResult};

pub(super) fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) fn descend<'a>(value: &'a Value, path: &[&str]) -> CommandResult<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current
            .get(*segment)
            .ok_or_else(|| failure(format!("BattleGroup is missing {segment}")))?;
    }
    Ok(current)
}
