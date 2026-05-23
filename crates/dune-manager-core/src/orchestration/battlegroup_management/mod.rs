//! BattleGroup lifecycle, status, and update orchestration.

mod lifecycle;
mod models;
mod update;

pub use lifecycle::BattlegroupManagementOrchestrator;
pub use models::{is_started_state, BattlegroupRef, ServiceUrl};
pub use update::BattlegroupUpdateOrchestrator;
