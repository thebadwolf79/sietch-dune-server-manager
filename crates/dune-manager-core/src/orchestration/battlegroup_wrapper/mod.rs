//! Vendor `/home/dune/.dune/bin/battlegroup` wrapper driver.
//!
//! The vendor wrapper is the source of truth for BattleGroup lifecycle and
//! status. This module shells out to it via a [`RemoteCommandRunner`] and
//! parses the human-readable status text into structured fields so the rest
//! of the orchestration code can keep typed [`BattlegroupState`] values.
//!
//! [`RemoteCommandRunner`]: crate::orchestration::RemoteCommandRunner
//! [`BattlegroupState`]: crate::orchestration::BattlegroupState

mod status_parser;
mod wrapper;

pub use status_parser::parse_wrapper_status;
pub use wrapper::{
    BattlegroupWrapperOps, VendorBattlegroupWrapper, WrapperAction, WrapperOutcome,
    VENDOR_EFFECTIVE_USER, VENDOR_WRAPPER_PATH,
};
