use super::models::{sh_single_quoted, UbuntuSshPrepareRequest};
use super::operator_yaml::OPERATOR_DEPLOYMENTS_YAML;

pub(super) fn bootstrap_kubernetes_script(request: &UbuntuSshPrepareRequest) -> String {
    format!(
        r#"
set -eu
SERVER_ROOT={server_root}
DOWNLOAD_PATH="$SERVER_ROOT/download"
if [ ! -d "$DOWNLOAD_PATH/images/operators/crds" ]; then
  echo "Dune server payload is missing operator CRDs at $DOWNLOAD_PATH/images/operators/crds." >&2
  exit 1
fi

wait_k3s_ready() {{
  elapsed=0
  while [ ! -S /run/k3s/containerd/containerd.sock ]; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 180 ]; then
      echo "k3s containerd socket did not become ready in 180s" >&2
      return 1
    fi
  done

  elapsed=0
  until sudo k3s ctr version >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 180 ]; then
      echo "k3s containerd did not accept commands in 180s" >&2
      return 1
    fi
  done

  elapsed=0
  until sudo kubectl get nodes >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if [ "$elapsed" -ge 180 ]; then
      echo "k3s API did not become ready in 180s" >&2
      return 1
    fi
  done
}}

load_image_from_file() {{
  file_name="$1"
  if [ ! -f "$DOWNLOAD_PATH/$file_name" ]; then
    echo "Image file $DOWNLOAD_PATH/$file_name does not exist" >&2
    exit 1
  fi
  attempt=1
  while [ "$attempt" -le 8 ]; do
    wait_k3s_ready
    if out=$(sudo k3s ctr images import "$DOWNLOAD_PATH/$file_name" 2>&1); then
      return 0
    fi
    printf '%s\n' "$out" >&2
    sleep 10
    attempt=$((attempt + 1))
  done
  echo "Failed to import $file_name after 8 attempts" >&2
  exit 1
}}

kubectl_retry() {{
  attempt=1
  last_out=""
  while [ "$attempt" -le 30 ]; do
    if out=$(sudo kubectl "$@" 2>&1); then
      [ -n "$out" ] && printf '%s\n' "$out" >&2
      return 0
    fi
    last_out="$out"
    if printf '%s' "$out" | grep -qiE 'connection refused|unable to connect to the server|i/o timeout|tls handshake|no route to host|EOF|ServiceUnavailable|currently unable to handle the request|Too Many Requests|timeout awaiting response headers'; then
      sleep 10
      attempt=$((attempt + 1))
      continue
    fi
    printf '%s\n' "$out" >&2
    return 1
  done
  echo "kubectl $* still failing after retries" >&2
  [ -n "$last_out" ] && printf '%s\n' "$last_out" >&2
  return 1
}}

wait_k3s_settled() {{
  elapsed=0
  stable=0
  while [ "$elapsed" -lt 300 ]; do
    if sudo kubectl get --raw=/readyz >/dev/null 2>&1 \
      && sudo kubectl get namespaces >/dev/null 2>&1 \
      && sudo kubectl get nodes >/dev/null 2>&1; then
      stable=$((stable + 1))
      if [ "$stable" -ge 3 ]; then
        return 0
      fi
    else
      stable=0
    fi
    sleep 10
    elapsed=$((elapsed + 10))
  done
  echo "k3s API did not stay ready for 3 consecutive checks within 300s" >&2
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

wait_deployment_created() {{
  ns="$1"
  name="$2"
  elapsed=0
  until sudo kubectl get -n "$ns" deployment "$name" >/dev/null 2>&1; do
    sleep 3
    elapsed=$((elapsed + 3))
    if [ "$elapsed" -ge 240 ]; then
      echo "deployment $ns/$name did not appear within 240s" >&2
      return 1
    fi
  done
}}

wait_k3s_ready
load_image_from_file "images/prerequisites/coredns-coredns.tar"
load_image_from_file "images/prerequisites/local-path-provisioner.tar"
load_image_from_file "images/prerequisites/metrics-server.tar"
load_image_from_file "images/prerequisites/cert-manager-webhook.tar"
load_image_from_file "images/prerequisites/cert-manager-controller.tar"
load_image_from_file "images/prerequisites/cert-manager-cainjector.tar"
load_image_from_file "images/prerequisites/igw-postgres.tar"

wait_k3s_settled
if ! sudo kubectl get deployment cert-manager -n cert-manager >/dev/null 2>&1; then
  kubectl_retry apply --validate=false -f https://github.com/cert-manager/cert-manager/releases/download/v1.8.0/cert-manager.yaml
fi
wait_deployment_created cert-manager cert-manager
wait_deployment_created cert-manager cert-manager-cainjector
wait_deployment_created cert-manager cert-manager-webhook
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

kubectl_retry apply --server-side --validate=false -f "$DOWNLOAD_PATH/images/operators/crds/"

operator_version=$(cat "$DOWNLOAD_PATH/images/operators/version.txt")
manifest="/tmp/dune-operator-deployments.yaml"
cat > "$manifest" <<'YAML'
{operator_deployments}
YAML
sed -i "s/__OPERATOR_VERSION__/$operator_version/g" "$manifest"
kubectl_retry apply --validate=false -f "$manifest"
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
