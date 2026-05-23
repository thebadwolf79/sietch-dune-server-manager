pub(super) const DUNE_HOME: &str = "/home/dune/.dune";
pub(super) const SERVER_APP_ID: &str = "4754530";

pub(super) fn with_guest_path(script: &str) -> String {
    format!(
        "export PATH=\"/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$PATH\"\n{script}"
    )
}

pub(super) fn download_script() -> String {
    format!(
        r#"
set -eu
DUNE_USER_PATH={dune_home}
DOWNLOAD_PATH="$DUNE_USER_PATH/download"
mkdir -p "$DOWNLOAD_PATH"
steamcmd_update_once() {{
  if command -v timeout >/dev/null 2>&1; then
    timeout 45m steamcmd +@ShutdownOnFailedCommand 1 +@NoPromptForPassword 1 +set_spew_level 1 1 +force_install_dir "$DOWNLOAD_PATH" +login anonymous +app_update {app_id} validate +logoff +quit < /dev/null >&2
  else
    steamcmd +@ShutdownOnFailedCommand 1 +@NoPromptForPassword 1 +set_spew_level 1 1 +force_install_dir "$DOWNLOAD_PATH" +login anonymous +app_update {app_id} validate +logoff +quit < /dev/null >&2
  fi
}}
attempt=1
max_attempts=5
while [ "$attempt" -le "$max_attempts" ]; do
  echo "SteamCMD payload download attempt $attempt/$max_attempts." >&2
  if steamcmd_update_once; then
    break
  fi
  status=$?
  if [ "$attempt" -ge "$max_attempts" ]; then
    echo "SteamCMD payload download failed after $max_attempts attempts, last exit code $status." >&2
    exit "$status"
  fi
  sleep_seconds=$((attempt * 15))
  echo "SteamCMD payload download failed with exit code $status; retrying in ${{sleep_seconds}}s." >&2
  sleep "$sleep_seconds"
  attempt=$((attempt + 1))
done
test -f "$DOWNLOAD_PATH/scripts/battlegroup.sh"
test -f "$DOWNLOAD_PATH/scripts/setup.sh"
"#,
        dune_home = DUNE_HOME,
        app_id = SERVER_APP_ID
    )
}

pub(super) fn shell_value(name: &str, value: &str) -> String {
    let delimiter = format!("__DUNE_MANAGER_{name}__");
    format!("{name}=$(cat <<'{delimiter}'\n{value}\n{delimiter}\n)\n")
}

pub(super) fn sh_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(super) const DISK_SCRIPT: &str = r#"
set -eu
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
  echo "Not enough guest disk space after resize: ${available_gb}GB available, need more than ${required_gb}GB" >&2
  exit 1
fi
"#;

pub(super) const START_K3S_SCRIPT: &str = r#"
set -eu
sudo rc-service k3s start >&2 || true
sudo rc-service k3s restart >&2
elapsed=0
while [ ! -S /run/k3s/containerd/containerd.sock ]; do
  sleep 2
  elapsed=$((elapsed + 2))
  if [ "$elapsed" -ge 60 ]; then echo "k3s containerd did not return in 60s" >&2; exit 1; fi
done
elapsed=0
until sudo kubectl get nodes >/dev/null 2>&1; do
  sleep 2
  elapsed=$((elapsed + 2))
  if [ "$elapsed" -ge 60 ]; then echo "k3s API did not return in 60s" >&2; exit 1; fi
done
sudo kubectl wait --for=condition=Ready node --all --timeout=180s >/dev/null || true
sudo rc-update add k3s >/dev/null
"#;

pub(super) const CONTAINER_IMAGE_HELPERS: &str = r#"
set -eu
DOWNLOAD_PATH=/home/dune/.dune/download
load_image_from_file() {
  local file_name="$1"
  if [ ! -f "$DOWNLOAD_PATH/$file_name" ]; then
    echo "Image file $DOWNLOAD_PATH/$file_name does not exist" >&2
    exit 1
  fi
  local attempt=1
  while [ "$attempt" -le 3 ]; do
    if sudo ctr -n k8s.io images import "$DOWNLOAD_PATH/$file_name" >&2; then
      return 0
    fi
    echo "Import of $file_name failed (attempt $attempt/3)." >&2
    if ! sudo ctr -n k8s.io version >/dev/null 2>&1; then
      echo "k3s/containerd is not responding; restarting k3s." >&2
      restart_k3s_and_wait_until_ready
    else
      sleep 5
    fi
    attempt=$((attempt + 1))
  done
  echo "Failed to import $file_name after 3 attempts" >&2
  exit 1
}
"#;

pub(super) const IMPORT_CORE_IMAGES_SCRIPT: &str = r#"
load_image_from_file "images/prerequisites/coredns-coredns.tar"
load_image_from_file "images/prerequisites/local-path-provisioner.tar"
load_image_from_file "images/prerequisites/metrics-server.tar"
load_image_from_file "images/prerequisites/cert-manager-webhook.tar"
load_image_from_file "images/prerequisites/cert-manager-controller.tar"
load_image_from_file "images/prerequisites/cert-manager-cainjector.tar"
load_image_from_file "images/prerequisites/igw-postgres.tar"
"#;

pub(super) const KUBECTL_HELPERS: &str = r#"
set -eu
kubectl_retry() {
  local attempt=1 out rc
  while [ "$attempt" -le 5 ]; do
    out=$(sudo kubectl "$@" 2>&1)
    rc=$?
    if [ "$rc" -eq 0 ]; then
      [ -n "$out" ] && printf '%s\n' "$out" >&2
      return 0
    fi
    if printf '%s' "$out" | grep -qiE 'connection refused|unable to connect to the server|i/o timeout|tls handshake|no route to host|EOF'; then
      sleep 5
      attempt=$((attempt + 1))
      continue
    fi
    printf '%s\n' "$out" >&2
    return "$rc"
  done
  echo "kubectl $* still failing after retries" >&2
  return 1
}
restart_k3s_and_wait_until_ready() {
  local elapsed=0
  sudo rc-service k3s restart >&2
  echo "Waiting for k3s containerd socket..." >&2
  while [ ! -S /run/k3s/containerd/containerd.sock ]; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 60 ]; then echo "k3s containerd did not return in 60s" >&2; return 1; fi
  done
  echo "Waiting for k3s API server..." >&2
  elapsed=0
  until sudo kubectl get nodes >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 60 ]; then echo "k3s API did not return in 60s" >&2; return 1; fi
  done
}
wait_for_deployment() {
  local ns="$1" name="$2" timeout="${3:-120}" elapsed=0
  until sudo kubectl get -n "$ns" deployment "$name" >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge "$timeout" ]; then echo "deployment $ns/$name did not appear within ${timeout}s" >&2; return 1; fi
  done
}
scale_deployment() {
  local ns="$1" name="$2" replicas="$3"
  wait_for_deployment "$ns" "$name" 120
  kubectl_retry scale -n "$ns" "deployment/$name" "--replicas=$replicas"
}
operator_versions_differ() {
  local version_file="$DOWNLOAD_PATH/images/operators/version.txt"
  if [ ! -f "$version_file" ]; then
    echo "No operator version file found at $version_file" >&2
    return 1
  fi
  local current_version new_operator_version
  current_version=$(kubectl_retry get -n funcom-operators deployment/battlegroupoperator-controller-manager -o jsonpath='{.spec.template.spec.containers[0].image}' | sed 's/.*://')
  new_operator_version=$(cat "$version_file")
  [ "$current_version" != "$new_operator_version" ]
}
DOWNLOAD_PATH=/home/dune/.dune/download
"#;

pub(super) const SCALE_CORE_SCRIPT: &str = r#"
scale_deployment kube-system coredns 1
scale_deployment kube-system local-path-provisioner 1
scale_deployment kube-system metrics-server 1
scale_deployment cert-manager cert-manager 1
scale_deployment cert-manager cert-manager-cainjector 1
scale_deployment cert-manager cert-manager-webhook 1
"#;

pub(super) const INSTALL_HELPER_SCRIPT: &str = r#"
set -eu
mkdir -p /home/dune/.dune/bin
test -f /home/dune/.dune/download/scripts/battlegroup.sh
test -f /home/dune/.dune/download/scripts/bg-util
ln -sfn /home/dune/.dune/download/scripts/battlegroup.sh /home/dune/.dune/bin/battlegroup
chmod +x /home/dune/.dune/download/scripts/battlegroup.sh
ln -sfn /home/dune/.dune/download/scripts/bg-util /home/dune/.dune/bin/bg-util
chmod +x /home/dune/.dune/download/scripts/bg-util
"#;
