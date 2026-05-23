use crate::{
    models::CommandResult,
    orchestration::{DriveCandidate, HostProvider, HostReadiness, NetworkAdapterCandidate},
};

use super::bridge::StrictPowerShellHyperV;

impl HostProvider for StrictPowerShellHyperV {
    fn readiness(&self) -> CommandResult<HostReadiness> {
        self.run_json(
            "hyperv.host.readiness",
            r#"
$ErrorActionPreference = 'Stop'
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$vmms = Get-Service -Name vmms -ErrorAction SilentlyContinue
$cpu = Get-CimInstance -ClassName Win32_Processor -ErrorAction SilentlyContinue | Select-Object -First 1
$os = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction SilentlyContinue | Select-Object -First 1
$processors = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction SilentlyContinue)
$vmHost = Get-VMHost -ErrorAction SilentlyContinue
$logicalProcessorCount = [uint32]0
foreach ($processor in $processors) {
  if ($processor.NumberOfLogicalProcessors) {
    $logicalProcessorCount += [uint32]$processor.NumberOfLogicalProcessors
  }
}
[pscustomobject]@{
  elevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
  hypervAvailable = [bool](Get-Command Get-VM -ErrorAction SilentlyContinue)
  vmmsRunning = if ($vmms) { $vmms.Status.ToString() -eq 'Running' } else { $false }
  virtualizationFirmwareEnabled = if ($vmHost) { $true } elseif ($cpu) { [bool]$cpu.VirtualizationFirmwareEnabled } else { $null }
  totalPhysicalMemoryBytes = if ($os) { [uint64]$os.TotalVisibleMemorySize * 1024 } else { 0 }
  availablePhysicalMemoryBytes = if ($os) { [uint64]$os.FreePhysicalMemory * 1024 } else { 0 }
  logicalProcessorCount = $logicalProcessorCount
} | ConvertTo-Json -Compress -Depth 4
"#
            .to_string(),
        )
    }

    fn drives_with_minimum_free_space(
        &self,
        minimum_free_bytes: u64,
    ) -> CommandResult<Vec<DriveCandidate>> {
        self.run_json(
            "hyperv.host.drives",
            format!(
                r#"
$ErrorActionPreference = 'Stop'
$items = @(Get-PSDrive -PSProvider FileSystem |
  Where-Object {{ $_.Free -ge {minimum_free_bytes} }} |
  Sort-Object Name |
  ForEach-Object {{
    [pscustomobject]@{{
      name = $_.Name
      root = $_.Root
      freeBytes = [uint64]$_.Free
    }}
  }})
ConvertTo-Json -InputObject $items -Compress -Depth 4
"#
            ),
        )
    }

    fn active_physical_adapters(&self) -> CommandResult<Vec<NetworkAdapterCandidate>> {
        self.run_json(
            "hyperv.host.network-adapters",
            r#"
$ErrorActionPreference = 'Stop'
function ConvertTo-IPv4Int($address) {
  $bytes = [System.Net.IPAddress]::Parse($address).GetAddressBytes()
  [Array]::Reverse($bytes)
  return [BitConverter]::ToUInt32($bytes, 0)
}
function ConvertFrom-IPv4Int([uint32]$value) {
  $bytes = [BitConverter]::GetBytes($value)
  [Array]::Reverse($bytes)
  return ([System.Net.IPAddress]::new($bytes)).ToString()
}
function Get-SuggestedIPv4Address($address, [int]$prefixLength, $gateway) {
  if ([string]::IsNullOrWhiteSpace($address) -or $prefixLength -lt 1 -or $prefixLength -gt 30) {
    return ''
  }
  $ipInt = ConvertTo-IPv4Int $address
  $mask = [uint32]::MaxValue -shl (32 - $prefixLength)
  $network = $ipInt -band $mask
  $broadcast = $network -bor (-bnot $mask)
  $reserved = @{}
  @($address, $gateway) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | ForEach-Object {
    $reserved[$_] = $true
  }
  Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    ForEach-Object { $reserved[$_.IPAddress] = $true }
  Get-NetNeighbor -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object { $_.State -in @('Reachable', 'Stale', 'Delay', 'Probe', 'Permanent') } |
    ForEach-Object { $reserved[$_.IPAddress] = $true }
  for ($candidate = [uint32]($broadcast - 2); $candidate -gt [uint32]($network + 1); $candidate--) {
    $candidateIp = ConvertFrom-IPv4Int $candidate
    if (-not $reserved.ContainsKey($candidateIp)) {
      return $candidateIp
    }
  }
  return ''
}
$switches = @(Get-VMSwitch -SwitchType External -ErrorAction SilentlyContinue)
$adapters = @(Get-NetAdapter)
$items = @(Get-NetAdapter |
  Where-Object { $_.Status -eq 'Up' -and $_.HardwareInterface -eq $true } |
  ForEach-Object {
    $adapter = $_
    $boundSwitch = $switches | Where-Object { $_.NetAdapterInterfaceDescription -eq $adapter.InterfaceDescription } | Select-Object -First 1
    $ipAdapter = $adapter
    if ($boundSwitch) {
      $managementAdapterName = "vEthernet ($($boundSwitch.Name))"
      $managementAdapter = $adapters | Where-Object { $_.Name -eq $managementAdapterName } | Select-Object -First 1
      if ($managementAdapter) {
        $ipAdapter = $managementAdapter
      }
    }
    $ip = Get-NetIPAddress -InterfaceIndex $ipAdapter.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue |
      Where-Object { $_.IPAddress -notlike '169.254.*' } |
      Select-Object -First 1
    $route = Get-NetRoute -InterfaceIndex $ipAdapter.ifIndex -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
      Sort-Object RouteMetric |
      Select-Object -First 1
    if ($ip -and $route -and -not [string]::IsNullOrWhiteSpace($route.NextHop) -and $route.NextHop -ne '0.0.0.0') {
      [pscustomobject]@{
        name = $adapter.Name
        interfaceDescription = $adapter.InterfaceDescription
        ipv4Address = $ip.IPAddress
        prefixLength = [int]$ip.PrefixLength
        gateway = $route.NextHop
        suggestedIpv4Address = Get-SuggestedIPv4Address $ip.IPAddress ([int]$ip.PrefixLength) $route.NextHop
        existingExternalSwitch = if ($boundSwitch) { $boundSwitch.Name } else { '' }
      }
    }
  })
ConvertTo-Json -InputObject $items -Compress -Depth 5
"#
            .to_string(),
        )
    }
}
