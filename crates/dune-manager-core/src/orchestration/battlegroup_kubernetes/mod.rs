//! Kubernetes-backed battlegroup queries, patches, shell specs, and log exports.

mod ops;
mod region_patch;
mod types;

pub use ops::StructuredBattlegroupOps;
pub use types::{BattlegroupStatusSnapshot, LogFile, PodContainerRef, PodShellSpec};
