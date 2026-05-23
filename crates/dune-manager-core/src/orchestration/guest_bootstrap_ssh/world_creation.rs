use serde::Deserialize;
use serde_json::json;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::WorldManifestRequest,
    validation::validate_kube_arg,
};

use super::scripts::shell_value;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreateWorldOutput {
    pub(super) namespace: String,
    pub(super) battlegroup_name: String,
}

pub(super) fn validate_world_manifest_request(
    request: &WorldManifestRequest,
) -> CommandResult<()> {
    validate_kube_arg(&request.world_unique_name, "world unique name")?;
    validate_ipv4ish(&request.player_ip, "player-facing IP")?;
    if request.world_name.trim().is_empty()
        || request.world_name.chars().count() > 50
        || request.world_name.contains('\n')
        || request.world_name.contains('\r')
    {
        return Err(failure(
            "World name must be 1-50 characters and single-line",
        ));
    }
    match request.world_region.as_str() {
        "Asia" | "Europe" | "North America" | "Oceania" | "South America" => {}
        _ => {
            return Err(failure(
                "Region must be Asia, Europe, North America, Oceania, or South America",
            ))
        }
    }
    if request.self_host_token.trim().is_empty()
        || request.self_host_token.contains('\n')
        || request.self_host_token.contains('\r')
    {
        return Err(failure("Self-host token is required"));
    }
    Ok(())
}

fn validate_ipv4ish(value: &str, label: &str) -> CommandResult<()> {
    let parts = value.trim().split('.').collect::<Vec<_>>();
    if parts.len() == 4 && parts.iter().all(|part| part.parse::<u8>().is_ok()) {
        Ok(())
    } else {
        Err(failure(format!("{label} must be an IPv4 address")))
    }
}

pub(super) fn create_world_script(request: &WorldManifestRequest) -> String {
    let namespace = format!("funcom-seabass-{}", request.world_unique_name);
    let title_patch = json!({
        "spec": {
            "title": request.world_name.trim(),
        }
    })
    .to_string();
    let mut script = String::from("set -eu\n");
    script.push_str("G_SPEC_PATH=/home/dune/.dune\n");
    script.push_str("G_SCRIPT_PATH=/home/dune/.dune/download/scripts/setup\n");
    script.push_str(&shell_value("WORLD_NAME", request.world_name.trim()));
    script.push_str(&shell_value("WORLD_REGION", request.world_region.trim()));
    script.push_str(&shell_value("PLAYER_IP", request.player_ip.trim()));
    script.push_str(&shell_value(
        "WORLD_UNIQUE_NAME",
        &request.world_unique_name,
    ));
    script.push_str(&shell_value("NS", &namespace));
    script.push_str(&shell_value("FLS_TOKEN", request.self_host_token.trim()));
    script.push_str(&shell_value("TITLE_PATCH", &title_patch));
    script.push_str(
        r#"
if sudo kubectl get battlegroup "$WORLD_UNIQUE_NAME" -n "$NS" >/dev/null 2>&1; then
  sudo kubectl patch battlegroup "$WORLD_UNIQUE_NAME" -n "$NS" --type=merge -p "$TITLE_PATCH" >/dev/null
  printf '%s' "$WORLD_UNIQUE_NAME" > /home/dune/.dune/.manager-bootstrap-world-name
  printf '{"namespace":"%s","battlegroupName":"%s"}\n' "$NS" "$WORLD_UNIQUE_NAME"
  exit 0
fi
RMQ_SECRET=$(openssl rand -base64 64 | tr -d '\n')
DB_PASSWORD=$(openssl rand -hex 32)
DB_SUPER_PASSWORD=$(openssl rand -hex 32)
escape_sed() { printf '%s' "$1" | sed -e 's/[\/&]/\\&/g'; }
escape_sed_pipe() { printf '%s' "$1" | sed -e 's/[|&]/\\&/g'; }
cp "$G_SCRIPT_PATH/templates/world-template.yaml" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
cp "$G_SCRIPT_PATH/templates/fls-secret.yaml" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME-fls-secret.yaml"
cp "$G_SCRIPT_PATH/templates/rmq-secret.yaml" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME-rmq-secret.yaml"
sed -i "s/{WORLD_NAME}/$(escape_sed "$WORLD_NAME")/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
sed -i "s/{WORLD_UNIQUE_NAME}/$(escape_sed "$WORLD_UNIQUE_NAME")/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
sed -i "s/{WORLD_REGION}/$(escape_sed "$WORLD_REGION")/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
sed -i "s/{WORLD_IMAGE_TAG}/0-0-shipping/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
sed -i "s/{FLS_SECRET}/$(escape_sed "$FLS_TOKEN")/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
sed -i "s/{WORLD_DUNE_PASS}/$(escape_sed "$DB_PASSWORD")/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
sed -i "s/{WORLD_POSTGRES_PASS}/$(escape_sed "$DB_SUPER_PASSWORD")/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
sed -i "s/{FLS_SECRET}/$(escape_sed "$FLS_TOKEN")/g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME-fls-secret.yaml"
sed -i "s|{RMQ_SECRET}|$(escape_sed_pipe "$RMQ_SECRET")|g" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME-rmq-secret.yaml"
world_tmp="$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml.tmp"
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
' "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml" > "$world_tmp" || {
  rm -f "$world_tmp"
  echo "No HOST_DATACENTER_IP_ADDRESS values were found in world manifest" >&2
  exit 1
}
mv "$world_tmp" "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml"
elapsed=0
while [ "$elapsed" -lt 300 ]; do
  all_ready=true
  for op in battlegroupoperator-controller-manager databaseoperator-controller-manager serveroperator-controller-manager utilitiesoperator-controller-manager; do
    ready=$(sudo kubectl get -n funcom-operators deployment/"$op" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || true)
    if [ "$ready" != "1" ]; then all_ready=false; break; fi
  done
  if $all_ready; then break; fi
  sleep 5
  elapsed=$((elapsed + 5))
done
if [ "$elapsed" -ge 300 ]; then
  echo "Timed out waiting for operators" >&2
  exit 1
fi
if ! sudo kubectl get ns "$NS" >/dev/null 2>&1; then
  sudo kubectl create ns "$NS" >/dev/null
fi
sudo kubectl apply -n "$NS" -f "$G_SPEC_PATH/$WORLD_UNIQUE_NAME-fls-secret.yaml" >/dev/null
sudo kubectl apply -n "$NS" -f "$G_SPEC_PATH/$WORLD_UNIQUE_NAME-rmq-secret.yaml" >/dev/null
sudo kubectl apply -n "$NS" -f "$G_SPEC_PATH/$WORLD_UNIQUE_NAME.yaml" >/dev/null
sudo kubectl patch battlegroup "$WORLD_UNIQUE_NAME" -n "$NS" --type=merge -p "$TITLE_PATCH" >/dev/null
printf '%s' "$WORLD_UNIQUE_NAME" > /home/dune/.dune/.manager-bootstrap-world-name
printf '{"namespace":"%s","battlegroupName":"%s"}\n' "$NS" "$WORLD_UNIQUE_NAME"
"#,
    );
    script
}
