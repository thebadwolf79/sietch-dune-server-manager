//! SSH-based installation of the VM-side Manager API service.

use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use serde::Serialize;

use crate::{
    errors::failure,
    models::CommandResult,
    orchestration::{
        OperationSink, OrchestrationEvent, ProviderKind, RemoteCommandRunner, StepAction,
        StepDomain,
    },
    validation::{validate_kube_arg, validate_plain_value},
};

const DEFAULT_REMOTE_BINARY_PATH: &str = "/opt/dune-manager/dune-manager-api";
const DEFAULT_ENV_PATH: &str = "/etc/dune-manager-api.env";
const DEFAULT_SERVICE_PATH: &str = "/etc/init.d/dune-manager-api";
const DEFAULT_LOG_PATH: &str = "/var/log/dune-manager-api.log";
const DEFAULT_KUBECONFIG_PATH: &str = "/etc/rancher/k3s/k3s.yaml";
const DEFAULT_SERVICE_NAME: &str = "dune-manager-api";
const DEFAULT_PORT: u16 = 8787;
const UPLOAD_B64_PATH: &str = "/tmp/dune-manager-api-upload.b64";
const UPLOAD_BIN_PATH: &str = "/tmp/dune-manager-api-upload.bin";
const BASE64_CHUNK_SIZE: usize = 512 * 1024;

/// Request for installing or updating the Manager API service inside the guest VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagerApiInstallRequest {
    /// Local path to the Linux Manager API executable.
    pub binary_path: PathBuf,
    /// Bearer token required by the Manager API.
    pub token: String,
    /// Kubernetes namespace the Manager API should treat as its default battlegroup namespace.
    pub namespace: String,
    /// TCP port used by the Manager API inside the VM.
    pub port: u16,
    /// Optional internal Director base URL. When omitted, the Manager API discovers Director.
    pub director_base_url: Option<String>,
    /// OpenRC service name to install.
    pub service_name: String,
    /// Remote executable path inside the guest VM.
    pub remote_binary_path: String,
    /// Remote environment file path inside the guest VM.
    pub env_path: String,
    /// Remote service log file path inside the guest VM.
    pub log_path: String,
    /// Guest kubeconfig path used by the root-owned OpenRC service.
    pub kubeconfig_path: String,
}

impl ManagerApiInstallRequest {
    /// Creates a request using the standard Manager API service paths and port.
    pub fn new(
        binary_path: impl Into<PathBuf>,
        token: impl Into<String>,
        namespace: impl Into<String>,
    ) -> Self {
        Self {
            binary_path: binary_path.into(),
            token: token.into(),
            namespace: namespace.into(),
            port: DEFAULT_PORT,
            director_base_url: None,
            service_name: DEFAULT_SERVICE_NAME.to_string(),
            remote_binary_path: DEFAULT_REMOTE_BINARY_PATH.to_string(),
            env_path: DEFAULT_ENV_PATH.to_string(),
            log_path: DEFAULT_LOG_PATH.to_string(),
            kubeconfig_path: DEFAULT_KUBECONFIG_PATH.to_string(),
        }
    }

    /// Validates the local executable, service paths, and environment values.
    pub fn validate(&self) -> CommandResult<()> {
        require_existing_file(&self.binary_path, "Manager API binary")?;
        validate_plain_value(&self.token, "Manager API token")?;
        validate_kube_arg(&self.namespace, "namespace")?;
        validate_plain_value(&self.service_name, "Manager API service name")?;
        validate_absolute_remote_path(&self.remote_binary_path, "remote binary path")?;
        validate_absolute_remote_path(&self.env_path, "env path")?;
        validate_absolute_remote_path(&self.log_path, "log path")?;
        validate_absolute_remote_path(&self.kubeconfig_path, "kubeconfig path")?;
        if let Some(url) = &self.director_base_url {
            validate_plain_value(url, "Director base URL")?;
        }
        Ok(())
    }
}

/// Result returned after installing and starting the Manager API service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerApiInstallResult {
    /// Installed OpenRC service name.
    pub service_name: String,
    /// Remote executable path.
    pub binary_path: String,
    /// Remote environment file path.
    pub env_path: String,
    /// Remote log file path.
    pub log_path: String,
    /// TCP port configured for the Manager API.
    pub port: u16,
    /// Local-in-guest health URL used for verification.
    pub health_url: String,
}

/// Reachability result for the installed Manager API service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerApiStatus {
    /// Whether the health endpoint responded from inside the guest.
    pub reachable: bool,
    /// Raw health body returned by the service, when reachable.
    pub health_body: Option<String>,
    /// TCP port checked inside the guest VM.
    pub port: u16,
}

/// Installs and verifies the VM-side Manager API using an SSH command runner.
#[derive(Debug, Clone)]
pub struct ManagerApiInstaller<R> {
    runner: R,
}

impl<R> ManagerApiInstaller<R>
where
    R: RemoteCommandRunner,
{
    /// Creates a Manager API installer around a remote command runner.
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    /// Uploads the binary, writes the OpenRC service, starts it, and checks `/health`.
    pub fn install(
        &self,
        request: &ManagerApiInstallRequest,
        sink: &mut impl OperationSink,
    ) -> CommandResult<ManagerApiInstallResult> {
        request.validate()?;

        emit_manager_event(
            sink,
            "manager.upload-binary",
            "Uploading Manager API binary.",
            StepAction::Upload,
        );
        let bytes = fs::read(&request.binary_path).map_err(|err| {
            failure(format!(
                "Failed to read {}: {err}",
                request.binary_path.display()
            ))
        })?;
        self.upload_binary(&bytes, &request.remote_binary_path)?;

        emit_manager_event(
            sink,
            "manager.write-config",
            "Writing Manager API environment.",
            StepAction::Configure,
        );
        self.write_config(request)?;

        emit_manager_event(
            sink,
            "manager.write-service",
            "Installing Manager API OpenRC service.",
            StepAction::Configure,
        );
        self.write_service(request)?;

        emit_manager_event(
            sink,
            "manager.start-service",
            "Starting Manager API service.",
            StepAction::Start,
        );
        self.restart_service(&request.service_name)?;

        emit_manager_event(
            sink,
            "manager.verify-health",
            "Verifying Manager API health.",
            StepAction::Check,
        );
        self.wait_for_health(request.port, 30)?;

        Ok(ManagerApiInstallResult {
            service_name: request.service_name.clone(),
            binary_path: request.remote_binary_path.clone(),
            env_path: request.env_path.clone(),
            log_path: request.log_path.clone(),
            port: request.port,
            health_url: health_url(request.port),
        })
    }

    /// Checks whether the Manager API health endpoint is reachable inside the guest.
    pub fn status(&self, port: u16) -> CommandResult<ManagerApiStatus> {
        let script = format!(
            r#"set -eu
out=$(wget -qO- {} 2>/dev/null || true)
printf '%s' "$out"
"#,
            shell_quote(&health_url(port))
        );
        let output = self.runner.run_script(&script)?;
        let trimmed = output.trim().to_string();
        Ok(ManagerApiStatus {
            reachable: !trimmed.is_empty(),
            health_body: (!trimmed.is_empty()).then_some(trimmed),
            port,
        })
    }

    fn upload_binary(&self, bytes: &[u8], remote_path: &str) -> CommandResult<()> {
        let setup_script = format!(
            "set -eu\nrm -f {} {}\ntouch {}\nchmod 600 {}\n",
            shell_quote(UPLOAD_B64_PATH),
            shell_quote(UPLOAD_BIN_PATH),
            shell_quote(UPLOAD_B64_PATH),
            shell_quote(UPLOAD_B64_PATH)
        );
        self.runner.run_script(&setup_script)?;

        let encoded = base64_encode(bytes);
        for chunk in encoded.as_bytes().chunks(BASE64_CHUNK_SIZE) {
            let chunk = std::str::from_utf8(chunk)
                .map_err(|_| failure("Base64 payload contained invalid UTF-8"))?;
            let script = format!(
                "set -eu\ncat >> {} <<'__DUNE_MANAGER_API_CHUNK__'\n{}\n__DUNE_MANAGER_API_CHUNK__\n",
                shell_quote(UPLOAD_B64_PATH),
                chunk
            );
            self.runner.run_script(&script)?;
        }

        let finalize_script = format!(
            "set -eu\nbase64 -d {} > {}\nsudo install -D -m 0755 {} {}\nrm -f {} {}\n",
            shell_quote(UPLOAD_B64_PATH),
            shell_quote(UPLOAD_BIN_PATH),
            shell_quote(UPLOAD_BIN_PATH),
            shell_quote(remote_path),
            shell_quote(UPLOAD_B64_PATH),
            shell_quote(UPLOAD_BIN_PATH)
        );
        self.runner.run_script(&finalize_script)?;
        Ok(())
    }

    fn write_config(&self, request: &ManagerApiInstallRequest) -> CommandResult<()> {
        let mut env = String::new();
        env.push_str(&env_assignment("MANAGER_API_TOKEN", &request.token));
        env.push_str(&env_assignment("DUNE_NAMESPACE", &request.namespace));
        env.push_str(&env_assignment("PORT", &request.port.to_string()));
        env.push_str(&env_assignment("RUST_LOG", "dune_manager_api=info"));
        env.push_str(&env_assignment("KUBECONFIG", &request.kubeconfig_path));
        if let Some(url) = &request.director_base_url {
            env.push_str(&env_assignment("DIRECTOR_BASE_URL", url));
        }
        self.install_text_file(&request.env_path, "0600", &env)
    }

    fn write_service(&self, request: &ManagerApiInstallRequest) -> CommandResult<()> {
        let service = format!(
            r#"#!/sbin/openrc-run
name="Dune Manager API"
description="Authenticated control plane for the Dune dedicated server"

if [ -f {env_path} ]; then
    . {env_path}
    export MANAGER_API_TOKEN DUNE_NAMESPACE DIRECTOR_BASE_URL PORT RUST_LOG KUBECONFIG
fi

command={binary_path}
command_background=true
pidfile="/run/{service_name}.pid"
output_log={log_path}
error_log={log_path}

depend() {{
    need net
    after k3s
}}

start_pre() {{
    checkpath --directory --mode 0755 /opt/dune-manager
    checkpath --file --mode 0640 {log_path}
}}
"#,
            env_path = shell_quote(&request.env_path),
            binary_path = shell_quote(&request.remote_binary_path),
            service_name = request.service_name,
            log_path = shell_quote(&request.log_path),
        );
        self.install_text_file(DEFAULT_SERVICE_PATH, "0755", &service)
    }

    fn install_text_file(&self, path: &str, mode: &str, content: &str) -> CommandResult<()> {
        let script = format!(
            "set -eu\ntmp=$(mktemp)\ncat > \"$tmp\" <<'__DUNE_MANAGER_FILE__'\n{}{}__DUNE_MANAGER_FILE__\nsudo install -D -m {} \"$tmp\" {}\nrm -f \"$tmp\"\n",
            content,
            if content.ends_with('\n') { "" } else { "\n" },
            shell_quote(mode),
            shell_quote(path)
        );
        self.runner.run_script(&script)?;
        Ok(())
    }

    fn restart_service(&self, service_name: &str) -> CommandResult<()> {
        let service_name = shell_quote(service_name);
        let script = format!(
            "set -eu\nsudo rc-update add {service_name} default >/dev/null 2>&1 || true\nsudo rc-service {service_name} stop >/dev/null 2>&1 || true\nsudo rc-service {service_name} start\n"
        );
        self.runner.run_script(&script)?;
        Ok(())
    }

    fn wait_for_health(&self, port: u16, timeout_seconds: u64) -> CommandResult<()> {
        let started = std::time::Instant::now();
        while started.elapsed().as_secs() < timeout_seconds {
            if self.status(port)?.reachable {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(1));
        }
        Err(failure(format!(
            "Manager API did not become healthy on port {port}"
        )))
    }
}

fn emit_manager_event(
    sink: &mut impl OperationSink,
    step_id: &'static str,
    message: impl Into<String>,
    action: StepAction,
) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.into(),
        domain: StepDomain::Guest,
        action,
        provider: ProviderKind::Ssh,
    });
}

fn require_existing_file(path: &Path, label: &str) -> CommandResult<()> {
    if !path.is_file() {
        return Err(failure(format!(
            "{label} was not found: {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_absolute_remote_path(value: &str, label: &str) -> CommandResult<()> {
    validate_plain_value(value, label)?;
    if !value.starts_with('/') {
        return Err(failure(format!("{label} must be an absolute guest path")));
    }
    Ok(())
}

fn health_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/health")
}

fn env_assignment(name: &str, value: &str) -> String {
    format!("{name}={}\n", shell_quote(value))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        collections::VecDeque,
        rc::Rc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::orchestration::VecOperationSink;

    #[derive(Clone, Default)]
    struct MockRemote {
        outputs: Rc<RefCell<VecDeque<String>>>,
        scripts: Rc<RefCell<Vec<String>>>,
    }

    impl MockRemote {
        fn with_outputs(outputs: impl IntoIterator<Item = impl Into<String>>) -> Self {
            Self {
                outputs: Rc::new(RefCell::new(outputs.into_iter().map(Into::into).collect())),
                scripts: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl RemoteCommandRunner for MockRemote {
        fn run(&self, command: &str) -> CommandResult<String> {
            self.run_script(command)
        }

        fn run_script(&self, script: &str) -> CommandResult<String> {
            self.scripts.borrow_mut().push(script.to_string());
            Ok(self.outputs.borrow_mut().pop_front().unwrap_or_default())
        }
    }

    #[test]
    fn installs_manager_api_without_serializing_token_in_result() {
        let binary = test_file("manager-api", b"test-binary");
        let remote = MockRemote::with_outputs(["", "", "", "", "", "", r#"{"ok":true}"#]);
        let scripts = remote.scripts.clone();
        let installer = ManagerApiInstaller::new(remote);
        let mut request =
            ManagerApiInstallRequest::new(&binary, "manager-token-secret", "funcom-seabass-test");
        request.director_base_url = Some("http://127.0.0.1:30841".to_string());
        let mut sink = VecOperationSink::default();

        let result = installer.install(&request, &mut sink).unwrap();
        let json = serde_json::to_string(&result).unwrap();
        let joined_scripts = scripts.borrow().join("\n");

        assert_eq!(result.port, DEFAULT_PORT);
        assert!(!json.contains("manager-token-secret"));
        assert!(joined_scripts.contains("MANAGER_API_TOKEN"));
        assert!(joined_scripts.contains("rc-service 'dune-manager-api' start"));
        assert!(sink
            .events
            .iter()
            .any(|event| event.step_id == "manager.verify-health"));
        let _ = fs::remove_file(binary);
    }

    #[test]
    fn status_reports_reachable_when_health_has_a_body() {
        let remote = MockRemote::with_outputs([r#"{"ok":true}"#]);
        let status = ManagerApiInstaller::new(remote).status(8787).unwrap();
        assert!(status.reachable);
        assert_eq!(status.health_body.as_deref(), Some(r#"{"ok":true}"#));
    }

    #[test]
    fn validates_required_token() {
        let binary = test_file("manager-api-empty-token", b"test-binary");
        let request = ManagerApiInstallRequest::new(&binary, "", "funcom-seabass-test");
        let err = request.validate().unwrap_err();
        assert!(err.message.contains("Manager API token"));
        let _ = fs::remove_file(binary);
    }

    #[test]
    fn base64_encoder_matches_standard_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    fn test_file(name: &str, contents: &[u8]) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("dune-manager-{name}-{nanos}"));
        fs::write(&path, contents).unwrap();
        path
    }
}
