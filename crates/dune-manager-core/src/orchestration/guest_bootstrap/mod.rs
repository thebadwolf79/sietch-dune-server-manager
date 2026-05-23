//! Guest bootstrap planning, validation, and orchestration of the guest setup sequence.

mod orchestrator;
mod plan;

pub use orchestrator::{GuestBootstrapOrchestrator, GuestBootstrapResult};
pub use plan::{
    host_id_from_self_host_token, random_lowercase_suffix, validate_host_id, validate_region,
    validate_world_name, validate_world_suffix, GuestBootstrapPlan,
};
