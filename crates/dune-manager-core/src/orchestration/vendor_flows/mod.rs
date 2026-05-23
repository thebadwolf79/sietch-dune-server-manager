//! Vendor flow specifications and battlegroup command catalog.
//!
//! This module documents the vendor PowerShell and shell scripts and the
//! native replacement strategy for each step, plus the catalog of supported
//! battlegroup management commands.

mod battlegroup_flows;
mod battlegroup_flows_part2;
mod battlegroup_flows_part3;
mod flow_models;
mod hyperv_initial_setup_flow;
mod hyperv_initial_setup_flow_part2;
mod hyperv_initial_setup_flow_part3;

pub use battlegroup_flows::*;
pub use flow_models::*;
pub use hyperv_initial_setup_flow::*;
