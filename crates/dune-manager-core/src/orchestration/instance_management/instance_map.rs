//! Map identifiers supported by instance count and display-name operations.

use serde::{Deserialize, Serialize};

use crate::{errors::failure, models::CommandResult};

/// Supported map family for user-facing instance count operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceMap {
    /// The primary survival map, stored as `Survival_1`.
    Survival1,
    /// The Deep Desert map, stored as `DeepDesert_1`.
    DeepDesert,
}

impl InstanceMap {
    /// Parses a CLI/user map name.
    pub fn parse(value: &str) -> CommandResult<Self> {
        match value.to_ascii_lowercase().as_str() {
            "survival-1" | "survival_1" | "survival" => Ok(Self::Survival1),
            "deep-desert" | "deep_desert" | "deepdesert" | "deepdesert_1" | "deep-desert-1" => {
                Ok(Self::DeepDesert)
            }
            _ => Err(failure(format!(
                "Unsupported instance map {value}; use survival-1 or deep-desert"
            ))),
        }
    }

    /// Returns the Kubernetes/game map name.
    pub fn map_name(self) -> &'static str {
        match self {
            Self::Survival1 => "Survival_1",
            Self::DeepDesert => "DeepDesert_1",
        }
    }
}
