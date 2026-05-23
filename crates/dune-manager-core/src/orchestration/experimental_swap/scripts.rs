pub(super) const EXPERIMENTAL_SWAP_STATUS_SCRIPT: &str = r#"
set -euo pipefail
swap_file_exists=false
swap_active=false
swap_file_bytes=null
active_swap_bytes=null
fstab_configured=false
openrc_swap_enabled=false
kubelet_swap_configured=false

if [ -f /swapfile ]; then
  swap_file_exists=true
  swap_file_bytes=$(stat -c '%s' /swapfile 2>/dev/null || echo null)
fi
if awk '$1 == "/swapfile" { found = 1 } END { exit(found ? 0 : 1) }' /proc/swaps 2>/dev/null; then
  swap_active=true
  active_swap_bytes=$(awk '$1 == "/swapfile" { print $3 * 1024 }' /proc/swaps | head -n1)
fi
if grep -Eq '^[[:space:]]*/swapfile[[:space:]]' /etc/fstab 2>/dev/null; then
  fstab_configured=true
fi
if rc-update show 2>/dev/null | grep -Eq '^[[:space:]]*swap[[:space:]].*boot'; then
  openrc_swap_enabled=true
fi
if grep -REq '^[[:space:]]*failSwapOn:[[:space:]]*false[[:space:]]*$' /etc/rancher/k3s/kubelet.config /etc/rancher/k3s/kubelet-config.yaml 2>/dev/null \
  && { grep -Eq 'config=/etc/rancher/k3s/kubelet-config.yaml' /etc/rancher/k3s/config.yaml 2>/dev/null \
    || grep -REq 'config=/etc/rancher/k3s/kubelet.config|config=/etc/rancher/k3s/kubelet-config.yaml' /etc/rancher/k3s/config.yaml.d 2>/dev/null; }; then
  kubelet_swap_configured=true
fi
printf '{"swapFileExists":%s,"swapActive":%s,"swapFileBytes":%s,"activeSwapBytes":%s,"fstabConfigured":%s,"openrcSwapEnabled":%s,"kubeletSwapConfigured":%s,"battlegroupProfileApplied":null}\n' \
  "$swap_file_exists" "$swap_active" "$swap_file_bytes" "$active_swap_bytes" "$fstab_configured" "$openrc_swap_enabled" "$kubelet_swap_configured"
"#;

pub(super) fn enable_swap_script(swap_size_gib: u64, restart_k3s: bool) -> String {
    let restart = if restart_k3s { "true" } else { "false" };
    format!(
        r#"
set -euo pipefail
swap_size_gib={swap_size_gib}
restart_k3s={restart}
swap_bytes=$((swap_size_gib * 1024 * 1024 * 1024))

if [ ! -f /swapfile ] || [ "$(stat -c '%s' /swapfile 2>/dev/null || echo 0)" -lt "$swap_bytes" ]; then
  sudo swapoff /swapfile >/dev/null 2>&1 || true
  sudo rm -f /swapfile
  sudo dd if=/dev/zero of=/swapfile bs=1M count=$((swap_size_gib * 1024)) status=none
  sudo chmod 600 /swapfile
  sudo mkswap /swapfile >/dev/null
fi

if ! grep -Eq '^[[:space:]]*/swapfile[[:space:]]' /etc/fstab 2>/dev/null; then
  printf '/swapfile none swap sw 0 0\n' | sudo tee -a /etc/fstab >/dev/null
fi

sudo swapon /swapfile >/dev/null 2>&1 || true
sudo rc-update add swap boot >/dev/null 2>&1 || true

sudo mkdir -p /etc/rancher/k3s
if [ ! -f /etc/rancher/k3s/kubelet-config.yaml ] || ! grep -Eq '^[[:space:]]*failSwapOn:[[:space:]]*false[[:space:]]*$' /etc/rancher/k3s/kubelet-config.yaml; then
  printf 'failSwapOn: false\nmemorySwap:\n  swapBehavior: LimitedSwap\n' | sudo tee /etc/rancher/k3s/kubelet-config.yaml >/dev/null
fi
if ! grep -Eq 'config=/etc/rancher/k3s/kubelet-config.yaml' /etc/rancher/k3s/config.yaml /etc/rancher/k3s/config.yaml.d/*.yaml 2>/dev/null; then
  sudo mkdir -p /etc/rancher/k3s/config.yaml.d
  printf 'kubelet-arg+:\n- config=/etc/rancher/k3s/kubelet-config.yaml\n' | sudo tee /etc/rancher/k3s/config.yaml.d/99-dune-manager-swap.yaml >/dev/null
fi

if [ "$restart_k3s" = "true" ]; then
  sudo rc-service k3s stop >/dev/null 2>&1 || true
  if [ -x /usr/local/bin/k3s-killall.sh ]; then
    sudo /usr/local/bin/k3s-killall.sh >/dev/null 2>&1 || true
  fi
  sudo rc-service k3s start >/dev/null
  elapsed=0
  until sudo kubectl get nodes >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 180 ]; then echo "k3s API did not return after enabling swap" >&2; exit 1; fi
  done
  sudo kubectl wait --for=condition=Ready node --all --timeout=180s >/dev/null || true
fi
"#
    )
}
