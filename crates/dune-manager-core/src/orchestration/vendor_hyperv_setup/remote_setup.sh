set -eu
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
