$ErrorActionPreference = 'Stop'
$scriptDir = __SCRIPT_DIR__
$script:__managerPreferredDrive = __DRIVE__
$script:__managerAdapterName = __ADAPTER__
$script:__managerMemoryChoice = __MEMORY_CHOICE__
$script:__managerMemoryGb = __MEMORY_GB__
$script:__managerStaticNetwork = __STATIC_NETWORK__
$script:__managerStaticIp = __STATIC_IP__
$script:__managerGateway = __GATEWAY__
$script:__managerDns = __DNS__
$script:__managerPlayerIp = __PLAYER_IP__
$script:__managerEnableSwap = __ENABLE_SWAP__
$script:__managerRemoteSetupInputB64 = __REMOTE_SETUP_INPUT_B64__
$script:__managerRemoteSetupScriptB64 = __REMOTE_SETUP_SCRIPT_B64__
$script:__managerChoice12Count = 0
$script:__managerAskPass = Join-Path $env:TEMP ("dune-manager-askpass-" + ([guid]::NewGuid().ToString('N')) + ".cmd")
$script:__managerPasswordBytes = New-Object byte[] 18
$script:__managerRng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$script:__managerRng.GetBytes($script:__managerPasswordBytes)
$script:__managerRng.Dispose()
$script:__managerNewVmPassword = "Dune-" + ([Convert]::ToBase64String($script:__managerPasswordBytes).TrimEnd('=').Replace('+','A').Replace('/','B')) + "!7"
Set-Content -LiteralPath $script:__managerAskPass -Encoding ASCII -Value "@echo off`r`necho dune"
$env:SSH_ASKPASS = $script:__managerAskPass
$env:SSH_ASKPASS_REQUIRE = 'force'
$env:DISPLAY = 'dune-manager'
function Invoke-ManagerSsh {
    param([object[]]$SshArgs)
    $oldErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & ssh.exe @SshArgs
    } finally {
        $script:__managerSshExitCode = $LASTEXITCODE
        $ErrorActionPreference = $oldErrorActionPreference
        $global:LASTEXITCODE = $script:__managerSshExitCode
    }
}
function ssh {
    $sshArgs = @($args)
    if ($sshArgs.Count -gt 0 -and $sshArgs[$sshArgs.Count - 1] -eq '/home/dune/.dune/bin/setup') {
        $forwardArgs = @()
        if ($sshArgs.Count -gt 1) { $forwardArgs = $sshArgs[0..($sshArgs.Count - 2)] }
        $remoteCommand = "SETUP_INPUT_B64='$script:__managerRemoteSetupInputB64' sh -c 'echo $script:__managerRemoteSetupScriptB64 | base64 -d | sh'"
        Invoke-ManagerSsh -SshArgs ($forwardArgs + @($remoteCommand))
        return
    }
    Invoke-ManagerSsh -SshArgs $sshArgs
}
function Read-Host {
    param(
        [string]$Prompt,
        [switch]$AsSecureString
    )
    $answer = ''
    if ($Prompt.StartsWith('Select drive')) {
        $drives = (Get-Variable -Name availableDrives -Scope 1 -ErrorAction SilentlyContinue).Value
        if ($drives) {
            for ($i = 0; $i -lt $drives.Count; $i++) {
                $name = [string]$drives[$i].Name
                if ($name.TrimEnd(':').ToUpperInvariant() -eq $script:__managerPreferredDrive) {
                    $answer = [string]($i + 1)
                    break
                }
            }
        }
        if ([string]::IsNullOrWhiteSpace($answer)) { $answer = '1' }
    } elseif ($Prompt.StartsWith('Do you want to remove it')) {
        $answer = 'N'
    } elseif ($Prompt.StartsWith('Turn off the VM now')) {
        $answer = 'N'
    } elseif ($Prompt.StartsWith('Incompatibilities detected')) {
        $answer = 'N'
    } elseif ($Prompt.StartsWith('Add external switch')) {
        $answer = 'Y'
    } elseif ($Prompt.StartsWith('Select adapter')) {
        $nics = (Get-Variable -Name physicalNics -Scope 1 -ErrorAction SilentlyContinue).Value
        if ($nics) {
            for ($i = 0; $i -lt $nics.Count; $i++) {
                if ([string]$nics[$i].Name -eq $script:__managerAdapterName) {
                    $answer = [string]($i + 1)
                    break
                }
            }
        }
        if ([string]::IsNullOrWhiteSpace($answer)) { $answer = '1' }
    } elseif ($Prompt.StartsWith('Enter choice [1/2/3/4/5]')) {
        $answer = $script:__managerMemoryChoice
    } elseif ($Prompt.StartsWith('Enter memory in GB')) {
        $answer = $script:__managerMemoryGb
    } elseif ($Prompt.StartsWith('Would you like to change the default password')) {
        $answer = 'N'
    } elseif ($Prompt.StartsWith('Enter new password')) {
        $answer = $script:__managerNewVmPassword
    } elseif ($Prompt.StartsWith('Confirm new password')) {
        $answer = $script:__managerNewVmPassword
    } elseif ($Prompt.StartsWith('Choice [1/2]')) {
        $script:__managerChoice12Count++
        if ($script:__managerStaticNetwork) { $answer = '2' } else { $answer = '1' }
    } elseif ($Prompt.StartsWith('Enter the static IP for the VM')) {
        $answer = $script:__managerStaticIp
    } elseif ($Prompt.StartsWith('Enter the CIDR suffix')) {
        $answer = '/24'
    } elseif ($Prompt.StartsWith('Enter the gateway IP')) {
        $answer = $script:__managerGateway
    } elseif ($Prompt.StartsWith('Enter the DNS server')) {
        $answer = $script:__managerDns
    } elseif ($Prompt -eq 'Choice') {
        if ([string]::IsNullOrWhiteSpace($script:__managerPlayerIp)) { $answer = '2' } else { $answer = '3' }
    } elseif ($Prompt.StartsWith('Enter IP')) {
        $answer = $script:__managerPlayerIp
    } elseif ($Prompt.StartsWith('Steam download failed')) {
        $answer = 'N'
    } elseif ($Prompt.StartsWith('Enable experimental swap memory now')) {
        if ($script:__managerEnableSwap) { $answer = 'Y' } else { $answer = 'N' }
    } else {
        Write-Host "[manager] Unrecognized vendor prompt: $Prompt" -ForegroundColor Yellow
    }
    if ($AsSecureString) {
        $secure = New-Object System.Security.SecureString
        foreach ($ch in $answer.ToCharArray()) { $secure.AppendChar($ch) }
        $secure.MakeReadOnly()
        return $secure
    }
    Write-Host "[manager] Answered vendor prompt: $Prompt" -ForegroundColor DarkGray
    return $answer
}
try {
    . (Join-Path $scriptDir 'initial-setup.ps1')
} finally {
    Remove-Item -LiteralPath $script:__managerAskPass -Force -ErrorAction SilentlyContinue
}