//! Stdio driver for the unmodified vendor Hyper-V setup script.

use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{OperationSink, OrchestrationEvent, ProviderKind, StepAction, StepDomain},
    shell::{ps_single_quoted, suppress_console_window},
};

/// Request used to answer the vendor Hyper-V setup prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorHyperVSetupRequest {
    /// Preferred VM destination from the UI. The vendor script only uses the drive.
    pub vm_destination: PathBuf,
    /// Selected network adapter name from the UI.
    pub adapter_name: String,
    /// Requested VM memory in GiB.
    pub memory_gb: u64,
    /// Whether the guest should use static networking.
    pub static_network: bool,
    /// Static guest IP when static networking is selected.
    pub static_ip: String,
    /// Static gateway when static networking is selected.
    pub gateway: String,
    /// Static DNS server when static networking is selected.
    pub dns: String,
    /// Player-facing IP written to server settings.
    pub player_ip: String,
    /// World name for downstream vendor setup prompts.
    pub world_name: String,
    /// Region name for downstream vendor setup prompts.
    pub region: String,
    /// Self-host token for downstream vendor setup prompts.
    pub self_host_token: String,
    /// Whether to accept the low-memory experimental swap prompt.
    pub enable_swap: bool,
}

impl VendorHyperVSetupRequest {
    /// Returns the drive letter the vendor script can use for installation.
    pub fn preferred_drive_name(&self) -> Option<String> {
        self.vm_destination
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_string_lossy().chars().next())
            .filter(|character| character.is_ascii_alphabetic())
            .map(|character| character.to_ascii_uppercase().to_string())
    }
}

/// Result from running the vendor Hyper-V setup script.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorHyperVSetupResult {
    /// Vendor script path that was executed.
    pub script_path: PathBuf,
    /// SHA-256 of the vendor script at execution time.
    pub script_sha256: String,
}

/// Prompt/answer row emitted by the dry-run harness and tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VendorPromptAnswer {
    /// Stable prompt identifier.
    pub prompt_id: &'static str,
    /// Redacted answer value.
    pub answer: String,
}

/// Runs the unmodified vendor Hyper-V setup script through stdio.
pub struct VendorHyperVSetupRunner {
    package_dir: PathBuf,
}

impl VendorHyperVSetupRunner {
    /// Creates a runner for a detected server package directory.
    pub fn new(package_dir: impl Into<PathBuf>) -> Self {
        Self {
            package_dir: package_dir.into(),
        }
    }

    /// Executes the vendor setup script with an in-process `Read-Host` answer shim.
    pub fn run(
        &self,
        request: &VendorHyperVSetupRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<VendorHyperVSetupResult> {
        let script_dir = self.script_dir()?;
        let script_path = script_dir.join("initial-setup.ps1");
        let script_sha256 = sha256_file(&script_path)?;
        emit(
            sink,
            "vendor.hyperv.script",
            format!(
                "Running vendor Hyper-V setup script {} (sha256 {}).",
                script_path.display(),
                script_sha256
            ),
        );

        let wrapper_path = write_wrapper_script(&vendor_powershell_wrapper(request, &script_dir))?;
        let mut command = Command::new("powershell");
        command
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                &wrapper_path.to_string_lossy(),
            ])
            .current_dir(&script_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        suppress_console_window(&mut command);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = fs::remove_file(&wrapper_path);
                return Err(failure(format!(
                    "Failed to start vendor Hyper-V setup script: {err}"
                )));
            }
        };
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| failure("Failed to open vendor setup stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| failure("Failed to open vendor setup stderr"))?;
        let (tx, rx) = mpsc::channel();
        spawn_reader(stdout, StreamKind::Stdout, tx.clone());
        spawn_reader(stderr, StreamKind::Stderr, tx);

        let mut stdout_text = String::new();
        let mut stderr_text = String::new();
        let mut stdout_line = String::new();
        let mut stderr_line = String::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(150)) {
                Ok(StreamChunk { kind, text }) => match kind {
                    StreamKind::Stdout => {
                        stdout_text.push_str(&text);
                        emit_lines(sink, "vendor.stdout", &mut stdout_line, &text);
                    }
                    StreamKind::Stderr => {
                        stderr_text.push_str(&text);
                        emit_lines(sink, "vendor.stderr", &mut stderr_line, &text);
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(status) = child
                        .try_wait()
                        .map_err(|err| failure(format!("Failed to poll vendor setup: {err}")))?
                    {
                        flush_line(sink, "vendor.stdout", &mut stdout_line);
                        flush_line(sink, "vendor.stderr", &mut stderr_line);
                        let _ = fs::remove_file(&wrapper_path);
                        if status.success() {
                            return Ok(VendorHyperVSetupResult {
                                script_path,
                                script_sha256,
                            });
                        }
                        return Err(failure(format!(
                            "Vendor Hyper-V setup script exited with {status}. Last stderr: {}",
                            last_non_empty_line(&stderr_text).unwrap_or_else(|| "none".to_string())
                        )));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = child.wait().map_err(|err| {
                        failure(format!("Failed to wait for vendor setup: {err}"))
                    })?;
                    flush_line(sink, "vendor.stdout", &mut stdout_line);
                    flush_line(sink, "vendor.stderr", &mut stderr_line);
                    let _ = fs::remove_file(&wrapper_path);
                    if status.success() {
                        return Ok(VendorHyperVSetupResult {
                            script_path,
                            script_sha256,
                        });
                    }
                    return Err(failure(format!(
                        "Vendor Hyper-V setup script exited with {status}. Last stderr: {}",
                        last_non_empty_line(&stderr_text).unwrap_or_else(|| "none".to_string())
                    )));
                }
            }
        }
    }

    /// Exercises prompt matching against a transcript without starting PowerShell.
    pub fn dry_run_answers(
        &self,
        request: &VendorHyperVSetupRequest,
        transcript: &str,
    ) -> Vec<VendorPromptAnswer> {
        let mut driver = VendorPromptDriver::new(request.clone());
        driver.observe(transcript, "")
    }

    fn script_dir(&self) -> CommandResult<PathBuf> {
        let candidates = [
            self.package_dir.join("battlegroup-management"),
            self.package_dir.join("internal-scripts"),
        ];
        candidates
            .into_iter()
            .find(|path| path.join("initial-setup.ps1").is_file())
            .ok_or_else(|| {
                failure(format!(
                    "Could not find vendor initial-setup.ps1 under {}",
                    self.package_dir.display()
                ))
            })
    }
}

fn write_wrapper_script(wrapper: &str) -> CommandResult<PathBuf> {
    let path = env::temp_dir().join(format!(
        "dune-vendor-hyperv-setup-{}.ps1",
        std::process::id()
    ));
    fs::write(&path, wrapper)
        .map_err(|err| failure(format!("Failed to write vendor setup wrapper: {err}")))?;
    Ok(path)
}

fn vendor_powershell_wrapper(request: &VendorHyperVSetupRequest, script_dir: &Path) -> String {
    format!(
        r#"$ErrorActionPreference = 'Stop'
$scriptDir = {script_dir}
$script:__managerPreferredDrive = {drive}
$script:__managerAdapterName = {adapter}
$script:__managerMemoryChoice = {memory_choice}
$script:__managerMemoryGb = {memory_gb}
$script:__managerStaticNetwork = {static_network}
$script:__managerStaticIp = {static_ip}
$script:__managerGateway = {gateway}
$script:__managerDns = {dns}
$script:__managerPlayerIp = {player_ip}
$script:__managerEnableSwap = {enable_swap}
$script:__managerRemoteSetupInputB64 = {remote_setup_input_b64}
$script:__managerRemoteSetupScriptB64 = {remote_setup_script_b64}
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
function Invoke-ManagerSsh {{
    param([object[]]$SshArgs)
    $oldErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {{
        & ssh.exe @SshArgs
    }} finally {{
        $script:__managerSshExitCode = $LASTEXITCODE
        $ErrorActionPreference = $oldErrorActionPreference
        $global:LASTEXITCODE = $script:__managerSshExitCode
    }}
}}
function ssh {{
    $sshArgs = @($args)
    if ($sshArgs.Count -gt 0 -and $sshArgs[$sshArgs.Count - 1] -eq '/home/dune/.dune/bin/setup') {{
        $forwardArgs = @()
        if ($sshArgs.Count -gt 1) {{ $forwardArgs = $sshArgs[0..($sshArgs.Count - 2)] }}
        $remoteCommand = "SETUP_INPUT_B64='$script:__managerRemoteSetupInputB64' sh -c 'echo $script:__managerRemoteSetupScriptB64 | base64 -d | sh'"
        Invoke-ManagerSsh -SshArgs ($forwardArgs + @($remoteCommand))
        return
    }}
    Invoke-ManagerSsh -SshArgs $sshArgs
}}
function Read-Host {{
    param(
        [string]$Prompt,
        [switch]$AsSecureString
    )
    $answer = ''
    if ($Prompt.StartsWith('Select drive')) {{
        $drives = (Get-Variable -Name availableDrives -Scope 1 -ErrorAction SilentlyContinue).Value
        if ($drives) {{
            for ($i = 0; $i -lt $drives.Count; $i++) {{
                $name = [string]$drives[$i].Name
                if ($name.TrimEnd(':').ToUpperInvariant() -eq $script:__managerPreferredDrive) {{
                    $answer = [string]($i + 1)
                    break
                }}
            }}
        }}
        if ([string]::IsNullOrWhiteSpace($answer)) {{ $answer = '1' }}
    }} elseif ($Prompt.StartsWith('Do you want to remove it')) {{
        $answer = 'N'
    }} elseif ($Prompt.StartsWith('Turn off the VM now')) {{
        $answer = 'N'
    }} elseif ($Prompt.StartsWith('Incompatibilities detected')) {{
        $answer = 'N'
    }} elseif ($Prompt.StartsWith('Add external switch')) {{
        $answer = 'Y'
    }} elseif ($Prompt.StartsWith('Select adapter')) {{
        $nics = (Get-Variable -Name physicalNics -Scope 1 -ErrorAction SilentlyContinue).Value
        if ($nics) {{
            for ($i = 0; $i -lt $nics.Count; $i++) {{
                if ([string]$nics[$i].Name -eq $script:__managerAdapterName) {{
                    $answer = [string]($i + 1)
                    break
                }}
            }}
        }}
        if ([string]::IsNullOrWhiteSpace($answer)) {{ $answer = '1' }}
    }} elseif ($Prompt.StartsWith('Enter choice [1/2/3/4/5]')) {{
        $answer = $script:__managerMemoryChoice
    }} elseif ($Prompt.StartsWith('Enter memory in GB')) {{
        $answer = $script:__managerMemoryGb
    }} elseif ($Prompt.StartsWith('Would you like to change the default password')) {{
        $answer = 'N'
    }} elseif ($Prompt.StartsWith('Enter new password')) {{
        $answer = $script:__managerNewVmPassword
    }} elseif ($Prompt.StartsWith('Confirm new password')) {{
        $answer = $script:__managerNewVmPassword
    }} elseif ($Prompt.StartsWith('Choice [1/2]')) {{
        $script:__managerChoice12Count++
        if ($script:__managerStaticNetwork) {{ $answer = '2' }} else {{ $answer = '1' }}
    }} elseif ($Prompt.StartsWith('Enter the static IP for the VM')) {{
        $answer = $script:__managerStaticIp
    }} elseif ($Prompt.StartsWith('Enter the CIDR suffix')) {{
        $answer = '/24'
    }} elseif ($Prompt.StartsWith('Enter the gateway IP')) {{
        $answer = $script:__managerGateway
    }} elseif ($Prompt.StartsWith('Enter the DNS server')) {{
        $answer = $script:__managerDns
    }} elseif ($Prompt -eq 'Choice') {{
        if ([string]::IsNullOrWhiteSpace($script:__managerPlayerIp)) {{ $answer = '2' }} else {{ $answer = '3' }}
    }} elseif ($Prompt.StartsWith('Enter IP')) {{
        $answer = $script:__managerPlayerIp
    }} elseif ($Prompt.StartsWith('Steam download failed')) {{
        $answer = 'N'
    }} elseif ($Prompt.StartsWith('Enable experimental swap memory now')) {{
        if ($script:__managerEnableSwap) {{ $answer = 'Y' }} else {{ $answer = 'N' }}
    }} else {{
        Write-Host "[manager] Unrecognized vendor prompt: $Prompt" -ForegroundColor Yellow
    }}
    if ($AsSecureString) {{
        $secure = New-Object System.Security.SecureString
        foreach ($ch in $answer.ToCharArray()) {{ $secure.AppendChar($ch) }}
        $secure.MakeReadOnly()
        return $secure
    }}
    Write-Host "[manager] Answered vendor prompt: $Prompt" -ForegroundColor DarkGray
    return $answer
}}
try {{
    . (Join-Path $scriptDir 'initial-setup.ps1')
}} finally {{
    Remove-Item -LiteralPath $script:__managerAskPass -Force -ErrorAction SilentlyContinue
}}"#,
        script_dir = ps_single_quoted(&script_dir.to_string_lossy()),
        drive = ps_single_quoted(
            &request
                .preferred_drive_name()
                .unwrap_or_else(|| "C".to_string())
        ),
        adapter = ps_single_quoted(&request.adapter_name),
        memory_choice = ps_single_quoted(&memory_choice(request.memory_gb)),
        memory_gb = ps_single_quoted(&request.memory_gb.max(1).to_string()),
        static_network = if request.static_network {
            "$true"
        } else {
            "$false"
        },
        static_ip = ps_single_quoted(&request.static_ip),
        gateway = ps_single_quoted(&request.gateway),
        dns = ps_single_quoted(non_empty_or(&request.dns, "1.1.1.1").as_ref()),
        player_ip = ps_single_quoted(&request.player_ip),
        enable_swap = if request.enable_swap {
            "$true"
        } else {
            "$false"
        },
        remote_setup_input_b64 = ps_single_quoted(&base64_text(&remote_setup_answers(request))),
        remote_setup_script_b64 = ps_single_quoted(&base64_text(remote_setup_script())),
    )
}

fn base64_text(value: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = value.as_bytes();
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

fn remote_setup_script() -> &'static str {
    r#"set -eu
DUNE_USER_PATH=/home/dune/.dune
DOWNLOAD_PATH="$DUNE_USER_PATH/download"
INPUT_FILE=/tmp/dune-manager-setup.stdin
printf '%s' "$SETUP_INPUT_B64" | base64 -d > "$INPUT_FILE"
chmod 600 "$INPUT_FILE"
mkdir -p "$DOWNLOAD_PATH"
required_gb=30
available_gb=$(df -B1G -P / | awk '$NF == "/" {print $(NF-2)+0}')
if [ "$available_gb" -le "$required_gb" ]; then
  sudo growpart /dev/sda 2 >&2 || true
  sudo pvresize /dev/sda2 >&2 || true
  sudo lvextend -l +100%FREE /dev/mapper/vg0-lv_root >&2 || true
  sudo resize2fs /dev/mapper/vg0-lv_root >&2 || true
fi
available_gb=$(df -B1G -P / | awk '$NF == "/" {print $(NF-2)+0}')
if [ "$available_gb" -le "$required_gb" ]; then
  echo "Not enough guest disk space after resize: ${available_gb}GB available, need more than ${required_gb}GB"
  exit 1
fi
if [ ! -f "$DOWNLOAD_PATH/scripts/battlegroup.sh" ] || [ ! -f "$DOWNLOAD_PATH/scripts/setup.sh" ]; then
  for attempt in 1 2 3 4 5; do
    echo "Steam setup attempt $attempt"
    steamcmd +set_spew_level 1 1 +force_install_dir "$DOWNLOAD_PATH" +login anonymous +app_update 4754530 +logoff +quit || true
    if [ -f "$DOWNLOAD_PATH/scripts/battlegroup.sh" ] && [ -f "$DOWNLOAD_PATH/scripts/setup.sh" ]; then
      break
    fi
    sleep 5
  done
fi
if [ ! -f "$DOWNLOAD_PATH/scripts/battlegroup.sh" ] || [ ! -f "$DOWNLOAD_PATH/scripts/setup.sh" ]; then
  echo "Steam download did not produce vendor setup scripts"
  exit 1
fi
bash "$DOWNLOAD_PATH/scripts/setup/k3s.sh"
bash "$DOWNLOAD_PATH/scripts/setup/system.sh"
bash "$DOWNLOAD_PATH/scripts/setup/world.sh" < "$INPUT_FILE" || true
WORLD_FILE=$(ls -t "$DUNE_USER_PATH"/sh-*.yaml 2>/dev/null | grep -Ev '(-fls-secret|-rmq-secret)\.yaml$' | head -n1)
if [ -z "$WORLD_FILE" ]; then
  echo "Vendor world setup did not produce a battlegroup manifest"
  exit 1
fi
PLAYER_IP=$(awk 'NF { value=$0 } END { print value }' "$DUNE_USER_PATH/settings.conf")
if [ -z "$PLAYER_IP" ]; then
  echo "Player-facing IP was not written to settings.conf"
  exit 1
fi
WORLD_TMP="$WORLD_FILE.tmp"
awk -v player_ip="$PLAYER_IP" '
  next_is_host_ip {
    if ($0 ~ /^[[:space:]]*value:/) {
      sub(/value:.*/, "value: " player_ip)
      replaced++
    }
    next_is_host_ip=0
  }
  /name:[[:space:]]*HOST_DATACENTER_IP_ADDRESS/ { next_is_host_ip=1 }
  { print }
  END { if (replaced == 0) exit 42 }
' "$WORLD_FILE" > "$WORLD_TMP" || {
  rm -f "$WORLD_TMP"
  echo "No HOST_DATACENTER_IP_ADDRESS values were found in world manifest"
  exit 1
}
mv "$WORLD_TMP" "$WORLD_FILE"
WORLD_UNIQUE_NAME=$(basename "$WORLD_FILE" .yaml)
NAMESPACE="funcom-seabass-$WORLD_UNIQUE_NAME"
for attempt in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24; do
  sudo kubectl get namespace "$NAMESPACE" >/dev/null 2>&1 || sudo kubectl create namespace "$NAMESPACE" >/dev/null 2>&1 || true
  sudo kubectl apply -n "$NAMESPACE" -f "$DUNE_USER_PATH/$WORLD_UNIQUE_NAME-fls-secret.yaml" >/dev/null 2>&1 || true
  sudo kubectl apply -n "$NAMESPACE" -f "$DUNE_USER_PATH/$WORLD_UNIQUE_NAME-rmq-secret.yaml" >/dev/null 2>&1 || true
  if sudo kubectl get battlegroup "$WORLD_UNIQUE_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
    break
  fi
  sudo kubectl create -n "$NAMESPACE" -f "$WORLD_FILE" >/dev/null 2>&1 || true
  if sudo kubectl get battlegroup "$WORLD_UNIQUE_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
    break
  fi
  echo "Still working: Waiting for battlegroup resource to be accepted."
  sleep 5
done
if ! sudo kubectl get battlegroup "$WORLD_UNIQUE_NAME" -n "$NAMESPACE" >/dev/null 2>&1; then
  echo "Battlegroup resource was not created after waiting for admission webhooks"
  exit 1
fi
for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
  if "$DOWNLOAD_PATH/scripts/battlegroup.sh" update-from-downloads; then
    break
  fi
  if [ "$attempt" -eq 12 ]; then
    echo "Failed to patch battlegroup image revisions after retries"
    exit 1
  fi
  echo "Still working: Waiting before retrying battlegroup image patch."
  sleep 10
done
for attempt in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24; do
  FILEBROWSER_POD=$(sudo kubectl get pods -n "$NAMESPACE" -l role=igw-filebrowser --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null | head -n1 || true)
  if [ -n "$FILEBROWSER_POD" ]; then
    FILEBROWSER_PHASE=$(sudo kubectl get pod "$FILEBROWSER_POD" -n "$NAMESPACE" -o jsonpath='{.status.phase}' 2>/dev/null || true)
    if [ "$FILEBROWSER_PHASE" = "Running" ]; then
      break
    fi
  fi
  echo "Still working: Waiting for file browser pod."
  sleep 5
done
for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
  if "$DOWNLOAD_PATH/scripts/battlegroup.sh" apply-default-usersettings; then
    break
  fi
  if [ "$attempt" -eq 12 ]; then
    echo "Failed to apply default user settings after retries"
    exit 1
  fi
  echo "Still working: Waiting before retrying default user settings."
  sleep 10
done
"#
}

fn remote_setup_answers(request: &VendorHyperVSetupRequest) -> String {
    let region_choice = vendor_region_choice(&request.region);
    [
        non_empty_or(&request.world_name, "Arrakis"),
        region_choice.to_string(),
        request.self_host_token.trim().to_string(),
    ]
    .join("\n")
        + "\n"
}

#[derive(Debug, Clone)]
struct VendorPromptDriver {
    request: VendorHyperVSetupRequest,
    answered: Vec<&'static str>,
}

impl VendorPromptDriver {
    fn new(request: VendorHyperVSetupRequest) -> Self {
        Self {
            request,
            answered: Vec::new(),
        }
    }

    fn observe(&mut self, stdout: &str, stderr: &str) -> Vec<VendorPromptAnswer> {
        let mut answers = Vec::new();
        let combined = tail(&(stdout.to_string() + stderr), 8_000).to_ascii_lowercase();
        let stdout_tail = tail(stdout, 8_000);
        self.maybe_answer(
            &combined,
            "drive",
            "select drive (1-",
            drive_answer(&stdout_tail, self.request.preferred_drive_name().as_deref()),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "remove-existing-vm",
            "do you want to remove it and continue? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "turn-off-existing-vm",
            "turn off the vm now? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "continue-incompatible-vm",
            "incompatibilities detected. continue anyway? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "external-switch",
            "add external switch? [y/n]",
            "Y",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "adapter",
            "select adapter (1-",
            adapter_answer(&stdout_tail, &self.request.adapter_name),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "memory-choice",
            "enter choice [1/2/3/4/5]",
            memory_choice(self.request.memory_gb),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "manual-memory",
            "enter memory in gb",
            self.request.memory_gb.max(1).to_string(),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "change-password",
            "would you like to change the default password? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "network-mode",
            "choice [1/2]",
            if self.request.static_network {
                "2"
            } else {
                "1"
            },
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "static-mode",
            "static ip configuration:",
            if self.request.static_network {
                "2"
            } else {
                "1"
            },
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "static-ip",
            "enter the static ip for the vm",
            non_empty_or(&self.request.static_ip, ""),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "cidr",
            "enter the cidr suffix",
            "/24",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "gateway",
            "enter the gateway ip",
            non_empty_or(&self.request.gateway, ""),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "dns",
            "enter the dns server",
            non_empty_or(&self.request.dns, "1.1.1.1"),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "player-ip-choice",
            "select the ip that players will connect to",
            if self.request.player_ip.trim().is_empty() {
                "1"
            } else {
                "3"
            },
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "player-ip-manual",
            "enter ip",
            non_empty_or(&self.request.player_ip, ""),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "steam-retry",
            "steam download failed. retry? [y/n]",
            "N",
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "world-name",
            "world name",
            non_empty_or(&self.request.world_name, "Arrakis"),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "region",
            "region",
            vendor_region_choice(&self.request.region),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "self-host-token",
            "self-host",
            self.request.self_host_token.clone(),
            &mut answers,
        );
        self.maybe_answer(
            &combined,
            "experimental-swap",
            "enable experimental swap memory now? [y/n]",
            if self.request.enable_swap { "Y" } else { "N" },
            &mut answers,
        );
        answers
    }

    fn maybe_answer(
        &mut self,
        haystack: &str,
        id: &'static str,
        pattern: &str,
        answer: impl Into<String>,
        answers: &mut Vec<VendorPromptAnswer>,
    ) {
        if self.answered.contains(&id) || !haystack.contains(pattern) {
            return;
        }
        self.answered.push(id);
        answers.push(VendorPromptAnswer {
            prompt_id: id,
            answer: answer.into(),
        });
    }
}

fn vendor_region_choice(region: &str) -> &'static str {
    match region.trim().to_ascii_lowercase().as_str() {
        "asia" => "1",
        "north america" => "3",
        "oceania" => "4",
        "south america" => "5",
        _ => "2",
    }
}

#[derive(Debug, Clone, Copy)]
enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug)]
struct StreamChunk {
    kind: StreamKind,
    text: String,
}

fn spawn_reader<R>(mut reader: R, kind: StreamKind, tx: mpsc::Sender<StreamChunk>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    let text = String::from_utf8_lossy(&buffer[..size]).to_string();
                    if tx.send(StreamChunk { kind, text }).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn emit_lines(
    sink: &mut impl OperationSink,
    scope: &'static str,
    pending: &mut String,
    chunk: &str,
) {
    pending.push_str(chunk);
    while let Some(index) = pending.find('\n') {
        let line = pending[..index].trim_end_matches('\r').trim().to_string();
        pending.replace_range(..=index, "");
        if !line.is_empty() {
            emit(sink, scope, redact_log_line(&line));
        }
    }
}

fn flush_line(sink: &mut impl OperationSink, scope: &'static str, pending: &mut String) {
    let line = pending.trim();
    if !line.is_empty() {
        emit(sink, scope, redact_log_line(line));
    }
    pending.clear();
}

fn emit(sink: &mut impl OperationSink, step_id: &'static str, message: impl Into<String>) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.into(),
        domain: StepDomain::HyperV,
        action: StepAction::Configure,
        provider: ProviderKind::HyperV,
    });
}

fn redact_log_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if [
        "token",
        "secret",
        "password",
        "apikey",
        "auth",
        "private key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        "[redacted sensitive vendor output]".to_string()
    } else {
        line.to_string()
    }
}

fn sha256_file(path: &Path) -> CommandResult<String> {
    let bytes = fs::read(path)
        .map_err(|err| failure(format!("Failed to read {}: {err}", path.display())))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn drive_answer(stdout: &str, preferred: Option<&str>) -> String {
    preferred
        .and_then(|drive| drive_line_index(stdout, drive))
        .unwrap_or(1)
        .to_string()
}

fn drive_line_index(stdout: &str, drive: &str) -> Option<usize> {
    let drive = drive.trim().trim_end_matches(':').to_ascii_lowercase();
    if drive.is_empty() {
        return None;
    }
    stdout.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let (number, rest) = trimmed.split_once('.')?;
        let index = number.trim().parse::<usize>().ok()?;
        let rest = rest.trim_start().to_ascii_lowercase();
        (rest == drive || rest.starts_with(&format!("{drive} "))).then_some(index)
    })
}

fn adapter_answer(stdout: &str, adapter_name: &str) -> String {
    numbered_line_index(stdout, adapter_name.trim())
        .unwrap_or(1)
        .to_string()
}

fn numbered_line_index(stdout: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    stdout.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let (number, rest) = trimmed.split_once('.')?;
        let index = number.trim().parse::<usize>().ok()?;
        rest.to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase())
            .then_some(index)
    })
}

fn memory_choice(memory_gb: u64) -> String {
    match memory_gb {
        10 => "1".to_string(),
        20 => "2".to_string(),
        30 => "3".to_string(),
        40 => "4".to_string(),
        _ => "5".to_string(),
    }
}

fn non_empty_or(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn tail(value: &str, max_chars: usize) -> String {
    let len = value.chars().count();
    value.chars().skip(len.saturating_sub(max_chars)).collect()
}

fn last_non_empty_line(value: &str) -> Option<String> {
    value
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> VendorHyperVSetupRequest {
        VendorHyperVSetupRequest {
            vm_destination: PathBuf::from("F:\\DuneAwakeningServer"),
            adapter_name: "Ethernet".to_string(),
            memory_gb: 24,
            static_network: true,
            static_ip: "192.168.1.50".to_string(),
            gateway: "192.168.1.1".to_string(),
            dns: "1.1.1.1".to_string(),
            player_ip: "203.0.113.10".to_string(),
            world_name: "Arrakis".to_string(),
            region: "Europe".to_string(),
            self_host_token: "secret-token".to_string(),
            enable_swap: false,
        }
    }

    #[test]
    fn answers_vendor_hyperv_prompts_from_transcript() {
        let mut driver = VendorPromptDriver::new(request());
        let transcript = r#"
Multiple drives with enough free space (>100GB) detected.
  1. C (120 GB free)
  2. F (500 GB free)
Select drive (1-2)
Multiple network adapters detected.
  1. Wi-Fi (Wireless)
  2. Ethernet (Intel)
Select adapter (1-2)
Enter choice [1/2/3/4/5]
Enter memory in GB (e.g. 16)
Would you like to change the default password? [Y/N]
How do you want the VM to be assigned an IP?
Choice [1/2]
Static IP configuration:
Choice [1/2]
Enter the static IP for the VM [192.168.1.10]
Enter the CIDR suffix (e.g. /24) [/24]
Enter the gateway IP [192.168.1.1]
Enter the DNS server [1.1.1.1]
Select the IP that players will connect to
Choice
Enter IP
World name
Region
Enable experimental swap memory now? [Y/N]
"#;
        let answers = driver.observe(transcript, "");
        let rows = answers
            .iter()
            .map(|answer| (answer.prompt_id, answer.answer.as_str()))
            .collect::<Vec<_>>();
        assert!(rows.contains(&("drive", "2")));
        assert!(rows.contains(&("adapter", "2")));
        assert!(rows.contains(&("memory-choice", "5")));
        assert!(rows.contains(&("manual-memory", "24")));
        assert!(rows.contains(&("network-mode", "2")));
        assert!(rows.contains(&("static-mode", "2")));
        assert!(rows.contains(&("player-ip-choice", "3")));
        assert!(rows.contains(&("player-ip-manual", "203.0.113.10")));
        assert!(rows.contains(&("world-name", "Arrakis")));
        assert!(rows.contains(&("region", "2")));
        assert!(rows.contains(&("experimental-swap", "N")));
    }

    #[test]
    fn maps_release_region_menu_choices() {
        assert_eq!(vendor_region_choice("Asia"), "1");
        assert_eq!(vendor_region_choice("Europe"), "2");
        assert_eq!(vendor_region_choice("North America"), "3");
        assert_eq!(vendor_region_choice("Oceania"), "4");
        assert_eq!(vendor_region_choice("South America"), "5");
    }

    #[test]
    fn redacts_sensitive_vendor_output() {
        assert_eq!(
            redact_log_line("Self-Host Service Token: abc"),
            "[redacted sensitive vendor output]"
        );
        assert_eq!(redact_log_line("VM memory set"), "VM memory set");
    }
}
