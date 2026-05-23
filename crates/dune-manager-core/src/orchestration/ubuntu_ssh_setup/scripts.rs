use super::models::{
    sh_single_quoted, UbuntuSshPrepareRequest, LEGACY_SERVER_APP_ID, SERVER_APP_ID,
};

pub(super) fn prepare_host_script(
    request: &UbuntuSshPrepareRequest,
    force_steamcmd: bool,
) -> String {
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

pub(super) fn install_payload_script(request: &UbuntuSshPrepareRequest) -> String {
    format!(
        r#"
set -eu
LINUX_USER={linux_user}
SERVER_ROOT={server_root}
STEAMCMD_URL={steamcmd_url}
DOWNLOAD_PATH="$SERVER_ROOT/download"
USER_HOME=$(getent passwd "$LINUX_USER" | cut -d: -f6)
STEAMCMD="$USER_HOME/Steam/steamcmd.sh"
STEAM_HOME="$USER_HOME/Steam"
if [ ! -x "$STEAMCMD" ]; then
  echo "SteamCMD is missing at $STEAMCMD; reinstalling it." >&2
  mkdir -p "$STEAM_HOME" "$USER_HOME/.steam"
  chown -R "$LINUX_USER:$LINUX_USER" "$STEAM_HOME" "$USER_HOME/.steam"
  tmp="$(mktemp -t dune-steamcmd.XXXXXX.tar.gz)"
  curl -fsSL "$STEAMCMD_URL" -o "$tmp"
  chmod 0644 "$tmp"
  sudo -u "$LINUX_USER" tar -xzf "$tmp" -C "$STEAM_HOME"
  rm -f "$tmp"
  sudo -u "$LINUX_USER" ln -sfn "$STEAM_HOME" "$USER_HOME/.steam/root"
  sudo -u "$LINUX_USER" ln -sfn "$STEAM_HOME" "$USER_HOME/.steam/steam"
fi
if [ ! -x "$STEAMCMD" ]; then
  echo "SteamCMD install did not produce an executable at $STEAMCMD." >&2
  exit 1
fi
mkdir -p "$DOWNLOAD_PATH"
chown -R "$LINUX_USER:$LINUX_USER" "$SERVER_ROOT"
if [ -f "$DOWNLOAD_PATH/steamapps/appmanifest_{legacy_app_id}.acf" ] && [ ! -f "$DOWNLOAD_PATH/steamapps/appmanifest_{app_id}.acf" ]; then
  find "$DOWNLOAD_PATH" -mindepth 1 -maxdepth 1 -exec rm -rf {{}} +
  mkdir -p "$DOWNLOAD_PATH"
  chown -R "$LINUX_USER:$LINUX_USER" "$DOWNLOAD_PATH"
elif [ -f "$DOWNLOAD_PATH/steamapps/appmanifest_{legacy_app_id}.acf" ]; then
  rm -f "$DOWNLOAD_PATH/steamapps/appmanifest_{legacy_app_id}.acf"
fi

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
        steamcmd_url = sh_single_quoted(&request.steamcmd_url),
        app_id = SERVER_APP_ID,
        legacy_app_id = LEGACY_SERVER_APP_ID,
    )
    .replacen(
        "set -eu\n",
        "set -eu\njson_quote() { python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' \"$1\"; }\n",
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_install_migrates_old_playtest_manifest_to_release_app() {
        let script = install_payload_script(&UbuntuSshPrepareRequest::default());

        assert!(script.contains("appmanifest_3104830.acf"));
        assert!(script.contains("appmanifest_4754530.acf"));
        assert!(script.contains("find \"$DOWNLOAD_PATH\" -mindepth 1 -maxdepth 1 -exec rm -rf"));
        assert!(script.contains("+app_update 4754530 validate"));
    }

    #[test]
    fn payload_install_repairs_missing_steamcmd() {
        let script = install_payload_script(&UbuntuSshPrepareRequest::default());

        assert!(script.contains("SteamCMD is missing at $STEAMCMD; reinstalling it."));
        assert!(script.contains("curl -fsSL \"$STEAMCMD_URL\" -o \"$tmp\""));
        assert!(script.contains("tar -xzf \"$tmp\" -C \"$STEAM_HOME\""));
        assert!(script.contains("SteamCMD install did not produce an executable"));
    }
}
