pub(super) const UPDATE_OPERATOR_CRDS_SCRIPT: &str = r#"
if operator_versions_differ; then
  kubectl_retry replace -n funcom-operators -f "$DOWNLOAD_PATH/images/operators/crds/" || kubectl_retry apply -n funcom-operators -f "$DOWNLOAD_PATH/images/operators/crds/"
fi
"#;

pub(super) const PATCH_OPERATOR_IMAGES_SCRIPT: &str = r#"
if operator_versions_differ; then
  new_operator_version=$(cat "$DOWNLOAD_PATH/images/operators/version.txt")
  patch_database_operator_concurrency
  load_image_from_file "images/operators/battlegroup-operator.tar"
  load_image_from_file "images/operators/database-operator.tar"
  load_image_from_file "images/operators/server-operator.tar"
  load_image_from_file "images/operators/utilities-operator.tar"
  kubectl_retry set -n funcom-operators image deployment/battlegroupoperator-controller-manager manager=registry.funcom.com/funcom/self-hosting/igw-k8s-battlegroup-operator:"$new_operator_version"
  kubectl_retry set -n funcom-operators image deployment/databaseoperator-controller-manager manager=registry.funcom.com/funcom/self-hosting/igw-k8s-database-operator:"$new_operator_version"
  kubectl_retry set -n funcom-operators image deployment/serveroperator-controller-manager manager=registry.funcom.com/funcom/self-hosting/igw-k8s-server-operator:"$new_operator_version"
  kubectl_retry set -n funcom-operators image deployment/utilitiesoperator-controller-manager manager=registry.funcom.com/funcom/self-hosting/igw-k8s-utilities-operator:"$new_operator_version"
fi
"#;

pub(super) const PATCH_DATABASE_OPERATOR_SCRIPT: &str = r#"
patch_database_operator_concurrency() {
  current_args=$(sudo kubectl get -n funcom-operators deployment/databaseoperator-controller-manager -o jsonpath='{.spec.template.spec.containers[0].args}' 2>/dev/null || true)
  if ! printf '%s' "$current_args" | grep -q 'dbutil-max-concurrent=2'; then
    return 0
  fi
  patch='[{"op":"replace","path":"/spec/template/spec/containers/0/args","value":["--leader-elect","--zap-devel=false","--zap-log-level=debug","--zap-time-encoding=iso8601","--db-max-concurrent=1","--dbdepl-max-concurrent=1","--dbutil-max-concurrent=1","--dbop-max-concurrent=1","--dbb-max-concurrent=1","--dbbs-max-concurrent=1","--dbr-max-concurrent=1","--dbm-max-concurrent=1","--dbutil-supports-prometheus=false"]}]'
  kubectl_retry patch deployment -n funcom-operators databaseoperator-controller-manager --type=json -p="$patch"
  kubectl_retry rollout -n funcom-operators status deployment/databaseoperator-controller-manager --timeout=120s
}
"#;

pub(super) const SCALE_OPERATOR_SCRIPT: &str = r#"
scale_deployment funcom-operators battlegroupoperator-controller-manager 1
scale_deployment funcom-operators databaseoperator-controller-manager 1
scale_deployment funcom-operators serveroperator-controller-manager 1
scale_deployment funcom-operators utilitiesoperator-controller-manager 1
"#;

pub(super) const IMPORT_BATTLEGROUP_IMAGES_SCRIPT: &str = r#"
load_image_from_file "images/battlegroup/server-rabbitmq.tar"
load_image_from_file "images/battlegroup/server-text-router.tar"
load_image_from_file "images/battlegroup/server-bg-director.tar"
load_image_from_file "images/battlegroup/server-gateway.tar"
load_image_from_file "images/battlegroup/server-db-utils.tar"
load_image_from_file "images/battlegroup/server.tar"
"#;

pub(super) const READ_BATTLEGROUP_VERSION_SCRIPT: &str = r#"
DOWNLOAD_PATH=/home/dune/.dune/download
version_file="$DOWNLOAD_PATH/images/battlegroup/version.txt"
if [ ! -f "$version_file" ]; then
  echo "No battlegroup version file found at $version_file" >&2
  exit 1
fi
cat "$version_file"
"#;

pub(super) const SYNC_POSTGRES_SUPERUSER_PASSWORD_SCRIPT: &str = r#"
DDEP="$BG-db-dbdepl"
if ! sudo kubectl get databasedeployment "$DDEP" -n "$NS" >/dev/null 2>&1; then
  DDEP=$(sudo kubectl get databasedeployments -n "$NS" --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null | awk -v bg="$BG" '$1 ~ "^" bg ".*dbdepl$" { print $1; exit }' || true)
fi
if [ -z "$DDEP" ]; then
  echo "No existing database deployment found for $BG; skipping Postgres password sync." >&2
  exit 0
fi

DBPOD="$DDEP-sts-0"
elapsed=0
while [ "$elapsed" -lt 180 ]; do
  phase=$(sudo kubectl get pod "$DBPOD" -n "$NS" -o jsonpath='{.status.phase}' 2>/dev/null || true)
  if [ "$phase" = "Running" ]; then break; fi
  sleep 5
  elapsed=$((elapsed + 5))
done
if [ "${phase:-}" != "Running" ]; then
  echo "No running database pod found for $DDEP; skipping Postgres credential sync." >&2
  exit 0
fi

SUPER_PASSWORD=$(sudo kubectl get databasedeployment "$DDEP" -n "$NS" -o jsonpath='{.spec.superPassword}' 2>/dev/null || true)
if [ -z "$SUPER_PASSWORD" ]; then
  echo "Database deployment $DDEP has no superPassword; skipping Postgres password sync." >&2
  exit 0
fi

SUPER_USER=$(sudo kubectl get databasedeployment "$DDEP" -n "$NS" -o jsonpath='{.spec.superUser}' 2>/dev/null || true)
DB_PORT=$(sudo kubectl get databasedeployment "$DDEP" -n "$NS" -o jsonpath='{.spec.port}' 2>/dev/null || true)
if [ -z "$SUPER_USER" ]; then SUPER_USER=postgres; fi
if [ -z "$DB_PORT" ]; then DB_PORT=15432; fi

DB_USER=$(sudo kubectl get databasedeployment "$DDEP" -n "$NS" -o jsonpath='{.spec.user}' 2>/dev/null || true)
DB_PASSWORD=$(sudo kubectl get databasedeployment "$DDEP" -n "$NS" -o jsonpath='{.spec.password}' 2>/dev/null || true)
DB_NAME=$(sudo kubectl get databasedeployment "$DDEP" -n "$NS" -o jsonpath='{.spec.gameDatabaseName}' 2>/dev/null || true)
if [ -z "$DB_USER" ]; then DB_USER=dune; fi
if [ -z "$DB_NAME" ]; then DB_NAME=dune; fi
if [ -z "$DB_PASSWORD" ]; then
  echo "Database deployment $DDEP has no game database password; skipping game role sync." >&2
else
  sudo kubectl exec -i -n "$NS" "$DBPOD" -- \
    psql -h 127.0.0.1 -p "$DB_PORT" -U "$SUPER_USER" -d postgres \
      -v ON_ERROR_STOP=1 \
      -v db_user="$DB_USER" \
      -v db_password="$DB_PASSWORD" \
      -v db_name="$DB_NAME" >/dev/null <<'SQL'
SELECT format('CREATE ROLE %I LOGIN PASSWORD %L', :'db_user', :'db_password')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'db_user') \gexec
ALTER ROLE :"db_user" WITH LOGIN PASSWORD :'db_password';
SELECT format('CREATE DATABASE %I OWNER %I', :'db_name', :'db_user')
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = :'db_name') \gexec
SQL
fi

ESCAPED_PASSWORD=$(printf '%s' "$SUPER_PASSWORD" | sed "s/'/''/g")
ESCAPED_USER=$(printf '%s' "$SUPER_USER" | sed 's/"/""/g')
printf "ALTER ROLE \"%s\" WITH PASSWORD '%s';\n" "$ESCAPED_USER" "$ESCAPED_PASSWORD" |
  sudo kubectl exec -i -n "$NS" "$DBPOD" -- \
    psql -h 127.0.0.1 -p "$DB_PORT" -U "$SUPER_USER" -d postgres -v ON_ERROR_STOP=1 >/dev/null
echo "Postgres credentials are aligned with database deployment $DDEP." >&2
"#;

pub(super) const APPLY_DEFAULT_SETTINGS_SCRIPT: &str = r#"
DOWNLOAD_PATH=/home/dune/.dune/download
config_dir="$DOWNLOAD_PATH/scripts/setup/config"
if ! ls "$config_dir"/User*.ini >/dev/null 2>&1; then
  echo "No User*.ini files found in $config_dir" >&2
  exit 1
fi
elapsed=0
fb_pod=""
while [ "$elapsed" -lt 240 ]; do
  fb_pod=$(sudo kubectl get pods -n "$NS" -l role=igw-filebrowser --no-headers -o custom-columns=NAME:.metadata.name 2>/dev/null | head -n1 || true)
  if [ -n "$fb_pod" ]; then break; fi
  sleep 5
  elapsed=$((elapsed + 5))
done
if [ -z "$fb_pod" ]; then
  echo "No filebrowser pod became available in $NS" >&2
  exit 1
fi
sudo kubectl exec -n "$NS" "$fb_pod" -- mkdir -p /srv/UserSettings >&2
for config_file in "$config_dir"/User*.ini; do
  filename=$(basename "$config_file")
  sudo kubectl cp "$config_file" "$NS/$fb_pod:/srv/UserSettings/$filename" >&2
done
"#;
