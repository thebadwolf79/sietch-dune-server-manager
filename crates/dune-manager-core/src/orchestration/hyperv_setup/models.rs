use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{errors::failure, models::CommandResult};

/// Default virtual disk size used when importing the vendor VM.
pub const DEFAULT_VM_DISK_BYTES: u64 = 100 * 1024 * 1024 * 1024;

/// Memory presets for the imported dedicated-server VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryProfile {
    /// 20 GiB VM profile for a small Sietch-style server.
    Sietch20Gb,
    /// 30 GiB VM profile for Sietch plus story content.
    SietchStory30Gb,
    /// 40 GiB VM profile for Sietch, story, and Deep Desert content.
    SietchStoryDeepDesert40Gb,
    /// Caller-provided startup memory in bytes.
    CustomBytes(u64),
}

impl MemoryProfile {
    /// Returns the configured memory size in bytes.
    pub fn bytes(self) -> u64 {
        match self {
            Self::Sietch20Gb => 20 * 1024 * 1024 * 1024,
            Self::SietchStory30Gb => 30 * 1024 * 1024 * 1024,
            Self::SietchStoryDeepDesert40Gb => 40 * 1024 * 1024 * 1024,
            Self::CustomBytes(bytes) => bytes,
        }
    }
}

/// Host-side request for importing and preparing the Hyper-V VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperVVmSetupRequest {
    /// Server package folder containing the vendor VM files.
    pub install_path: PathBuf,
    /// Hyper-V VM name to create or replace.
    pub vm_name: String,
    /// Destination folder where VM files are copied.
    pub destination_path: PathBuf,
    /// External switch name to create or reuse.
    pub switch_name: String,
    /// Host network adapter backing the external switch.
    pub adapter_name: String,
    /// Startup memory profile for the VM.
    pub memory: MemoryProfile,
    /// Virtual processor count assigned during initial setup.
    pub processor_count: u32,
    /// Whether an existing VM registration with the same name may be removed.
    pub replace_existing_vm: bool,
    /// Whether an existing destination folder may be deleted first.
    pub clear_destination: bool,
    /// Final virtual disk size in bytes.
    pub disk_size_bytes: u64,
}

impl HyperVVmSetupRequest {
    /// Validates required paths, names, memory, and disk settings.
    pub fn validate(&self) -> CommandResult<()> {
        if self.vm_name.trim().is_empty() {
            return Err(failure("VM name is required"));
        }
        if self.switch_name.trim().is_empty() {
            return Err(failure("Hyper-V switch name is required"));
        }
        if self.adapter_name.trim().is_empty() {
            return Err(failure("Host network adapter name is required"));
        }
        if self.memory.bytes() == 0 {
            return Err(failure("VM memory must be greater than zero"));
        }
        if self.processor_count == 0 {
            return Err(failure("VM processor count must be greater than zero"));
        }
        if self.disk_size_bytes == 0 {
            return Err(failure("VM disk size must be greater than zero"));
        }
        validate_existing_dir(&self.install_path, "server install path")?;
        validate_destination_parent(&self.destination_path)?;
        Ok(())
    }
}

impl Default for HyperVVmSetupRequest {
    fn default() -> Self {
        Self {
            install_path: PathBuf::new(),
            vm_name: String::new(),
            destination_path: PathBuf::new(),
            switch_name: "DuneAwakeningServerSwitch".to_string(),
            adapter_name: String::new(),
            memory: MemoryProfile::Sietch20Gb,
            processor_count: 4,
            replace_existing_vm: false,
            clear_destination: false,
            disk_size_bytes: DEFAULT_VM_DISK_BYTES,
        }
    }
}

/// Host-side result of importing and preparing the Hyper-V VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperVVmSetupResult {
    /// Name of the imported VM.
    pub vm_name: String,
    /// Destination path used for VM files.
    pub destination_path: String,
    /// External switch connected to the VM.
    pub switch_name: String,
    /// Vendor VM configuration file that was imported.
    pub vmcx_path: String,
}

fn validate_existing_dir(path: &Path, label: &str) -> CommandResult<()> {
    if !path.exists() {
        return Err(failure(format!(
            "{label} does not exist: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(failure(format!(
            "{label} is not a directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_destination_parent(path: &Path) -> CommandResult<()> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| failure("VM destination must have a parent directory"))?;
    if !parent.exists() {
        return Err(failure(format!(
            "VM destination parent does not exist: {}",
            parent.display()
        )));
    }
    Ok(())
}
