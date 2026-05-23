//! Ubuntu-over-SSH remote setup phases.

mod kubernetes_bootstrap;
mod kubernetes_scripts;
mod models;
mod operator_yaml;
mod provider;
mod scripts;
mod swap_script;

pub use models::*;
pub use provider::*;
