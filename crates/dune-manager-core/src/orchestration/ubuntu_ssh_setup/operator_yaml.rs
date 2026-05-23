pub(super) const OPERATOR_DEPLOYMENTS_YAML: &str = r#"apiVersion: v1
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
        - --db-max-concurrent=1
        - --dbdepl-max-concurrent=1
        - --dbutil-max-concurrent=1
        - --dbop-max-concurrent=1
        - --dbb-max-concurrent=1
        - --dbbs-max-concurrent=1
        - --dbr-max-concurrent=1
        - --dbm-max-concurrent=1
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ubuntu_operator_manifest_matches_vendor_database_concurrency_patch() {
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--db-max-concurrent=1"));
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--dbdepl-max-concurrent=1"));
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--dbutil-max-concurrent=1"));
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--dbop-max-concurrent=1"));
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--dbb-max-concurrent=1"));
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--dbbs-max-concurrent=1"));
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--dbr-max-concurrent=1"));
        assert!(OPERATOR_DEPLOYMENTS_YAML.contains("--dbm-max-concurrent=1"));
        assert!(!OPERATOR_DEPLOYMENTS_YAML.contains("--dbutil-max-concurrent=2"));
    }
}
