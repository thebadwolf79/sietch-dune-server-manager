pub(super) const PREFLIGHT_SCRIPT: &str = r#"
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

pub(super) const K3S_INSTALL_SCRIPT: &str = r#"
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
