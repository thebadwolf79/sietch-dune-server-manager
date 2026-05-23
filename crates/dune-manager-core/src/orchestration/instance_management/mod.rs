//! BattleGroup map instance partition management.
//!
//! The vendor BattleGroup custom resource stores the durable list of map
//! partitions at `spec.database.template.spec.deployment.spec.worldPartitions`.
//! Updating that list, then restarting the BattleGroup, lets the operators and
//! game database converge on additional Survival or Deep Desert instances.
//! Deep Desert instances are distinct partition IDs on dimension zero; Survival
//! instances use distinct dimensions.

mod count_models;
mod display_name_helpers;
mod display_name_models;
mod instance_map;
mod orchestrator;
mod orchestrator_helpers;
mod shell;

pub use count_models::{SetMapInstancesRequest, SetMapInstancesResult};
pub use display_name_models::{SetMapDisplayNameRequest, SetMapDisplayNameResult};
pub use instance_map::InstanceMap;
pub use orchestrator::MapInstanceOrchestrator;

#[cfg(test)]
mod tests_display_name;
#[cfg(test)]
mod tests_fixtures;
#[cfg(test)]
mod tests_instance_count;
