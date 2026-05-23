use std::{
    net::{TcpListener, TcpStream},
    path::PathBuf,
};

use dune_manager_core::orchestration::{OpenSshRunner, OpenSshTarget, RemoteCommandRunner};
use dune_manager_core::toolchain::{ManagedTool, Toolchain};

use crate::commands::shared::{command_error_message, sh_single_quoted};
use crate::dto::ServerTunnelStartRequest;

pub fn tunnel_target(request: &ServerTunnelStartRequest) -> Result<OpenSshTarget, String> {
    let toolchain = Toolchain::from_default_root().map_err(|err| err.message)?;
    toolchain
        .install(ManagedTool::OpenSsh, false, None)
        .map_err(|err| err.message)?;
    let ssh_path = toolchain.status(ManagedTool::OpenSsh).executable;
    match request.server_kind.trim() {
        "ubuntu" => Ok(OpenSshTarget::new(
            ssh_path,
            PathBuf::from(
                request
                    .key_path
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            ),
            request.user.as_deref().unwrap_or("root").trim().to_string(),
            request.host.trim().to_string(),
        )),
        other => Err(format!("Unsupported remote server kind: {other}")),
    }
}

pub fn normalize_tunnel_service(service: &str) -> Result<String, String> {
    match service.trim() {
        "director" => Ok("director".to_string()),
        "fileBrowser" => Ok("fileBrowser".to_string()),
        "database" => Ok("database".to_string()),
        "pgHero" => Ok("pgHero".to_string()),
        other => Err(format!("Unsupported tunnel service: {other}")),
    }
}

pub fn tunnel_url(service: &str, local_port: u16) -> String {
    if service == "database" {
        format!("postgresql://127.0.0.1:{local_port}/dune")
    } else {
        format!("http://127.0.0.1:{local_port}/")
    }
}

pub fn discover_director_tunnel_port(
    target: &OpenSshTarget,
    namespace: &str,
) -> Result<u16, String> {
    let namespace = namespace.trim();
    if namespace.is_empty() {
        return Err(
            "BattleGroup namespace is required before starting the Director tunnel.".to_string(),
        );
    }
    let runner = OpenSshRunner::new(target.clone());
    let value = runner
        .run_json(
            &format!(
                "sudo kubectl get svc -n {} -o json",
                sh_single_quoted(namespace)
            ),
            "director service list",
        )
        .map_err(command_error_message)?;
    for service in value["items"].as_array().cloned().unwrap_or_default() {
        for port in service["spec"]["ports"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            if port["port"].as_u64() == Some(11717) {
                if let Some(node_port) = port["nodePort"]
                    .as_u64()
                    .and_then(|value| u16::try_from(value).ok())
                {
                    return Ok(node_port);
                }
            }
        }
    }
    Err("Director service is not currently exposed in Kubernetes.".to_string())
}

pub fn discover_database_tunnel_port(
    target: &OpenSshTarget,
    namespace: &str,
) -> Result<u16, String> {
    const DEFAULT_DATABASE_PORT: u16 = dune_manager_core::database::DEFAULT_DUNE_DATABASE_PORT;

    let namespace = namespace.trim();
    if namespace.is_empty() {
        return Err(
            "BattleGroup namespace is required before starting the database tunnel.".to_string(),
        );
    }
    let runner = OpenSshRunner::new(target.clone());
    let value = runner
        .run_json(
            &format!(
                "sudo kubectl get databasedeployments -n {} -o json",
                sh_single_quoted(namespace)
            ),
            "database deployment list",
        )
        .map_err(command_error_message)?;
    for deployment in value["items"].as_array().cloned().unwrap_or_default() {
        if let Some(port) = deployment["spec"]["port"]
            .as_u64()
            .and_then(|value| u16::try_from(value).ok())
        {
            return Ok(port);
        }
    }
    Ok(DEFAULT_DATABASE_PORT)
}

pub fn discover_pg_hero_tunnel_port(
    target: &OpenSshTarget,
    namespace: &str,
) -> Result<u16, String> {
    const DEFAULT_PG_HERO_PORT: u16 = 21111;

    let namespace = namespace.trim();
    if namespace.is_empty() {
        return Err(
            "BattleGroup namespace is required before starting the PgHero tunnel.".to_string(),
        );
    }
    let runner = OpenSshRunner::new(target.clone());
    let value = runner
        .run_json(
            &format!(
                "sudo kubectl get pods -n {} -l role=igw-database-pghero -o json",
                sh_single_quoted(namespace)
            ),
            "PgHero pod list",
        )
        .map_err(command_error_message)?;
    for pod in value["items"].as_array().cloned().unwrap_or_default() {
        for container in pod["spec"]["containers"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            for env in container["env"].as_array().cloned().unwrap_or_default() {
                if env["name"].as_str() == Some("PORT") {
                    if let Some(port) = env["value"]
                        .as_str()
                        .and_then(|value| value.parse::<u16>().ok())
                    {
                        return Ok(port);
                    }
                }
            }
        }
    }
    Ok(DEFAULT_PG_HERO_PORT)
}

pub fn pick_available_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|err| format!("Failed to reserve a local tunnel port: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("Failed to read local tunnel port: {err}"))?
        .port();
    drop(listener);
    Ok(port)
}

pub fn is_local_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn wait_for_local_tunnel(port: u16, timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}
