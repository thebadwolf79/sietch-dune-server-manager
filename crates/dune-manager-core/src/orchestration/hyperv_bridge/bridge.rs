use serde::Deserialize;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        powershell_json_command, StrictCommandRunner, VmInventoryRecord, VmPowerState,
    },
};

/// Hyper-V provider implemented through strict JSON PowerShell commands.
#[derive(Debug, Clone)]
pub struct StrictPowerShellHyperV {
    runner: StrictCommandRunner,
}

impl StrictPowerShellHyperV {
    /// Creates a Hyper-V bridge that invokes local PowerShell.
    pub fn new() -> Self {
        Self {
            runner: StrictCommandRunner,
        }
    }

    pub(super) fn run_json<T: for<'de> Deserialize<'de>>(
        &self,
        id: &'static str,
        script: String,
    ) -> CommandResult<T> {
        self.runner.run_json(&powershell_json_command(id, &script))
    }

    pub(super) fn run_unit(&self, id: &'static str, script: String) -> CommandResult<()> {
        let output: UnitOutput = self.run_json(id, script)?;
        if !output.ok {
            return Err(failure(format!("{id} returned ok=false")));
        }
        Ok(())
    }
}

impl Default for StrictPowerShellHyperV {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnitOutput {
    ok: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawVmRecord {
    name: String,
    state: String,
    configuration_location: String,
    path: String,
    memory_assigned_bytes: Option<u64>,
    processor_count: Option<u32>,
    uptime_seconds: Option<u64>,
    ipv4_addresses: Vec<String>,
    hard_disk_paths: Vec<String>,
    disk_size_bytes: Option<u64>,
    disk_file_size_bytes: Option<u64>,
    switch_names: Vec<String>,
}

impl From<RawVmRecord> for VmInventoryRecord {
    fn from(value: RawVmRecord) -> Self {
        Self {
            name: value.name,
            state: VmPowerState::from_hyperv_state(&value.state),
            raw_state: value.state,
            configuration_location: value.configuration_location,
            path: value.path,
            memory_assigned_bytes: value.memory_assigned_bytes.unwrap_or_default(),
            processor_count: value.processor_count.unwrap_or_default(),
            uptime_seconds: value.uptime_seconds.unwrap_or_default(),
            ipv4_addresses: value.ipv4_addresses,
            hard_disk_paths: value.hard_disk_paths,
            disk_size_bytes: value.disk_size_bytes.unwrap_or_default(),
            disk_file_size_bytes: value.disk_file_size_bytes.unwrap_or_default(),
            switch_names: value.switch_names,
        }
    }
}
