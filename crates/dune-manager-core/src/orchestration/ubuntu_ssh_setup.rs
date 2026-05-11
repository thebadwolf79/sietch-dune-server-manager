use serde::{Deserialize, Serialize};

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        parse_single_json_document, OperationSink, OrchestrationEvent, ProviderKind,
        RemoteCommandRunner, StepAction, StepDomain,
    },
};

const DEFAULT_SERVER_ROOT: &str = "/home/dune/.dune";
const DEFAULT_LINUX_USER: &str = "dune";
const DEFAULT_STEAMCMD_URL: &str =
    "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz";
const SERVER_APP_ID: &str = "3104830";

/// Read-only inventory of a remote Ubuntu host before setup begins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbuntuSshPreflight {
    /// Kernel host name.
    pub hostname: String,
    /// Operating system pretty name from `/etc/os-release`.
    pub os_pretty_name: String,
    /// Distribution identifier from `/etc/os-release`.
    pub os_id: String,
    /// Distribution version identifier.
    pub version_id: String,
    /// CPU architecture reported by Python's platform module.
    pub architecture: String,
    /// Linux kernel release.
    pub kernel_release: String,
    /// Connected SSH username.
    pub user: String,
    /// Effective user id for the SSH session.
    pub uid: u32,
    /// Whether the session can run privileged commands without a password.
    pub passwordless_sudo: bool,
    /// Whether `systemctl` is available.
    pub systemd_available: bool,
    /// Logical CPU count.
    pub logical_processor_count: u32,
    /// Total physical memory in bytes.
    pub total_memory_bytes: u64,
    /// Available physical memory in bytes.
    pub available_memory_bytes: u64,
    /// Configured swap in bytes.
    pub swap_total_bytes: u64,
    /// Root filesystem size in bytes.
    pub root_disk_total_bytes: u64,
    /// Root filesystem free bytes.
    pub root_disk_available_bytes: u64,
    /// Public egress IP detected from the host, if reachable.
    pub public_ip: Option<String>,
    /// Non-loopback IPv4 addresses found on the host.
    pub ipv4_addresses: Vec<String>,
    /// Whether the app-owned SteamCMD path already exists.
    pub steamcmd_installed: bool,
    /// Whether k3s is already installed.
    pub k3s_installed: bool,
    /// Whether kubectl is reachable through k3s.
    pub kubectl_available: bool,
}

/// Request for preparing a fresh Ubuntu host for Dune server installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UbuntuSshPrepareRequest {
    /// Remote user that owns the server payload and writable config.
    pub linux_user: String,
    /// Root directory for app-managed server state.
    pub server_root: String,
    /// URL for the SteamCMD Linux tarball.
    pub steamcmd_url: String,
}

impl Default for UbuntuSshPrepareRequest {
    fn default() -> Self {
        Self {
            linux_user: DEFAULT_LINUX_USER.to_string(),
            server_root: DEFAULT_SERVER_ROOT.to_string(),
            steamcmd_url: DEFAULT_STEAMCMD_URL.to_string(),
        }
    }
}

impl UbuntuSshPrepareRequest {
    /// Validates names and absolute paths before sending shell to the host.
    pub fn validate(&self) -> CommandResult<()> {
        validate_linux_user(&self.linux_user)?;
        validate_absolute_path(&self.server_root, "server root")?;
        if self.steamcmd_url.trim().is_empty()
            || self.steamcmd_url.contains('\n')
            || self.steamcmd_url.contains('\r')
        {
            return Err(failure("SteamCMD source URL is required"));
        }
        Ok(())
    }
}

/// Remote paths prepared for subsequent Ubuntu setup phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbuntuSshPreparedHost {
    /// Remote user that owns the server files.
    pub linux_user: String,
    /// Server root directory.
    pub server_root: String,
    /// Server payload download directory.
    pub download_path: String,
    /// SteamCMD shell script path.
    pub steamcmd_path: String,
}

/// Result of downloading the Steam server package on Ubuntu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UbuntuServerPayload {
    /// Server payload directory.
    pub download_path: String,
    /// Whether the expected setup script is present.
    pub setup_script_present: bool,
    /// Whether the expected battlegroup script is present.
    pub battlegroup_script_present: bool,
}

/// SSH-backed Ubuntu setup phases for remote or bare-metal servers.
#[derive(Debug, Clone)]
pub struct UbuntuSshSetup<R> {
    runner: R,
}

impl<R> UbuntuSshSetup<R>
where
    R: RemoteCommandRunner,
{
    /// Creates an Ubuntu SSH setup provider from a remote command runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Performs read-only OS, resource, and tool detection.
    pub fn preflight(&self) -> CommandResult<UbuntuSshPreflight> {
        let output = self.runner.run_script(PREFLIGHT_SCRIPT)?;
        let result: UbuntuSshPreflight = parse_single_json_document(&output, "ubuntu preflight")?;
        if result.os_id != "ubuntu" {
            return Err(failure(format!(
                "Remote host is {}, expected Ubuntu",
                result.os_pretty_name
            )));
        }
        Ok(result)
    }

    /// Installs base packages, creates the service user, and installs SteamCMD.
    pub fn prepare_host(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<UbuntuSshPreparedHost> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.prepare.packages",
            "Installing Ubuntu prerequisites.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        let output = self
            .runner
            .run_script(&prepare_host_script(request, false))?;
        parse_single_json_document(&output, "ubuntu prepare host")
    }

    /// Installs or starts k3s using systemd.
    pub fn install_k3s(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.k3s.install",
            "Installing or validating k3s.",
            StepDomain::Guest,
            StepAction::Configure,
        );
        self.runner.run_script(K3S_INSTALL_SCRIPT)?;
        Ok(())
    }

    /// Bootstraps cert-manager and the initial Funcom operator deployments on fresh Ubuntu.
    pub fn bootstrap_kubernetes(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.k3s.bootstrap",
            "Bootstrapping Kubernetes images and operators.",
            StepDomain::Kubernetes,
            StepAction::Configure,
        );
        self.runner
            .run_script(&bootstrap_kubernetes_script(request))?;
        Ok(())
    }

    /// Downloads the Dune server package through SteamCMD on the Ubuntu host.
    pub fn install_server_payload(
        &self,
        request: &UbuntuSshPrepareRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<UbuntuServerPayload> {
        request.validate()?;
        emit(
            sink,
            "ubuntu.steam.download",
            "Installing or validating the Dune server payload.",
            StepDomain::Steam,
            StepAction::Download,
        );
        let output = self.runner.run_script(&install_payload_script(request))?;
        parse_single_json_document(&output, "ubuntu server payload")
    }
}

fn emit(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: &str,
    domain: StepDomain,
    action: StepAction,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.to_string(),
        domain,
        action,
        provider: ProviderKind::Ssh,
    });
}

fn prepare_host_script(request: &UbuntuSshPrepareRequest, force_steamcmd: bool) -> String {
    format!(
        r#"
set -eu
export DEBIAN_FRONTEND=noninteractive
LINUX_USER={linux_user}
SERVER_ROOT={server_root}
STEAMCMD_URL={steamcmd_url}
FORCE_STEAMCMD={force_steamcmd}

if [ "$(id -u)" -ne 0 ] && ! sudo -n true >/dev/null 2>&1; then
  echo "This setup phase requires root or passwordless sudo." >&2
  exit 1
fi
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

$SUDO apt-get update -y >/dev/null
$SUDO apt-get install -y \
  ca-certificates curl tar gzip unzip openssl util-linux iproute2 procps lsb-release \
  sudo lib32gcc-s1 lib32stdc++6 >/dev/null

if ! id "$LINUX_USER" >/dev/null 2>&1; then
  $SUDO useradd -m -s /bin/bash "$LINUX_USER"
fi

USER_HOME=$(getent passwd "$LINUX_USER" | cut -d: -f6)
STEAM_HOME="$USER_HOME/Steam"
DOWNLOAD_PATH="$SERVER_ROOT/download"
$SUDO mkdir -p "$SERVER_ROOT" "$DOWNLOAD_PATH" "$STEAM_HOME" "$USER_HOME/.steam"
$SUDO chown -R "$LINUX_USER:$LINUX_USER" "$SERVER_ROOT" "$STEAM_HOME" "$USER_HOME/.steam"

if [ "$FORCE_STEAMCMD" = "1" ] || [ ! -x "$STEAM_HOME/steamcmd.sh" ]; then
  tmp="$(mktemp -t dune-steamcmd.XXXXXX.tar.gz)"
  curl -fsSL "$STEAMCMD_URL" -o "$tmp"
  chmod 0644 "$tmp"
  sudo -u "$LINUX_USER" tar -xzf "$tmp" -C "$STEAM_HOME"
  rm -f "$tmp"
fi

sudo -u "$LINUX_USER" mkdir -p "$USER_HOME/.steam"
sudo -u "$LINUX_USER" ln -sfn "$STEAM_HOME" "$USER_HOME/.steam/root"
sudo -u "$LINUX_USER" ln -sfn "$STEAM_HOME" "$USER_HOME/.steam/steam"

printf '{{"linuxUser":%s,"serverRoot":%s,"downloadPath":%s,"steamcmdPath":%s}}\n' \
  "$(json_quote "$LINUX_USER")" \
  "$(json_quote "$SERVER_ROOT")" \
  "$(json_quote "$DOWNLOAD_PATH")" \
  "$(json_quote "$STEAM_HOME/steamcmd.sh")"
"#,
        linux_user = sh_single_quoted(&request.linux_user),
        server_root = sh_single_quoted(&request.server_root),
        steamcmd_url = sh_single_quoted(&request.steamcmd_url),
        force_steamcmd = if force_steamcmd { "1" } else { "0" },
    )
    .replacen(
        "set -eu\n",
        "set -eu\njson_quote() { python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' \"$1\"; }\n",
        1,
    )
}

fn install_payload_script(request: &UbuntuSshPrepareRequest) -> String {
    format!(
        r#"
set -eu
LINUX_USER={linux_user}
SERVER_ROOT={server_root}
DOWNLOAD_PATH="$SERVER_ROOT/download"
USER_HOME=$(getent passwd "$LINUX_USER" | cut -d: -f6)
STEAMCMD="$USER_HOME/Steam/steamcmd.sh"
if [ ! -x "$STEAMCMD" ]; then
  echo "SteamCMD is not installed at $STEAMCMD." >&2
  exit 1
fi
mkdir -p "$DOWNLOAD_PATH"
chown -R "$LINUX_USER:$LINUX_USER" "$SERVER_ROOT"

steamcmd_update_once() {{
  sudo -u "$LINUX_USER" env HOME="$USER_HOME" "$STEAMCMD" \
    +@ShutdownOnFailedCommand 1 \
    +@NoPromptForPassword 1 \
    +set_spew_level 1 1 \
    +force_install_dir "$DOWNLOAD_PATH" \
    +login anonymous \
    +app_update {app_id} validate \
    +logoff \
    +quit < /dev/null >/tmp/dune-steamcmd-stdout.log 2>/tmp/dune-steamcmd-stderr.log
}}

attempt=1
max_attempts=5
while [ "$attempt" -le "$max_attempts" ]; do
  if steamcmd_update_once; then
    break
  fi
  status=$?
  if [ "$attempt" -ge "$max_attempts" ]; then
    cat /tmp/dune-steamcmd-stdout.log >&2 || true
    cat /tmp/dune-steamcmd-stderr.log >&2 || true
    echo "SteamCMD payload download failed after $max_attempts attempts, last exit code $status." >&2
    exit "$status"
  fi
  sleep_seconds=$((attempt * 20))
  sleep "$sleep_seconds"
  attempt=$((attempt + 1))
done

SETUP_PRESENT=false
BG_PRESENT=false
[ -f "$DOWNLOAD_PATH/scripts/setup.sh" ] && SETUP_PRESENT=true
[ -f "$DOWNLOAD_PATH/scripts/battlegroup.sh" ] && BG_PRESENT=true
printf '{{"downloadPath":%s,"setupScriptPresent":%s,"battlegroupScriptPresent":%s}}\n' \
  "$(json_quote "$DOWNLOAD_PATH")" "$SETUP_PRESENT" "$BG_PRESENT"
"#,
        linux_user = sh_single_quoted(&request.linux_user),
        server_root = sh_single_quoted(&request.server_root),
        app_id = SERVER_APP_ID,
    )
    .replacen(
        "set -eu\n",
        "set -eu\njson_quote() { python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' \"$1\"; }\n",
        1,
    )
}

fn bootstrap_kubernetes_script(request: &UbuntuSshPrepareRequest) -> String {
    format!(
        r#"
set -eu
SERVER_ROOT={server_root}
DOWNLOAD_PATH="$SERVER_ROOT/download"
if [ ! -d "$DOWNLOAD_PATH/images/operators/crds" ]; then
  echo "Dune server payload is missing operator CRDs at $DOWNLOAD_PATH/images/operators/crds." >&2
  exit 1
fi

load_image_from_file() {{
  file_name="$1"
  if [ ! -f "$DOWNLOAD_PATH/$file_name" ]; then
    echo "Image file $DOWNLOAD_PATH/$file_name does not exist" >&2
    exit 1
  fi
  attempt=1
  while [ "$attempt" -le 3 ]; do
    if sudo k3s ctr images import "$DOWNLOAD_PATH/$file_name" >/dev/null; then
      return 0
    fi
    sleep 5
    attempt=$((attempt + 1))
  done
  echo "Failed to import $file_name after 3 attempts" >&2
  exit 1
}}

kubectl_retry() {{
  attempt=1
  while [ "$attempt" -le 5 ]; do
    if out=$(sudo kubectl "$@" 2>&1); then
      [ -n "$out" ] && printf '%s\n' "$out" >&2
      return 0
    fi
    if printf '%s' "$out" | grep -qiE 'connection refused|unable to connect to the server|i/o timeout|tls handshake|no route to host|EOF'; then
      sleep 5
      attempt=$((attempt + 1))
      continue
    fi
    printf '%s\n' "$out" >&2
    return 1
  done
  echo "kubectl $* still failing after retries" >&2
  return 1
}}

scale_deployment() {{
  ns="$1"
  name="$2"
  replicas="$3"
  elapsed=0
  until sudo kubectl get -n "$ns" deployment "$name" >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 180 ]; then
      echo "deployment $ns/$name did not appear within 180s" >&2
      return 1
    fi
  done
  kubectl_retry scale -n "$ns" "deployment/$name" "--replicas=$replicas"
}}

load_image_from_file "images/prerequisites/coredns-coredns.tar"
load_image_from_file "images/prerequisites/local-path-provisioner.tar"
load_image_from_file "images/prerequisites/metrics-server.tar"
load_image_from_file "images/prerequisites/cert-manager-webhook.tar"
load_image_from_file "images/prerequisites/cert-manager-controller.tar"
load_image_from_file "images/prerequisites/cert-manager-cainjector.tar"
load_image_from_file "images/prerequisites/igw-postgres.tar"

if ! sudo kubectl get deployment cert-manager -n cert-manager >/dev/null 2>&1; then
  kubectl_retry apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.8.0/cert-manager.yaml
fi
scale_deployment kube-system coredns 1
scale_deployment kube-system local-path-provisioner 1
scale_deployment kube-system metrics-server 1
scale_deployment cert-manager cert-manager 1
scale_deployment cert-manager cert-manager-cainjector 1
scale_deployment cert-manager cert-manager-webhook 1

sudo kubectl create namespace funcom-operators --dry-run=client -o yaml | sudo kubectl apply -f -
node_name=$(sudo kubectl get nodes -o jsonpath='{{.items[0].metadata.name}}')
sudo kubectl label node "$node_name" node.funcom.com/workload=infrastructure --overwrite >/dev/null

load_image_from_file "images/operators/battlegroup-operator.tar"
load_image_from_file "images/operators/database-operator.tar"
load_image_from_file "images/operators/server-operator.tar"
load_image_from_file "images/operators/utilities-operator.tar"

kubectl_retry apply --server-side -f "$DOWNLOAD_PATH/images/operators/crds/"

operator_version=$(cat "$DOWNLOAD_PATH/images/operators/version.txt")
manifest="/tmp/dune-operator-deployments.yaml"
cat > "$manifest" <<'YAML'
{operator_deployments}
YAML
sed -i "s/__OPERATOR_VERSION__/$operator_version/g" "$manifest"
sudo kubectl apply -f "$manifest"
rm -f "$manifest"

for op in battlegroupoperator databaseoperator serveroperator utilitiesoperator; do
  secret="${{op}}-webhook-server-cert"
  if ! sudo kubectl get secret "$secret" -n funcom-operators >/dev/null 2>&1; then
    sudo openssl req -x509 -nodes -newkey rsa:2048 -days 3650 \
      -keyout /tmp/dune-webhook.key -out /tmp/dune-webhook.crt \
      -subj "/CN=${{op}}-webhook.funcom-operators.svc" >/dev/null 2>&1
    sudo kubectl create secret tls "$secret" -n funcom-operators \
      --cert=/tmp/dune-webhook.crt --key=/tmp/dune-webhook.key >/dev/null
    sudo rm -f /tmp/dune-webhook.key /tmp/dune-webhook.crt
  fi
  if ! sudo kubectl get clusterrolebinding "${{op}}-manager-rolebinding" >/dev/null 2>&1; then
    sudo kubectl create clusterrolebinding "${{op}}-manager-rolebinding" \
      --clusterrole="${{op}}-manager-role" \
      --serviceaccount="funcom-operators:${{op}}-controller-manager" >/dev/null
  fi
  if ! sudo kubectl get role "${{op}}-leader-election-role" -n funcom-operators >/dev/null 2>&1; then
    sudo kubectl create role "${{op}}-leader-election-role" \
      -n funcom-operators \
      --verb=get,list,watch,create,update,patch,delete \
      --resource=leases.coordination.k8s.io \
      --resource=events >/dev/null
    sudo kubectl create rolebinding "${{op}}-leader-election-rolebinding" \
      -n funcom-operators \
      --role="${{op}}-leader-election-role" \
      --serviceaccount="funcom-operators:${{op}}-controller-manager" >/dev/null
  fi
done

scale_deployment funcom-operators battlegroupoperator-controller-manager 1
scale_deployment funcom-operators databaseoperator-controller-manager 1
scale_deployment funcom-operators serveroperator-controller-manager 1
scale_deployment funcom-operators utilitiesoperator-controller-manager 1

sudo kubectl wait --for=condition=Available -n funcom-operators deployment/battlegroupoperator-controller-manager --timeout=180s
sudo kubectl wait --for=condition=Available -n funcom-operators deployment/databaseoperator-controller-manager --timeout=180s
sudo kubectl wait --for=condition=Available -n funcom-operators deployment/serveroperator-controller-manager --timeout=180s
sudo kubectl wait --for=condition=Available -n funcom-operators deployment/utilitiesoperator-controller-manager --timeout=180s
"#,
        server_root = sh_single_quoted(&request.server_root),
        operator_deployments = OPERATOR_DEPLOYMENTS_YAML,
    )
}

const PREFLIGHT_SCRIPT: &str = r#"
set -eu
python3 - <<'PY'
import json
import os
import platform
import shutil
import socket
import subprocess

def os_release():
    values = {}
    try:
        with open("/etc/os-release", "r", encoding="utf-8") as handle:
            for line in handle:
                if "=" not in line:
                    continue
                key, value = line.rstrip("\n").split("=", 1)
                values[key] = value.strip('"')
    except FileNotFoundError:
        pass
    return values

def command_ok(*args):
    return subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0

def meminfo_value(name):
    try:
        with open("/proc/meminfo", "r", encoding="utf-8") as handle:
            for line in handle:
                if line.startswith(name + ":"):
                    return int(line.split()[1]) * 1024
    except FileNotFoundError:
        pass
    return 0

def public_ip():
    for url in ("https://api.ipify.org", "https://ifconfig.me/ip"):
        try:
            result = subprocess.run(
                ["curl", "-fsSL", "--max-time", "5", url],
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
            )
            value = result.stdout.strip()
            if result.returncode == 0 and value:
                return value
        except OSError:
            return None
    return None

release = os_release()
stat = os.statvfs("/")
ip_result = subprocess.run(
    ["sh", "-c", "ip -o -4 addr show scope global | awk '{print $4}'"],
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
    text=True,
)
print(json.dumps({
    "hostname": socket.gethostname(),
    "osPrettyName": release.get("PRETTY_NAME", ""),
    "osId": release.get("ID", ""),
    "versionId": release.get("VERSION_ID", ""),
    "architecture": platform.machine(),
    "kernelRelease": platform.release(),
    "user": os.environ.get("USER") or subprocess.run(["id", "-un"], stdout=subprocess.PIPE, text=True).stdout.strip(),
    "uid": os.geteuid(),
    "passwordlessSudo": os.geteuid() == 0 or command_ok("sudo", "-n", "true"),
    "systemdAvailable": shutil.which("systemctl") is not None,
    "logicalProcessorCount": os.cpu_count() or 0,
    "totalMemoryBytes": meminfo_value("MemTotal"),
    "availableMemoryBytes": meminfo_value("MemAvailable"),
    "swapTotalBytes": meminfo_value("SwapTotal"),
    "rootDiskTotalBytes": stat.f_blocks * stat.f_frsize,
    "rootDiskAvailableBytes": stat.f_bavail * stat.f_frsize,
    "publicIp": public_ip(),
    "ipv4Addresses": [line.strip() for line in ip_result.stdout.splitlines() if line.strip()],
    "steamcmdInstalled": os.path.exists("/home/dune/Steam/steamcmd.sh"),
    "k3sInstalled": shutil.which("k3s") is not None,
    "kubectlAvailable": command_ok("sh", "-c", "command -v kubectl >/dev/null || command -v k3s >/dev/null"),
}))
PY
"#;

const K3S_INSTALL_SCRIPT: &str = r#"
set -eu
if [ "$(id -u)" -ne 0 ] && ! sudo -n true >/dev/null 2>&1; then
  echo "This setup phase requires root or passwordless sudo." >&2
  exit 1
fi
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

if ! command -v k3s >/dev/null 2>&1; then
  installer="$(mktemp -t dune-k3s-install.XXXXXX.sh)"
  curl -sfL https://get.k3s.io -o "$installer"
  chmod 0755 "$installer"
  if [ "$(id -u)" -eq 0 ]; then
    INSTALL_K3S_EXEC='server --disable=traefik --write-kubeconfig-mode=644' sh "$installer"
  else
    sudo INSTALL_K3S_EXEC='server --disable=traefik --write-kubeconfig-mode=644' sh "$installer"
  fi
  rm -f "$installer"
fi

$SUDO systemctl enable k3s >/dev/null
$SUDO systemctl start k3s
elapsed=0
while [ ! -S /run/k3s/containerd/containerd.sock ]; do
  sleep 2
  elapsed=$((elapsed + 2))
  if [ "$elapsed" -ge 120 ]; then echo "k3s containerd did not become ready in 120s" >&2; exit 1; fi
done
elapsed=0
until $SUDO kubectl get nodes >/dev/null 2>&1; do
  sleep 2
  elapsed=$((elapsed + 2))
  if [ "$elapsed" -ge 120 ]; then echo "k3s API did not become ready in 120s" >&2; exit 1; fi
done
$SUDO kubectl wait --for=condition=Ready node --all --timeout=180s >/dev/null || true
"#;

const OPERATOR_DEPLOYMENTS_YAML: &str = r#"apiVersion: v1
kind: ServiceAccount
metadata:
  name: battlegroupoperator-controller-manager
  namespace: funcom-operators
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: databaseoperator-controller-manager
  namespace: funcom-operators
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: serveroperator-controller-manager
  namespace: funcom-operators
---
apiVersion: v1
kind: ServiceAccount
metadata:
  name: utilitiesoperator-controller-manager
  namespace: funcom-operators
---
apiVersion: apps/v1
kind: Deployment
metadata:
  labels:
    control-plane: battlegroup-controller-manager
  name: battlegroupoperator-controller-manager
  namespace: funcom-operators
spec:
  replicas: 1
  selector:
    matchLabels:
      control-plane: battlegroup-controller-manager
  template:
    metadata:
      labels:
        control-plane: battlegroup-controller-manager
    spec:
      serviceAccountName: battlegroupoperator-controller-manager
      containers:
      - name: manager
        command: ["/app/operator"]
        args:
        - --leader-elect
        - --database-default-port=15432
        - --filebrowser-default-port=18888
        - --pghero-default-port=21111
        - --zap-devel=false
        - --zap-log-level=debug
        - --zap-time-encoding=iso8601
        - --bg-max-concurrent=2
        - --dr-max-concurrent=2
        - --sr-max-concurrent=2
        - --cfo-taints-ignore=node.kubernetes.io/unschedulable,node.funcom.com/new
        image: registry.funcom.com/funcom/self-hosting/igw-k8s-battlegroup-operator:__OPERATOR_VERSION__
        imagePullPolicy: IfNotPresent
        ports:
        - containerPort: 9443
          name: webhook-server
        volumeMounts:
        - mountPath: /tmp/k8s-webhook-server/serving-certs
          name: cert
          readOnly: true
      volumes:
      - name: cert
        secret:
          secretName: battlegroupoperator-webhook-server-cert
---
apiVersion: apps/v1
kind: Deployment
metadata:
  labels:
    control-plane: database-controller-manager
  name: databaseoperator-controller-manager
  namespace: funcom-operators
spec:
  replicas: 1
  selector:
    matchLabels:
      control-plane: database-controller-manager
  template:
    metadata:
      labels:
        control-plane: database-controller-manager
    spec:
      serviceAccountName: databaseoperator-controller-manager
      containers:
      - name: manager
        command: ["/app/operator"]
        args:
        - --leader-elect
        - --zap-devel=false
        - --zap-log-level=debug
        - --zap-time-encoding=iso8601
        - --db-max-concurrent=2
        - --dbdepl-max-concurrent=2
        - --dbutil-max-concurrent=2
        - --dbop-max-concurrent=2
        - --dbb-max-concurrent=2
        - --dbbs-max-concurrent=2
        - --dbr-max-concurrent=2
        - --dbm-max-concurrent=2
        - --dbutil-supports-prometheus=false
        image: registry.funcom.com/funcom/self-hosting/igw-k8s-database-operator:__OPERATOR_VERSION__
        imagePullPolicy: IfNotPresent
        ports:
        - containerPort: 9443
          name: webhook-server
        volumeMounts:
        - mountPath: /tmp/k8s-webhook-server/serving-certs
          name: cert
          readOnly: true
      volumes:
      - name: cert
        secret:
          secretName: databaseoperator-webhook-server-cert
---
apiVersion: apps/v1
kind: Deployment
metadata:
  labels:
    control-plane: server-controller-manager
  name: serveroperator-controller-manager
  namespace: funcom-operators
spec:
  replicas: 1
  selector:
    matchLabels:
      control-plane: server-controller-manager
  template:
    metadata:
      labels:
        control-plane: server-controller-manager
    spec:
      serviceAccountName: serveroperator-controller-manager
      containers:
      - name: manager
        command: ["/app/operator"]
        args:
        - --leader-elect
        - --zap-devel=false
        - --zap-log-level=debug
        - --zap-time-encoding=iso8601
        - --sg-max-concurrent=2
        - --ss-max-concurrent=2
        image: registry.funcom.com/funcom/self-hosting/igw-k8s-server-operator:__OPERATOR_VERSION__
        imagePullPolicy: IfNotPresent
        ports:
        - containerPort: 9443
          name: webhook-server
        volumeMounts:
        - mountPath: /tmp/k8s-webhook-server/serving-certs
          name: cert
          readOnly: true
      volumes:
      - name: cert
        secret:
          secretName: serveroperator-webhook-server-cert
---
apiVersion: apps/v1
kind: Deployment
metadata:
  labels:
    control-plane: utilities-controller-manager
  name: utilitiesoperator-controller-manager
  namespace: funcom-operators
spec:
  replicas: 1
  selector:
    matchLabels:
      control-plane: utilities-controller-manager
  template:
    metadata:
      labels:
        control-plane: utilities-controller-manager
    spec:
      serviceAccountName: utilitiesoperator-controller-manager
      containers:
      - name: manager
        command: ["/app/operator"]
        args:
        - --leader-elect
        - --zap-devel=false
        - --zap-log-level=debug
        - --zap-time-encoding=iso8601
        - --sgw-max-concurrent=2
        - --bgd-max-concurrent=2
        - --fb-max-concurrent=1
        - --mq-max-concurrent=2
        - --tr-max-concurrent=2
        image: registry.funcom.com/funcom/self-hosting/igw-k8s-utilities-operator:__OPERATOR_VERSION__
        imagePullPolicy: IfNotPresent
        ports:
        - containerPort: 9443
          name: webhook-server
        volumeMounts:
        - mountPath: /tmp/k8s-webhook-server/serving-certs
          name: cert
          readOnly: true
      volumes:
      - name: cert
        secret:
          secretName: utilitiesoperator-webhook-server-cert
"#;

fn validate_linux_user(value: &str) -> CommandResult<()> {
    if value.is_empty()
        || value.len() > 32
        || !value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
        || value.starts_with('-')
    {
        return Err(failure(
            "Linux user must contain only lowercase letters, digits, hyphen, or underscore",
        ));
    }
    Ok(())
}

fn validate_absolute_path(value: &str, label: &str) -> CommandResult<()> {
    if !value.starts_with('/') || value == "/" || value.contains('\n') || value.contains('\r') {
        return Err(failure(format!("{label} must be an absolute Linux path")));
    }
    Ok(())
}

fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_request_uses_app_owned_guest_paths() {
        let request = UbuntuSshPrepareRequest::default();
        assert_eq!(request.linux_user, "dune");
        assert_eq!(request.server_root, "/home/dune/.dune");
        request.validate().unwrap();
    }

    #[test]
    fn rejects_non_absolute_server_root() {
        let request = UbuntuSshPrepareRequest {
            server_root: "relative".to_string(),
            ..UbuntuSshPrepareRequest::default()
        };
        assert!(request.validate().is_err());
    }
}
