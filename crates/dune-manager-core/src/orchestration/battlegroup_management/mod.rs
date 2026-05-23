//! BattleGroup lifecycle and status orchestration.

mod lifecycle;
mod models;

pub use lifecycle::BattlegroupManagementOrchestrator;
pub use models::{is_started_state, BattlegroupRef, ServiceUrl};
