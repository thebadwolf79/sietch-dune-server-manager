//! PowerShell-backed Hyper-V provider implementation, grouped by Hyper-V domain
//! (bridge core, host operations, VM operations).

mod bridge;
mod host_operations;
mod vm_operations;

#[cfg(test)]
mod tests;

pub use bridge::StrictPowerShellHyperV;
