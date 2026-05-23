pub(super) fn ubuntu_swap_script(swap_size_gib: u64) -> String {
    format!(
        r#"
set -eu
swap_size_gib={swap_size_gib}
swap_bytes=$((swap_size_gib * 1024 * 1024 * 1024))

if [ "$(id -u)" -ne 0 ] && ! sudo -n true >/dev/null 2>&1; then
  echo "This setup phase requires root or passwordless sudo." >&2
  exit 1
fi
SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

if [ ! -f /swapfile ] || [ "$(stat -c '%s' /swapfile 2>/dev/null || echo 0)" -lt "$swap_bytes" ]; then
  $SUDO swapoff /swapfile >/dev/null 2>&1 || true
  $SUDO rm -f /swapfile
  if command -v fallocate >/dev/null 2>&1; then
    $SUDO fallocate -l "$swap_bytes" /swapfile
  else
    $SUDO dd if=/dev/zero of=/swapfile bs=1M count=$((swap_size_gib * 1024)) status=none
  fi
  $SUDO chmod 600 /swapfile
  $SUDO mkswap /swapfile >/dev/null
fi

if ! grep -Eq '^[[:space:]]*/swapfile[[:space:]]' /etc/fstab 2>/dev/null; then
  printf '/swapfile none swap sw 0 0\n' | $SUDO tee -a /etc/fstab >/dev/null
fi

$SUDO swapon /swapfile >/dev/null 2>&1 || true

$SUDO mkdir -p /etc/rancher/k3s
if [ ! -f /etc/rancher/k3s/kubelet-config.yaml ] \
  || ! grep -Eq '^[[:space:]]*kind:[[:space:]]*KubeletConfiguration[[:space:]]*$' /etc/rancher/k3s/kubelet-config.yaml \
  || ! grep -Eq '^[[:space:]]*failSwapOn:[[:space:]]*false[[:space:]]*$' /etc/rancher/k3s/kubelet-config.yaml; then
  cat <<'EOF' | $SUDO tee /etc/rancher/k3s/kubelet-config.yaml >/dev/null
apiVersion: kubelet.config.k8s.io/v1beta1
kind: KubeletConfiguration
imageGCHighThresholdPercent: 99
imageGCLowThresholdPercent: 98
failSwapOn: false
memorySwap:
  swapBehavior: LimitedSwap
evictionHard:
  memory.available: "100Mi"
  nodefs.available: "1%"
  nodefs.inodesFree: "1%"
  imagefs.available: "1%"
  imagefs.inodesFree: "1%"
containerLogMaxSize: "50Mi"
containerLogMaxFiles: 2
systemReserved:
  memory: "2Gi"
EOF
fi
if ! grep -Eq 'config=/etc/rancher/k3s/kubelet-config.yaml' /etc/rancher/k3s/config.yaml /etc/rancher/k3s/config.yaml.d/*.yaml 2>/dev/null; then
  $SUDO mkdir -p /etc/rancher/k3s/config.yaml.d
  printf 'kubelet-arg+:\n- config=/etc/rancher/k3s/kubelet-config.yaml\n' | $SUDO tee /etc/rancher/k3s/config.yaml.d/99-dune-manager-swap.yaml >/dev/null
fi

restarted_k3s=false
if systemctl is-active --quiet k3s 2>/dev/null; then
  $SUDO systemctl restart k3s
  restarted_k3s=true
fi

if [ "$restarted_k3s" = true ]; then
  elapsed=0
  while [ ! -S /run/k3s/containerd/containerd.sock ]; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 180 ]; then echo "k3s containerd did not return after enabling swap" >&2; exit 1; fi
  done
  elapsed=0
  consecutive_successes=0
  while [ "$consecutive_successes" -lt 2 ]; do
    if $SUDO kubectl get nodes >/dev/null 2>&1; then
      consecutive_successes=$((consecutive_successes + 1))
      sleep 2
    else
      consecutive_successes=0
      sleep 2
    fi
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 240 ]; then echo "k3s API did not return after enabling swap" >&2; exit 1; fi
  done
  $SUDO kubectl wait --for=condition=Ready node --all --timeout=180s >/dev/null || true
fi

swap_file_exists=false
swap_active=false
swap_file_bytes=0
swap_total_bytes=0
fstab_configured=false
kubelet_swap_configured=false

[ -f /swapfile ] && swap_file_exists=true
[ -f /swapfile ] && swap_file_bytes=$(stat -c '%s' /swapfile 2>/dev/null || echo 0)
if awk '$1 == "/swapfile" {{ found = 1 }} END {{ exit(found ? 0 : 1) }}' /proc/swaps 2>/dev/null; then
  swap_active=true
fi
swap_total_bytes=$(awk '$1 == "SwapTotal:" {{ print $2 * 1024 }}' /proc/meminfo 2>/dev/null | head -n1)
[ -n "$swap_total_bytes" ] || swap_total_bytes=0
if grep -Eq '^[[:space:]]*/swapfile[[:space:]]' /etc/fstab 2>/dev/null; then
  fstab_configured=true
fi
if grep -Eq '^[[:space:]]*kind:[[:space:]]*KubeletConfiguration[[:space:]]*$' /etc/rancher/k3s/kubelet-config.yaml 2>/dev/null \
  && grep -Eq '^[[:space:]]*failSwapOn:[[:space:]]*false[[:space:]]*$' /etc/rancher/k3s/kubelet-config.yaml 2>/dev/null \
  && grep -Eq 'config=/etc/rancher/k3s/kubelet-config.yaml' /etc/rancher/k3s/config.yaml /etc/rancher/k3s/config.yaml.d/*.yaml 2>/dev/null; then
  kubelet_swap_configured=true
fi

printf '{{"swapFileExists":%s,"swapActive":%s,"swapFileBytes":%s,"swapTotalBytes":%s,"fstabConfigured":%s,"kubeletSwapConfigured":%s}}\n' \
  "$swap_file_exists" "$swap_active" "$swap_file_bytes" "$swap_total_bytes" "$fstab_configured" "$kubelet_swap_configured"
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubuntu_swap_uses_k3s_drop_in_without_overwriting_config() {
        let script = ubuntu_swap_script(30);

        assert!(script.contains("/etc/rancher/k3s/config.yaml.d"));
        assert!(script.contains("kubelet-arg+:"));
        assert!(!script.contains("tee /etc/rancher/k3s/config.yaml >/dev/null"));
        assert!(script.contains("consecutive_successes"));
        assert!(script.contains("k3s API did not return after enabling swap"));
    }
}
