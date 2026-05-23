//! SSH-backed guest bootstrap provider.
//!
//! Splits the single-file implementation into focused submodules:
//! - [`provider`] hosts the [`SshGuestBootstrapProvider`] struct and the
//!   [`GuestBootstrapProvider`] trait implementation.
//! - [`world_creation`] handles world manifest validation and the
//!   `create_world` script construction.
//! - [`image_patching`] owns battlegroup image patch operations and the
//!   JSON-patch helpers used to revise seabass server images.
//! - [`scripts`] contains the embedded shell-script constants and small
//!   shell-quoting helpers shared between the other submodules.
//!
//! [`GuestBootstrapProvider`]: crate::orchestration::GuestBootstrapProvider

mod image_patching;
mod provider;
mod scripts;
mod scripts_kubernetes;
mod world_creation;

pub use provider::SshGuestBootstrapProvider;
