//! Proxmox VM provisioning support for the vendor Alpine image.

use std::{
    collections::BTreeMap,
    fs,
    net::TcpStream,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, UNIX_EPOCH},
};

use native_tls::TlsConnector;
use reqwest::{
    blocking::{multipart, Client},
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    errors::{command_failure, failure},
    models::CommandResult,
    orchestration::{OperationSink, OrchestrationEvent, ProviderKind, StepAction, StepDomain},
    shell::suppress_console_window,
};

/// Proxmox HTTPS API connection settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxmoxClientConfig {
    /// Base URL such as `https://pve.example.test:8006`.
    pub base_url: String,
    /// API token id in Proxmox form, for example `root@pam!manager`.
    pub token_id: String,
    /// API token secret. Treat as a secret and do not persist in plain text.
    pub token_secret: String,
    /// Accepted SHA-256 fingerprint for Trust On First Use certificate checks.
    pub accepted_certificate_sha256: Option<String>,
}

impl ProxmoxClientConfig {
    /// Validates required fields and HTTPS URL shape.
    pub fn validate(&self) -> CommandResult<Url> {
        let url = Url::parse(self.base_url.trim())
            .map_err(|err| failure(format!("Proxmox URL is invalid: {err}")))?;
        if url.scheme() != "https" {
            return Err(failure("Proxmox URL must use https"));
        }
        if url.host_str().unwrap_or_default().trim().is_empty() {
            return Err(failure("Proxmox host is required"));
        }
        if self.token_id.trim().is_empty() {
            return Err(failure("Proxmox API token id is required"));
        }
        if self.token_secret.trim().is_empty() {
            return Err(failure("Proxmox API token secret is required"));
        }
        Ok(url)
    }
}

/// HTTPS client for the subset of Proxmox API used by the manager.
#[derive(Debug, Clone)]
pub struct ProxmoxClient {
    config: ProxmoxClientConfig,
    base_url: Url,
    http: Client,
}

impl ProxmoxClient {
    /// Creates a Proxmox client and verifies the certificate fingerprint when provided.
    pub fn new(config: ProxmoxClientConfig) -> CommandResult<Self> {
        let base_url = config.validate()?;
        if let Some(expected) = config.accepted_certificate_sha256.as_deref() {
            let actual = proxmox_certificate_sha256(&base_url)?;
            if !fingerprints_equal(&actual, expected) {
                return Err(failure(format!(
                    "Proxmox certificate fingerprint changed. Expected {}, got {}.",
                    normalize_fingerprint(expected),
                    actual
                )));
            }
        }
        let http = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|err| failure(format!("Failed to create Proxmox HTTP client: {err}")))?;
        Ok(Self {
            config,
            base_url,
            http,
        })
    }

    /// Reads version, node, storage, bridge, and next VMID inventory.
    pub fn detect(&self) -> CommandResult<ProxmoxDetection> {
        let version: ProxmoxVersion = self.get("/version", &[])?;
        let certificate_sha256 = proxmox_certificate_sha256(&self.base_url)?;
        let nodes: Vec<ProxmoxNode> = self.get("/nodes", &[])?;
        let first_node = nodes.first().map(|node| node.node.clone());
        let mut storages = Vec::new();
        let mut bridges = Vec::new();
        if let Some(node) = first_node.as_deref() {
            storages = self.storages(node)?;
            bridges = self.bridges(node)?;
        }
        let next_vmid = self.next_vmid()?;
        let certificate_trusted = self
            .config
            .accepted_certificate_sha256
            .as_deref()
            .is_some_and(|expected| fingerprints_equal(expected, &certificate_sha256));
        Ok(ProxmoxDetection {
            version,
            certificate_sha256,
            certificate_trusted,
            nodes,
            storages,
            bridges,
            next_vmid,
        })
    }

    /// Lists storage inventory for one node.
    pub fn storages(&self, node: &str) -> CommandResult<Vec<ProxmoxStorage>> {
        self.get(&format!("/nodes/{}/storage", encode_path(node)), &[])
    }

    /// Lists Linux bridge inventory for one node.
    pub fn bridges(&self, node: &str) -> CommandResult<Vec<ProxmoxBridge>> {
        self.get(
            &format!("/nodes/{}/network", encode_path(node)),
            &[("type", "bridge")],
        )
    }

    /// Reads the next cluster VMID.
    pub fn next_vmid(&self) -> CommandResult<u64> {
        let value: Value = self.get("/cluster/nextid", &[])?;
        parse_proxmox_u64(&value, "next VMID")
    }

    /// Reads current VM power status.
    pub fn vm_status(&self, node: &str, vmid: u64) -> CommandResult<ProxmoxVmStatus> {
        self.get(
            &format!("/nodes/{}/qemu/{vmid}/status/current", encode_path(node)),
            &[],
        )
    }

    /// Reads the current VM configuration.
    pub fn vm_config(&self, node: &str, vmid: u64) -> CommandResult<ProxmoxVmConfig> {
        self.get(
            &format!("/nodes/{}/qemu/{vmid}/config", encode_path(node)),
            &[],
        )
    }

    /// Updates selected VM configuration keys.
    pub fn update_vm_config(
        &self,
        node: &str,
        vmid: u64,
        params: &BTreeMap<String, String>,
    ) -> CommandResult<Value> {
        self.put_value(
            &format!("/nodes/{}/qemu/{vmid}/config", encode_path(node)),
            params,
        )
    }

    /// Starts a VM.
    pub fn start_vm(&self, node: &str, vmid: u64) -> CommandResult<Value> {
        self.post_value(
            &format!("/nodes/{}/qemu/{vmid}/status/start", encode_path(node)),
            &BTreeMap::new(),
        )
    }

    /// Stops a VM.
    pub fn stop_vm(&self, node: &str, vmid: u64) -> CommandResult<Value> {
        self.post_value(
            &format!("/nodes/{}/qemu/{vmid}/status/stop", encode_path(node)),
            &BTreeMap::new(),
        )
    }

    /// Creates a VM shell and attaches the imported disk using Proxmox API calls.
    pub fn create_alpine_vm(
        &self,
        request: &ProxmoxCreateVmRequest,
        qcow2_path: &Path,
        sink: &mut impl OperationSink,
    ) -> CommandResult<ProxmoxCreateVmResult> {
        request.validate(qcow2_path)?;
        emit(
            sink,
            "proxmox.vm.create",
            "Creating Proxmox VM shell.",
            StepAction::Create,
        );
        let create_params = proxmox_create_vm_params(request);
        self.post_value(
            &format!("/nodes/{}/qemu", encode_path(&request.node)),
            &create_params,
        )?;

        emit(
            sink,
            "proxmox.disk.upload",
            "Uploading converted qcow2 image to import storage.",
            StepAction::Upload,
        );
        let uploaded_name =
            self.upload_import_image(&request.node, &request.import_storage, qcow2_path)?;

        emit(
            sink,
            "proxmox.disk.import",
            "Importing qcow2 image into VM storage.",
            StepAction::Import,
        );
        let import_source = proxmox_import_volume_ref(&request.import_storage, &uploaded_name);
        let disk_value = format!(
            "{}:0,import-from={},discard=on,ssd=1",
            request.vm_storage, import_source
        );
        let resize_value = format!("{}G", request.disk_gb);
        let config_params = BTreeMap::from([
            ("scsi0".to_string(), disk_value),
            ("boot".to_string(), "order=scsi0".to_string()),
            (
                "efidisk0".to_string(),
                format!("{}:1,efitype=4m,pre-enrolled-keys=0", request.vm_storage),
            ),
        ]);
        let config_result = self.post_value(
            &format!(
                "/nodes/{}/qemu/{}/config",
                encode_path(&request.node),
                request.vmid
            ),
            &config_params,
        )?;
        self.wait_for_task_value(&request.node, &config_result, Duration::from_secs(600))?;
        self.put_value(
            &format!(
                "/nodes/{}/qemu/{}/resize",
                encode_path(&request.node),
                request.vmid
            ),
            &BTreeMap::from([
                ("disk".to_string(), "scsi0".to_string()),
                ("size".to_string(), resize_value),
            ]),
        )?;

        emit(
            sink,
            "proxmox.vm.start",
            "Starting Proxmox VM.",
            StepAction::Start,
        );
        self.start_vm(&request.node, request.vmid)?;

        Ok(ProxmoxCreateVmResult {
            node: request.node.clone(),
            vmid: request.vmid,
            vm_name: request.vm_name.clone(),
            uploaded_import_name: uploaded_name,
        })
    }

    fn upload_import_image(
        &self,
        node: &str,
        storage: &str,
        qcow2_path: &Path,
    ) -> CommandResult<String> {
        let file_name = qcow2_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| failure("Converted qcow2 path has no file name"))?
            .to_string();
        let form = multipart::Form::new()
            .text("content", "import")
            .file("filename", qcow2_path)
            .map_err(|err| failure(format!("Failed to prepare qcow2 upload: {err}")))?;
        let response = self
            .http
            .post(self.api_url(&format!(
                "/nodes/{}/storage/{}/upload",
                encode_path(node),
                encode_path(storage)
            ))?)
            .headers(self.auth_headers()?)
            .multipart(form)
            .send()
            .map_err(|err| failure(format!("Proxmox upload failed: {err}")))?;
        self.decode_response::<Value>(response)?;
        Ok(file_name)
    }

    fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> CommandResult<T> {
        let response = self
            .http
            .get(self.api_url(path)?)
            .headers(self.auth_headers()?)
            .query(query)
            .send()
            .map_err(|err| failure(format!("Proxmox request failed: {err}")))?;
        self.decode_response(response)
    }

    fn post_value(&self, path: &str, params: &BTreeMap<String, String>) -> CommandResult<Value> {
        let response = self
            .http
            .post(self.api_url(path)?)
            .headers(self.auth_headers()?)
            .form(params)
            .send()
            .map_err(|err| failure(format!("Proxmox request failed: {err}")))?;
        self.decode_response(response)
    }

    fn put_value(&self, path: &str, params: &BTreeMap<String, String>) -> CommandResult<Value> {
        let response = self
            .http
            .put(self.api_url(path)?)
            .headers(self.auth_headers()?)
            .form(params)
            .send()
            .map_err(|err| failure(format!("Proxmox request failed: {err}")))?;
        self.decode_response(response)
    }

    fn wait_for_task_value(
        &self,
        node: &str,
        value: &Value,
        timeout: Duration,
    ) -> CommandResult<()> {
        let Some(upid) = value.as_str().filter(|value| value.starts_with("UPID:")) else {
            return Ok(());
        };
        self.wait_for_task(node, upid, timeout)
    }

    fn wait_for_task(&self, node: &str, upid: &str, timeout: Duration) -> CommandResult<()> {
        let started = std::time::Instant::now();
        loop {
            let status: ProxmoxTaskStatus = self.get(
                &format!(
                    "/nodes/{}/tasks/{}/status",
                    encode_path(node),
                    encode_path(upid)
                ),
                &[],
            )?;
            if status.status == "stopped" {
                return match status.exitstatus.as_deref() {
                    None | Some("OK") => Ok(()),
                    Some(exitstatus) => Err(failure(format!(
                        "Proxmox task failed with status {exitstatus}: {upid}"
                    ))),
                };
            }
            if started.elapsed() >= timeout {
                return Err(failure(format!(
                    "Timed out waiting for Proxmox task: {upid}"
                )));
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    fn decode_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::blocking::Response,
    ) -> CommandResult<T> {
        let status = response.status();
        let text = response
            .text()
            .map_err(|err| failure(format!("Failed to read Proxmox response: {err}")))?;
        if !status.is_success() {
            return Err(failure(format!(
                "Proxmox API returned HTTP {}: {}",
                status.as_u16(),
                trim_response_text(&text)
            )));
        }
        let envelope: ProxmoxEnvelope<T> = serde_json::from_str(&text)
            .map_err(|err| failure(format!("Failed to parse Proxmox response: {err}")))?;
        Ok(envelope.data)
    }

    fn api_url(&self, path: &str) -> CommandResult<Url> {
        let mut url = self.base_url.clone();
        url.set_path(&format!("/api2/json/{}", path.trim_start_matches('/')));
        Ok(url)
    }

    fn auth_headers(&self) -> CommandResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        let value = format!(
            "PVEAPIToken={}={}",
            self.config.token_id.trim(),
            self.config.token_secret.trim()
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&value)
                .map_err(|_| failure("Proxmox token contains invalid header characters"))?,
        );
        Ok(headers)
    }
}

/// Proxmox version response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxmoxVersion {
    /// Version string.
    pub version: String,
    /// Release string.
    #[serde(default)]
    pub release: String,
    /// Repository id.
    #[serde(default)]
    pub repoid: String,
}

/// Proxmox node inventory row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxmoxNode {
    /// Node name.
    pub node: String,
    /// Node status.
    #[serde(default)]
    pub status: String,
    /// Current CPU utilization.
    #[serde(default)]
    pub cpu: f64,
    /// Logical CPU count.
    #[serde(default)]
    pub maxcpu: u64,
    /// Current memory use in bytes.
    #[serde(default)]
    pub mem: u64,
    /// Total memory in bytes.
    #[serde(default)]
    pub maxmem: u64,
}

/// Proxmox storage inventory row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxmoxStorage {
    /// Storage id.
    pub storage: String,
    /// Storage type.
    #[serde(default)]
    pub r#type: String,
    /// Comma-separated content types.
    #[serde(default)]
    pub content: String,
    /// Whether the storage is active.
    #[serde(default)]
    pub active: u8,
    /// Whether the storage is shared.
    #[serde(default)]
    pub shared: u8,
    /// Available bytes.
    #[serde(default)]
    pub avail: u64,
    /// Total bytes.
    #[serde(default)]
    pub total: u64,
}

/// Proxmox bridge inventory row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxmoxBridge {
    /// Bridge interface name.
    pub iface: String,
    /// Network object type.
    #[serde(default)]
    pub r#type: String,
    /// Whether the bridge is active.
    #[serde(default)]
    pub active: u8,
    /// CIDR address when configured.
    #[serde(default)]
    pub cidr: Option<String>,
    /// Autostart flag.
    #[serde(default)]
    pub autostart: u8,
}

/// Aggregate detection result shown by the desktop UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxmoxDetection {
    /// Proxmox version.
    pub version: ProxmoxVersion,
    /// Current TLS certificate SHA-256 fingerprint.
    pub certificate_sha256: String,
    /// Whether the current certificate matched the accepted fingerprint.
    pub certificate_trusted: bool,
    /// Cluster nodes.
    pub nodes: Vec<ProxmoxNode>,
    /// Storage inventory for the first node.
    pub storages: Vec<ProxmoxStorage>,
    /// Bridge inventory for the first node.
    pub bridges: Vec<ProxmoxBridge>,
    /// Next available VMID.
    pub next_vmid: u64,
}

/// VM power/status response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxmoxVmStatus {
    /// Proxmox VM status, such as `running` or `stopped`.
    #[serde(default)]
    pub status: String,
    /// VM name.
    #[serde(default)]
    pub name: String,
    /// QEMU process id when running.
    #[serde(default)]
    pub pid: Option<u64>,
    /// Memory configured in bytes.
    #[serde(default)]
    pub maxmem: u64,
    /// CPU count.
    #[serde(default)]
    pub cpus: u64,
}

/// Selected Proxmox VM configuration fields used for resume validation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProxmoxVmConfig {
    /// VM display name.
    #[serde(default)]
    pub name: Option<String>,
    /// First network adapter configuration.
    #[serde(default)]
    pub net0: Option<String>,
    /// Boot order configuration.
    #[serde(default)]
    pub boot: Option<String>,
    /// First SCSI disk configuration.
    #[serde(default)]
    pub scsi0: Option<String>,
    /// QEMU guest agent setting.
    #[serde(default)]
    pub agent: Option<String>,
}

/// Request for creating a Dune Alpine VM on Proxmox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxmoxCreateVmRequest {
    /// Target node.
    pub node: String,
    /// VMID to create.
    pub vmid: u64,
    /// VM display name.
    pub vm_name: String,
    /// Storage receiving the imported VM disk.
    pub vm_storage: String,
    /// Storage used for qcow2 upload/import.
    pub import_storage: String,
    /// Linux bridge for guest networking.
    pub bridge: String,
    /// Optional explicit MAC address for DHCP discovery.
    #[serde(default)]
    pub mac_address: Option<String>,
    /// VM memory in GiB.
    pub memory_gb: u64,
    /// VM CPU core count.
    pub cores: u32,
    /// Final disk size in GiB.
    pub disk_gb: u64,
    /// Whether to enable Proxmox QEMU guest agent integration for the VM.
    #[serde(default)]
    pub qemu_guest_agent: bool,
}

impl ProxmoxCreateVmRequest {
    /// Validates required VM creation fields.
    pub fn validate(&self, qcow2_path: &Path) -> CommandResult<()> {
        if self.node.trim().is_empty() {
            return Err(failure("Proxmox node is required"));
        }
        if self.vmid == 0 {
            return Err(failure("Proxmox VMID must be greater than zero"));
        }
        if self.vm_name.trim().is_empty() {
            return Err(failure("Proxmox VM name is required"));
        }
        if self.vm_storage.trim().is_empty() {
            return Err(failure("Proxmox VM storage is required"));
        }
        if self.import_storage.trim().is_empty() {
            return Err(failure("Proxmox import storage is required"));
        }
        if self.bridge.trim().is_empty() {
            return Err(failure("Proxmox bridge is required"));
        }
        if self.memory_gb == 0 {
            return Err(failure("Proxmox VM memory must be greater than zero"));
        }
        if self.cores == 0 {
            return Err(failure("Proxmox VM cores must be greater than zero"));
        }
        if self.disk_gb == 0 {
            return Err(failure("Proxmox VM disk size must be greater than zero"));
        }
        if !qcow2_path.is_file() {
            return Err(failure(format!(
                "Converted qcow2 image was not found: {}",
                qcow2_path.display()
            )));
        }
        Ok(())
    }
}

/// Result of creating a Proxmox VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxmoxCreateVmResult {
    /// Target node.
    pub node: String,
    /// Created VMID.
    pub vmid: u64,
    /// Created VM name.
    pub vm_name: String,
    /// Uploaded import image filename.
    pub uploaded_import_name: String,
}

/// Cached conversion result for the vendor VHDX image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxmoxImageConversion {
    /// Source VHDX image.
    pub source_vhdx: PathBuf,
    /// Converted qcow2 image.
    pub qcow2_path: PathBuf,
    /// Whether conversion ran in this call.
    pub converted_now: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageCacheManifest {
    source_path: String,
    source_size: u64,
    source_modified_unix_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct ProxmoxEnvelope<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ProxmoxTaskStatus {
    #[serde(default)]
    status: String,
    #[serde(default)]
    exitstatus: Option<String>,
}

/// Returns the SHA-256 fingerprint of the current Proxmox TLS certificate.
pub fn proxmox_certificate_sha256(base_url: &Url) -> CommandResult<String> {
    let host = base_url
        .host_str()
        .ok_or_else(|| failure("Proxmox URL has no host"))?;
    let port = base_url.port_or_known_default().unwrap_or(443);
    let stream = TcpStream::connect((host, port))
        .map_err(|err| failure(format!("Failed to connect to Proxmox TLS endpoint: {err}")))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|err| failure(format!("Failed to set TLS read timeout: {err}")))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|err| failure(format!("Failed to set TLS write timeout: {err}")))?;
    let connector = TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|err| failure(format!("Failed to create TLS connector: {err}")))?;
    let tls = connector
        .connect(host, stream)
        .map_err(|err| failure(format!("Failed to read Proxmox certificate: {err}")))?;
    let certificate = tls
        .peer_certificate()
        .map_err(|err| failure(format!("Failed to read peer certificate: {err}")))?
        .ok_or_else(|| failure("Proxmox did not present a TLS certificate"))?;
    let der = certificate
        .to_der()
        .map_err(|err| failure(format!("Failed to encode peer certificate: {err}")))?;
    Ok(hex_sha256(&der))
}

/// Finds the vendor `dune-server.vhdx` image under a server package directory.
pub fn find_vendor_vhdx(server_package_dir: impl AsRef<Path>) -> CommandResult<PathBuf> {
    let root = server_package_dir.as_ref();
    let expected = root.join("Virtual Hard Disks").join("dune-server.vhdx");
    if expected.is_file() {
        return Ok(expected);
    }
    let candidates = collect_by_extension(root, "vhdx")?;
    match candidates.len() {
        0 => Err(failure(format!(
            "No .vhdx image was found under {}",
            root.display()
        ))),
        1 => Ok(candidates[0].clone()),
        _ => Err(failure(format!(
            "Multiple .vhdx images were found under {}; expected one vendor image",
            root.display()
        ))),
    }
}

/// Converts the vendor VHDX image to a cached qcow2 file when the source changed.
pub fn convert_vhdx_to_cached_qcow2(
    qemu_img: impl AsRef<Path>,
    source_vhdx: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
) -> CommandResult<ProxmoxImageConversion> {
    let qemu_img = qemu_img.as_ref();
    let source_vhdx = source_vhdx.as_ref();
    let cache_dir = cache_dir.as_ref();
    if !qemu_img.is_file() {
        return Err(failure(format!(
            "qemu-img executable was not found: {}",
            qemu_img.display()
        )));
    }
    if !source_vhdx.is_file() {
        return Err(failure(format!(
            "Source VHDX was not found: {}",
            source_vhdx.display()
        )));
    }
    fs::create_dir_all(cache_dir).map_err(|err| {
        failure(format!(
            "Failed to create Proxmox image cache {}: {err}",
            cache_dir.display()
        ))
    })?;
    let qcow2_path = cache_dir.join("dune-server.qcow2");
    let manifest_path = cache_dir.join("dune-server.qcow2.json");
    let manifest = image_cache_manifest(source_vhdx)?;
    if qcow2_path.is_file() && read_manifest(&manifest_path).as_ref() == Some(&manifest) {
        return Ok(ProxmoxImageConversion {
            source_vhdx: source_vhdx.to_path_buf(),
            qcow2_path,
            converted_now: false,
        });
    }
    let temp_path = cache_dir.join("dune-server.qcow2.tmp");
    if temp_path.exists() {
        fs::remove_file(&temp_path).map_err(|err| {
            failure(format!(
                "Failed to remove stale temporary image {}: {err}",
                temp_path.display()
            ))
        })?;
    }
    let mut command = Command::new(qemu_img);
    suppress_console_window(&mut command);
    let output = command
        .args(["convert", "-p", "-O", "qcow2"])
        .arg(source_vhdx)
        .arg(&temp_path)
        .output()
        .map_err(|err| failure(format!("Failed to run qemu-img: {err}")))?;
    if !output.status.success() {
        return Err(command_failure("qemu-img conversion failed", output));
    }
    fs::rename(&temp_path, &qcow2_path).map_err(|err| {
        failure(format!(
            "Failed to promote converted qcow2 image to {}: {err}",
            qcow2_path.display()
        ))
    })?;
    let manifest_text = serde_json::to_string_pretty(&manifest)
        .map_err(|err| failure(format!("Failed to serialize image cache manifest: {err}")))?;
    fs::write(&manifest_path, manifest_text).map_err(|err| {
        failure(format!(
            "Failed to write image cache manifest {}: {err}",
            manifest_path.display()
        ))
    })?;
    Ok(ProxmoxImageConversion {
        source_vhdx: source_vhdx.to_path_buf(),
        qcow2_path,
        converted_now: true,
    })
}

/// Builds Proxmox VM create parameters for tests and CLI inspection.
pub fn proxmox_create_vm_params(request: &ProxmoxCreateVmRequest) -> BTreeMap<String, String> {
    let mut params = BTreeMap::from([
        ("vmid".to_string(), request.vmid.to_string()),
        ("name".to_string(), request.vm_name.clone()),
        ("bios".to_string(), "ovmf".to_string()),
        ("machine".to_string(), "q35".to_string()),
        ("scsihw".to_string(), "virtio-scsi-single".to_string()),
        ("cpu".to_string(), "host".to_string()),
        ("cores".to_string(), request.cores.to_string()),
        (
            "memory".to_string(),
            request.memory_gb.saturating_mul(1024).to_string(),
        ),
        ("net0".to_string(), proxmox_net0_value(request)),
        ("ostype".to_string(), "l26".to_string()),
        ("onboot".to_string(), "1".to_string()),
    ]);
    if request.qemu_guest_agent {
        params.insert("agent".to_string(), "enabled=1".to_string());
    }
    params
}

fn proxmox_net0_value(request: &ProxmoxCreateVmRequest) -> String {
    match request
        .mac_address
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(mac) => format!("virtio={mac},bridge={}", request.bridge),
        None => format!("virtio,bridge={}", request.bridge),
    }
}

fn proxmox_import_volume_ref(storage: &str, file_name: &str) -> String {
    format!("{}:import/{}", storage.trim(), file_name.trim())
}

/// Parses neighbor/ARP output and returns the IPv4 address matching a MAC.
pub fn parse_dhcp_ip_from_arp(mac_address: &str, output: &str) -> Option<String> {
    let normalized_mac = normalize_mac(mac_address)?;
    let compact_mac = normalized_mac.replace(':', "");
    for line in output.lines() {
        let compact_line = line
            .chars()
            .filter(|character| character.is_ascii_hexdigit())
            .flat_map(char::to_lowercase)
            .collect::<String>();
        if !line.to_ascii_lowercase().contains(&normalized_mac)
            && !compact_line.contains(&compact_mac)
        {
            continue;
        }
        for token in line.split_whitespace() {
            if looks_like_ipv4(token.trim_matches(['(', ')'])) {
                return Some(token.trim_matches(['(', ')']).to_string());
            }
        }
    }
    None
}

fn image_cache_manifest(source_vhdx: &Path) -> CommandResult<ImageCacheManifest> {
    let metadata = fs::metadata(source_vhdx).map_err(|err| {
        failure(format!(
            "Failed to inspect VHDX source {}: {err}",
            source_vhdx.display()
        ))
    })?;
    let modified = metadata
        .modified()
        .map_err(|err| failure(format!("Failed to read VHDX modified time: {err}")))?
        .duration_since(UNIX_EPOCH)
        .map_err(|err| failure(format!("VHDX modified time is before Unix epoch: {err}")))?
        .as_secs();
    Ok(ImageCacheManifest {
        source_path: source_vhdx.to_string_lossy().to_string(),
        source_size: metadata.len(),
        source_modified_unix_seconds: modified,
    })
}

fn read_manifest(path: &Path) -> Option<ImageCacheManifest> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn collect_by_extension(root: &Path, extension: &str) -> CommandResult<Vec<PathBuf>> {
    let mut output = Vec::new();
    collect_by_extension_inner(root, extension, &mut output)?;
    output.sort();
    Ok(output)
}

fn collect_by_extension_inner(
    root: &Path,
    extension: &str,
    output: &mut Vec<PathBuf>,
) -> CommandResult<()> {
    for entry in fs::read_dir(root).map_err(|err| {
        failure(format!(
            "Failed to read server package directory {}: {err}",
            root.display()
        ))
    })? {
        let entry =
            entry.map_err(|err| failure(format!("Failed to read directory entry: {err}")))?;
        let path = entry.path();
        if path.is_dir() {
            collect_by_extension_inner(&path, extension, output)?;
        } else if path
            .extension()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(extension))
        {
            output.push(path);
        }
    }
    Ok(())
}

fn trim_response_text(text: &str) -> String {
    const MAX: usize = 800;
    let trimmed = text.trim();
    if trimmed.len() > MAX {
        format!("{}...", &trimmed[..MAX])
    } else {
        trimmed.to_string()
    }
}

fn parse_proxmox_u64(value: &Value, label: &str) -> CommandResult<u64> {
    if let Some(number) = value.as_u64() {
        return Ok(number);
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<u64>()
            .map_err(|_| failure(format!("Proxmox {label} was not an unsigned integer")));
    }
    Err(failure(format!(
        "Proxmox {label} had an unexpected JSON shape"
    )))
}

fn encode_path(value: &str) -> String {
    value.replace('/', "%2F")
}

fn emit(sink: &mut impl OperationSink, step_id: &'static str, message: &str, action: StepAction) {
    sink.emit(OrchestrationEvent {
        step_id,
        message: message.to_string(),
        domain: StepDomain::Proxmox,
        action,
        provider: ProviderKind::Proxmox,
    });
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fingerprints_equal(left: &str, right: &str) -> bool {
    normalize_fingerprint(left).eq_ignore_ascii_case(&normalize_fingerprint(right))
}

fn normalize_fingerprint(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalize_mac(value: &str) -> Option<String> {
    let hex = value
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    (hex.len() == 12).then(|| {
        hex.as_bytes()
            .chunks(2)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(":")
    })
}

fn looks_like_ipv4(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 4 && parts.iter().all(|part| part.parse::<u8>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proxmox_enveloped_storage_response() {
        let text = r#"{"data":[{"storage":"local-lvm","type":"lvmthin","content":"images,rootdir","active":1,"avail":1024,"total":2048}]}"#;
        let envelope: ProxmoxEnvelope<Vec<ProxmoxStorage>> = serde_json::from_str(text).unwrap();
        assert_eq!(envelope.data[0].storage, "local-lvm");
        assert!(envelope.data[0].content.contains("images"));
    }

    #[test]
    fn api_url_keeps_separator_after_json_prefix() {
        let client = ProxmoxClient::new(ProxmoxClientConfig {
            base_url: "https://pve.example.test:8006".to_string(),
            token_id: "root@pam!manager".to_string(),
            token_secret: "secret".to_string(),
            accepted_certificate_sha256: None,
        })
        .unwrap();

        assert_eq!(
            client.api_url("/version").unwrap().path(),
            "/api2/json/version"
        );
    }

    #[test]
    fn parses_next_vmid_from_number_or_string() {
        assert_eq!(
            parse_proxmox_u64(&Value::from(100), "next VMID").unwrap(),
            100
        );
        assert_eq!(
            parse_proxmox_u64(&Value::from("101"), "next VMID").unwrap(),
            101
        );
    }

    #[test]
    fn create_payload_uses_required_proxmox_vm_shape() {
        let request = ProxmoxCreateVmRequest {
            node: "pve".to_string(),
            vmid: 120,
            vm_name: "dune-proxmox".to_string(),
            vm_storage: "local-lvm".to_string(),
            import_storage: "local".to_string(),
            bridge: "vmbr0".to_string(),
            mac_address: Some("bc:24:11:22:33:44".to_string()),
            memory_gb: 20,
            cores: 4,
            disk_gb: 100,
            qemu_guest_agent: true,
        };
        let params = proxmox_create_vm_params(&request);
        assert_eq!(params["bios"], "ovmf");
        assert_eq!(params["machine"], "q35");
        assert_eq!(params["scsihw"], "virtio-scsi-single");
        assert_eq!(params["cpu"], "host");
        assert_eq!(params["net0"], "virtio=bc:24:11:22:33:44,bridge=vmbr0");
        assert_eq!(params["memory"], "20480");
        assert_eq!(params["agent"], "enabled=1");
    }

    #[test]
    fn cache_manifest_changes_when_source_changes() {
        let unique = std::process::id();
        let root = std::env::temp_dir().join(format!("dune-proxmox-cache-test-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("dune-server.vhdx");
        fs::write(&source, b"one").unwrap();
        let first = image_cache_manifest(&source).unwrap();
        std::thread::sleep(Duration::from_secs(1));
        fs::write(&source, b"two-two").unwrap();
        let second = image_cache_manifest(&source).unwrap();
        let _ = fs::remove_dir_all(&root);
        assert_ne!(first.source_size, second.source_size);
        assert_ne!(first, second);
    }

    #[test]
    fn parses_dhcp_ip_from_linux_neighbor_output() {
        let output = "10.77.2.201 dev vmbr0 lladdr bc:24:11:22:33:44 REACHABLE\n";
        assert_eq!(
            parse_dhcp_ip_from_arp("BC-24-11-22-33-44", output),
            Some("10.77.2.201".to_string())
        );
    }

    #[test]
    fn parses_dhcp_ip_from_windows_arp_output() {
        let output = "  10.77.2.201          bc-24-11-22-33-44     dynamic";
        assert_eq!(
            parse_dhcp_ip_from_arp("bc:24:11:22:33:44", output),
            Some("10.77.2.201".to_string())
        );
    }
}
