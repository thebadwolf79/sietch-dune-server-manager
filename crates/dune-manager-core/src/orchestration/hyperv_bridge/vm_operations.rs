use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        EnsureSwitchRequest, ExternalSwitch, VmCompatibilityReport, VmImportRequest,
        VmInventoryRecord, VmProvider,
    },
    shell::ps_single_quoted,
};

use super::bridge::{RawVmRecord, StrictPowerShellHyperV};

impl VmProvider for StrictPowerShellHyperV {
    fn list_vms(&self) -> CommandResult<Vec<VmInventoryRecord>> {
        let raw: Vec<RawVmRecord> = self.run_json(
            "hyperv.vm.list",
            r#"
$ErrorActionPreference = 'Stop'
$items = @(Get-VM | Sort-Object Name | ForEach-Object {
  $vm = $_
  $adapters = @(Get-VMNetworkAdapter -VMName $vm.Name -ErrorAction SilentlyContinue)
  $ips = @($adapters.IPAddresses | Where-Object { $_ -match '^\d+\.\d+\.\d+\.\d+$' })
  $ips = @($ips | Where-Object { $_ -match '^\d+\.\d+\.\d+\.\d+$' } | Select-Object -Unique)
  if ($ips.Count -eq 0) {
    $ips = @($adapters | ForEach-Object {
      $m = $_.MacAddress; if ($m.Length -ne 12) { return }
      $fmt = ($m -replace '(.{2})(.{2})(.{2})(.{2})(.{2})(.{2})', '$1-$2-$3-$4-$5-$6').ToUpper()
      Get-NetNeighbor -AddressFamily IPv4 -ErrorAction SilentlyContinue |
        Where-Object { $_.LinkLayerAddress -ieq $fmt -and $_.IPAddress -notlike '169.254.*' -and $_.State -in @('Reachable','Stale','Delay','Probe','Permanent') } |
        Select-Object -ExpandProperty IPAddress -First 1
    } | Where-Object { $_ } | Select-Object -Unique)
  }
  $switches = @($adapters | ForEach-Object { $_.SwitchName } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
  $disks = @(Get-VMHardDiskDrive -VMName $vm.Name -ErrorAction SilentlyContinue | ForEach-Object { $_.Path })
  $diskSizeBytes = [uint64]0
  $diskFileSizeBytes = [uint64]0
  foreach ($diskPath in $disks) {
    $vhd = Get-VHD -Path $diskPath -ErrorAction SilentlyContinue
    if ($vhd) {
      $diskSizeBytes += [uint64]$vhd.Size
      $diskFileSizeBytes += [uint64]$vhd.FileSize
    }
  }
  [pscustomobject]@{
    name = $vm.Name
    state = $vm.State.ToString()
    configurationLocation = $vm.ConfigurationLocation
    path = $vm.Path
    memoryAssignedBytes = [uint64]$vm.MemoryAssigned
    processorCount = [uint32]$vm.ProcessorCount
    uptimeSeconds = [uint64]$vm.Uptime.TotalSeconds
    ipv4Addresses = $ips
    hardDiskPaths = $disks
    diskSizeBytes = $diskSizeBytes
    diskFileSizeBytes = $diskFileSizeBytes
    switchNames = $switches
  }
})
ConvertTo-Json -InputObject $items -Compress -Depth 6
"#
            .to_string(),
        )?;
        Ok(raw.into_iter().map(Into::into).collect())
    }

    fn get_vm(&self, name: &str) -> CommandResult<Option<VmInventoryRecord>> {
        let raw: Option<RawVmRecord> = self.run_json(
            "hyperv.vm.get",
            format!(
                r#"
$ErrorActionPreference = 'Stop'
$vmName = {name}
$vm = Get-VM -Name $vmName -ErrorAction SilentlyContinue
if (-not $vm) {{
  [Console]::Out.Write('null')
  exit 0
}}
$adapters = @(Get-VMNetworkAdapter -VMName $vm.Name -ErrorAction SilentlyContinue)
$ips = @($adapters.IPAddresses | Where-Object {{ $_ -match '^\d+\.\d+\.\d+\.\d+$' }})
$ips = @($ips | Where-Object {{ $_ -match '^\d+\.\d+\.\d+\.\d+$' }} | Select-Object -Unique)
if ($ips.Count -eq 0) {{
  $ips = @($adapters | ForEach-Object {{
    $m = $_.MacAddress; if ($m.Length -ne 12) {{ return }}
    $fmt = ($m -replace '(.{{2}})(.{{2}})(.{{2}})(.{{2}})(.{{2}})(.{{2}})', '$1-$2-$3-$4-$5-$6').ToUpper()
    Get-NetNeighbor -AddressFamily IPv4 -ErrorAction SilentlyContinue |
      Where-Object {{ $_.LinkLayerAddress -ieq $fmt -and $_.IPAddress -notlike '169.254.*' -and $_.State -in @('Reachable','Stale','Delay','Probe','Permanent') }} |
      Select-Object -ExpandProperty IPAddress -First 1
  }} | Where-Object {{ $_ }} | Select-Object -Unique)
}}
$switches = @($adapters | ForEach-Object {{ $_.SwitchName }} | Where-Object {{ -not [string]::IsNullOrWhiteSpace($_) }} | Sort-Object -Unique)
$disks = @(Get-VMHardDiskDrive -VMName $vm.Name -ErrorAction SilentlyContinue | ForEach-Object {{ $_.Path }})
$diskSizeBytes = [uint64]0
$diskFileSizeBytes = [uint64]0
foreach ($diskPath in $disks) {{
  $vhd = Get-VHD -Path $diskPath -ErrorAction SilentlyContinue
  if ($vhd) {{
    $diskSizeBytes += [uint64]$vhd.Size
    $diskFileSizeBytes += [uint64]$vhd.FileSize
  }}
}}
[pscustomobject]@{{
  name = $vm.Name
  state = $vm.State.ToString()
  configurationLocation = $vm.ConfigurationLocation
  path = $vm.Path
  memoryAssignedBytes = [uint64]$vm.MemoryAssigned
  processorCount = [uint32]$vm.ProcessorCount
  uptimeSeconds = [uint64]$vm.Uptime.TotalSeconds
  ipv4Addresses = $ips
  hardDiskPaths = $disks
  diskSizeBytes = $diskSizeBytes
  diskFileSizeBytes = $diskFileSizeBytes
  switchNames = $switches
}} | ConvertTo-Json -Compress -Depth 5
"#,
                name = ps_single_quoted(name)
            ),
        )?;
        Ok(raw.map(Into::into))
    }

    fn compare_import(&self, request: &VmImportRequest) -> CommandResult<VmCompatibilityReport> {
        self.run_json(
            "hyperv.vm.compare-import",
            format!(
                r#"
$ErrorActionPreference = 'Stop'
$report = Compare-VM -Path {vmcx} -Copy -GenerateNewId -VirtualMachinePath {dest} -VhdDestinationPath (Join-Path {dest} 'Virtual Hard Disks') -ErrorAction Stop
$messages = @($report.Incompatibilities | ForEach-Object {{ $_.Message }})
[pscustomobject]@{{
  compatible = $messages.Count -eq 0
  incompatibilities = $messages
}} | ConvertTo-Json -Compress -Depth 6
"#,
                vmcx = ps_single_quoted(&request.vmcx_path),
                dest = ps_single_quoted(&request.destination_path)
            ),
        )
    }

    fn import_vm(
        &self,
        request: &VmImportRequest,
    ) -> CommandResult<crate::orchestration::ImportedVm> {
        self.run_json(
            "hyperv.vm.import",
            format!(
                r#"
$ErrorActionPreference = 'Stop'
$report = Compare-VM -Path {vmcx} -Copy -GenerateNewId -VirtualMachinePath {dest} -VhdDestinationPath (Join-Path {dest} 'Virtual Hard Disks') -ErrorAction Stop
$vm = Import-VM -CompatibilityReport $report -ErrorAction Stop
[pscustomobject]@{{
  name = $vm.Name
  configurationLocation = $vm.ConfigurationLocation
}} | ConvertTo-Json -Compress -Depth 4
"#,
                vmcx = ps_single_quoted(&request.vmcx_path),
                dest = ps_single_quoted(&request.destination_path)
            ),
        )
    }

    fn remove_vm(&self, name: &str) -> CommandResult<()> {
        self.run_unit(
            "hyperv.vm.remove",
            format!(
                "Remove-VM -Name {} -Force -ErrorAction Stop; [pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress",
                ps_single_quoted(name)
            ),
        )
    }

    fn start_vm(&self, name: &str) -> CommandResult<()> {
        self.run_unit(
            "hyperv.vm.start",
            format!(
                "Start-VM -Name {} -ErrorAction Stop | Out-Null; [pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress",
                ps_single_quoted(name)
            ),
        )
    }

    fn stop_vm(&self, name: &str, turn_off: bool) -> CommandResult<()> {
        let flag = if turn_off { " -TurnOff" } else { "" };
        self.run_unit(
            "hyperv.vm.stop",
            format!(
                "Stop-VM -Name {}{flag} -Force -ErrorAction Stop | Out-Null; [pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress",
                ps_single_quoted(name)
            ),
        )
    }

    fn connect_network_adapter(&self, vm_name: &str, switch_name: &str) -> CommandResult<()> {
        self.run_unit(
            "hyperv.vm.connect-network-adapter",
            format!(
                "Connect-VMNetworkAdapter -VMName {} -SwitchName {} -ErrorAction Stop; [pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress",
                ps_single_quoted(vm_name),
                ps_single_quoted(switch_name)
            ),
        )
    }

    fn ensure_external_switch(
        &self,
        request: &EnsureSwitchRequest,
    ) -> CommandResult<ExternalSwitch> {
        self.run_json(
            "hyperv.switch.ensure-external",
            format!(
                r#"
$ErrorActionPreference = 'Stop'
$switchName = {switch_name}
$adapterName = {adapter_name}
$adapter = Get-NetAdapter -Name $adapterName -ErrorAction Stop
$switch = Get-VMSwitch -SwitchType External -ErrorAction SilentlyContinue |
  Where-Object {{ $_.NetAdapterInterfaceDescription -eq $adapter.InterfaceDescription }} |
  Select-Object -First 1
if (-not $switch) {{
  $switch = New-VMSwitch -Name $switchName -NetAdapterName $adapterName -AllowManagementOS $true -ErrorAction Stop
}}
[pscustomobject]@{{
  name = $switch.Name
  netAdapterInterfaceDescription = $switch.NetAdapterInterfaceDescription
}} | ConvertTo-Json -Compress -Depth 4
"#,
                switch_name = ps_single_quoted(&request.switch_name),
                adapter_name = ps_single_quoted(&request.adapter_name)
            ),
        )
    }

    fn resize_first_vhd(&self, vm_name: &str, size_bytes: u64) -> CommandResult<()> {
        self.run_unit(
            "hyperv.vhd.resize-first",
            format!(
                r#"
$ErrorActionPreference = 'Stop'
$drive = Get-VMHardDiskDrive -VMName {vm_name} | Select-Object -First 1
if (-not $drive) {{ throw 'VM has no hard disk drive' }}
Resize-VHD -Path $drive.Path -SizeBytes {size_bytes} -ErrorAction Stop
[pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress
"#,
                vm_name = ps_single_quoted(vm_name)
            ),
        )
    }

    fn set_first_boot_disk(&self, vm_name: &str) -> CommandResult<()> {
        self.run_unit(
            "hyperv.vm.set-first-boot-disk",
            format!(
                r#"
$ErrorActionPreference = 'Stop'
$drive = Get-VMHardDiskDrive -VMName {vm_name} | Select-Object -First 1
if (-not $drive) {{ throw 'VM has no hard disk drive' }}
Set-VMFirmware -VMName {vm_name} -FirstBootDevice $drive -ErrorAction Stop
[pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress
"#,
                vm_name = ps_single_quoted(vm_name)
            ),
        )
    }

    fn set_startup_memory(&self, vm_name: &str, bytes: u64) -> CommandResult<()> {
        if bytes == 0 {
            return Err(failure("VM memory must be greater than zero"));
        }
        self.run_unit(
            "hyperv.vm.set-startup-memory",
            format!(
                "Set-VMMemory -VMName {} -StartupBytes {bytes} -ErrorAction Stop; [pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress",
                ps_single_quoted(vm_name)
            ),
        )
    }

    fn set_processor_count(&self, vm_name: &str, count: u32) -> CommandResult<()> {
        if count == 0 {
            return Err(failure("VM processor count must be greater than zero"));
        }
        self.run_unit(
            "hyperv.vm.set-processor-count",
            format!(
                "Set-VMProcessor -VMName {} -Count {count} -ErrorAction Stop; [pscustomobject]@{{ ok = $true }} | ConvertTo-Json -Compress",
                ps_single_quoted(vm_name)
            ),
        )
    }
}
