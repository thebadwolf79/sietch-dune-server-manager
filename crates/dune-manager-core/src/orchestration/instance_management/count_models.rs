//! Request and result types for setting map instance counts.

use serde::Serialize;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{instance_management::instance_map::InstanceMap, BattlegroupRef},
};

/// Request for setting the desired number of map partitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetMapInstancesRequest {
    /// BattleGroup namespace and resource name.
    pub battlegroup: BattlegroupRef,
    /// Map family to modify.
    pub map: InstanceMap,
    /// Desired partition count. Must be at least one.
    pub count: usize,
    /// Deep Desert partition IDs that should be marked PvP in user config.
    ///
    /// `None` leaves config files untouched. `Some(Vec::new())` clears the
    /// configured PvP partition list.
    pub pvp_partition_ids: Option<Vec<i64>>,
    /// Number of Deep Desert instances that should be marked PvP.
    ///
    /// When set, the highest selected Deep Desert partition IDs are marked as
    /// PvP and the remaining selected partitions stay PvE. This is the
    /// user-facing setup flow; `pvp_partition_ids` is the lower-level escape
    /// hatch for exact partition control.
    pub pvp_instance_count: Option<usize>,
}

impl SetMapInstancesRequest {
    /// Creates a request without PvP config changes.
    pub fn new(battlegroup: BattlegroupRef, map: InstanceMap, count: usize) -> Self {
        Self {
            battlegroup,
            map,
            count,
            pvp_partition_ids: None,
            pvp_instance_count: None,
        }
    }

    pub(super) fn validate(&self) -> CommandResult<()> {
        self.battlegroup.validate()?;
        if self.count == 0 || self.count > 64 {
            return Err(failure("--count must be between 1 and 64"));
        }
        if self.map == InstanceMap::DeepDesert && self.count > 1 {
            return Err(failure(
                "Only one Deep Desert instance is supported in this build",
            ));
        }
        if self.pvp_partition_ids.is_some() && self.pvp_instance_count.is_some() {
            return Err(failure(
                "Use either explicit PvP partition IDs or a PvP instance count, not both",
            ));
        }
        if let Some(ids) = &self.pvp_partition_ids {
            for id in ids {
                if *id <= 0 {
                    return Err(failure("PvP partition IDs must be positive"));
                }
            }
            if self.map != InstanceMap::DeepDesert {
                return Err(failure(
                    "PvP partition config is currently supported only for deep-desert",
                ));
            }
        }
        if let Some(count) = self.pvp_instance_count {
            if self.map != InstanceMap::DeepDesert {
                return Err(failure(
                    "PvP instance config is currently supported only for deep-desert",
                ));
            }
            if count > self.count {
                return Err(failure(
                    "PvP instance count cannot exceed total instance count",
                ));
            }
        }
        Ok(())
    }
}

/// Result of setting map partitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMapInstancesResult {
    /// Map name that was modified.
    pub map: String,
    /// Partition IDs after the patch.
    pub partition_ids: Vec<i64>,
    /// PvP partition IDs written to config.
    pub pvp_partition_ids: Vec<i64>,
    /// Whether a BattleGroup restart is required for all consumers to see the change.
    pub restart_required: bool,
    /// Whether the BattleGroup resource was patched.
    pub battlegroup_patched: bool,
    /// Whether PvP config files were updated.
    pub pvp_config_updated: bool,
}
