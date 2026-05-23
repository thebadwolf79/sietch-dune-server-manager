//! Stdio driver for the unmodified vendor Hyper-V setup script.

mod io;
mod models;
mod prompt_driver;
mod runner;
mod scripts;

pub use models::{VendorHyperVSetupRequest, VendorHyperVSetupResult, VendorPromptAnswer};
pub use runner::VendorHyperVSetupRunner;
