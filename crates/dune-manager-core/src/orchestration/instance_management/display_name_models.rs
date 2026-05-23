//! Request and result types for setting map display-name overrides.

use serde::Serialize;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{instance_management::instance_map::InstanceMap, BattlegroupRef},
};

/// Request for changing one map dimension's player-facing display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMapDisplayNameRequest {
    /// BattleGroup namespace and resource name.
    pub battlegroup: BattlegroupRef,
    /// Map family to modify.
    pub map: InstanceMap,
    /// Dimension index from the BattleGroup world partition list.
    pub dimension: i64,
    /// New display name. `None` clears the per-partition override.
    pub display_name: Option<String>,
}

impl SetMapDisplayNameRequest {
    /// Creates a request that sets a display-name override.
    pub fn set(
        battlegroup: BattlegroupRef,
        map: InstanceMap,
        dimension: i64,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            battlegroup,
            map,
            dimension,
            display_name: Some(display_name.into()),
        }
    }

    /// Creates a request that removes a display-name override.
    pub fn clear(battlegroup: BattlegroupRef, map: InstanceMap, dimension: i64) -> Self {
        Self {
            battlegroup,
            map,
            dimension,
            display_name: None,
        }
    }

    pub(super) fn validate(&self) -> CommandResult<()> {
        self.battlegroup.validate()?;
        if self.dimension < 0 {
            return Err(failure("--dimension must be zero or greater"));
        }
        if let Some(display_name) = &self.display_name {
            validate_display_name(display_name)?;
        }
        Ok(())
    }
}

/// Result of changing a map dimension display name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMapDisplayNameResult {
    /// Map name that was modified.
    pub map: String,
    /// Dimension index that was modified.
    pub dimension: i64,
    /// Backing partition ID used as the per-partition pod spec index.
    pub partition_id: i64,
    /// Effective display name override after the operation.
    pub display_name: Option<String>,
    /// Whether a BattleGroup restart/reconcile may be required for clients to see the change.
    pub restart_required: bool,
    /// Whether the BattleGroup resource was patched.
    pub battlegroup_patched: bool,
}

fn validate_display_name(value: &str) -> CommandResult<()> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(failure(
            "Display name must be a non-empty single-line value",
        ));
    }
    if value.chars().count() > 128 {
        return Err(failure("Display name must be 128 characters or fewer"));
    }
    Ok(())
}
