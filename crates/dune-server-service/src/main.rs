use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result};

use dune_server_service::admin::MqPublisher;
use dune_server_service::config::{resolve_command_auth_token, ServiceConfig};
use dune_server_service::http::{self, AppState};
use dune_server_service::kubectl::{BattlegroupCli, ClusterCache, KubectlClient, SteamCmd};
use dune_server_service::logger;
use dune_server_service::postgres::{PgClient, PgConfig};
use dune_server_service::scheduler::{Scheduler, TaskRunner};
use dune_server_service::store::Store;
use dune_server_service::systemd_compat;
use dune_server_service::tasks::TaskEnv;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PATH_EXTRAS: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

fn main() -> ExitCode {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" | "version" => {
                println!("dune-server-service {VERSION}");
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!(
                    "dune-server-service {VERSION}\n\
                     usage: dune-server-service [--version] [--help]\n\
                     With no flags, runs the daemon (see env vars + systemd unit)."
                );
                return ExitCode::SUCCESS;
            }
            _ => {}
        }
    }

    // SAFETY: set_var requires no other threads to be running. We are still
    // single-threaded here (before the tokio runtime is built below). Inject a
    // sane PATH that covers common kubectl / battlegroup / steamcmd locations
    // so the daemon's subprocesses don't depend on the init system's PATH.
    unsafe {
        let merged = match std::env::var_os("PATH") {
            Some(existing) if !existing.is_empty() => {
                let mut v = std::ffi::OsString::from(DEFAULT_PATH_EXTRAS);
                v.push(":");
                v.push(existing);
                v
            }
            _ => DEFAULT_PATH_EXTRAS.into(),
        };
        std::env::set_var("PATH", merged);
    }

    logger::init();

    match systemd_compat::repair_on_startup_if_needed() {
        Ok(true) => {
            tracing::warn!(
                "systemd sandbox blocked steamcmd text relocations; compatibility override installed, exiting for restart"
            );
            return ExitCode::SUCCESS;
        }
        Ok(false) => {}
        Err(err) => {
            tracing::error!(
                error = %err,
                "failed to verify systemd steamcmd compatibility; steam update checks may fail"
            );
        }
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(err) => {
            tracing::error!(error = %err, "failed to build tokio runtime");
            return ExitCode::FAILURE;
        }
    };

    runtime.block_on(async {
        match run().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                tracing::error!(error = %err, "dune-server-service exiting with error");
                ExitCode::FAILURE
            }
        }
    })
}

async fn run() -> Result<()> {
    let cfg = ServiceConfig::from_env().context("loading config")?;
    let token = resolve_command_auth_token(&cfg.command_auth_token_file);
    logger::register_token(&token);

    tracing::info!(
        version = VERSION,
        bind = %format!("{}:{}", cfg.dashboard_host, cfg.dashboard_port),
        db_path = %cfg.db_path.display(),
        time_zone = %cfg.time_zone,
        "dune-server-service starting"
    );

    let store = Store::open(&cfg.db_path).context("opening sqlite store")?;

    let kubectl = KubectlClient::new(
        cfg.kubectl_use_sudo,
        cfg.namespace_override.clone(),
        cfg.mq_pod_override.clone(),
        cfg.db_pod_override.clone(),
    );
    let cluster = ClusterCache::new(kubectl.clone());

    let bg_cli = BattlegroupCli::new(&cfg.bin_dir);
    let download_path = cfg
        .steamcmd_download_path
        .clone()
        .unwrap_or_else(|| cfg.service_home.join(".dune").join("download"));
    let steamcmd_bin = cfg
        .steamcmd_path
        .clone()
        .unwrap_or_else(|| cfg.service_home.join(".local").join("bin").join("steamcmd"));
    let steamcmd = SteamCmd::new(
        steamcmd_bin,
        download_path.clone(),
        cfg.service_home.clone(),
    );
    if let Err(err) = steamcmd.ensure_wrapper() {
        tracing::warn!(error = %err, "could not ensure steamcmd wrapper; update-check will fail until resolved");
    }

    let mq = Arc::new(MqPublisher::new(
        kubectl.clone(),
        cluster.clone(),
        token.clone(),
    ));
    let pg = Arc::new(PgClient::new(
        kubectl.clone(),
        PgConfig {
            host_override: cfg.pg_host_override.clone(),
            user_override: cfg.pg_user_override.clone(),
            db_override: cfg.pg_db_override.clone(),
        },
    ));

    // Defaults; operator can override any of these via POST /api/config which
    // upserts into the `task_config` KV table. We apply them at startup only —
    // a change to /api/config requires a service restart to take effect.
    let mut update_lead_secs: i64 = 30 * 60;
    let mut restart_hour: u32 = 5;
    let mut restart_minute: u32 = 0;
    let mut restart_warning_frequency_secs: u64 = 600;
    let mut restart_warning_duration_secs: u64 = 1800;
    let mut welcome_package_enabled = false;
    let mut welcome_message_enabled = false;
    let welcome_package_version = String::from("v1");
    let mut welcome_package_actions_json = String::from("[]");
    let mut welcome_whisper_source_player = String::new();
    let mut welcome_message = String::new();
    // Backups default to OFF. Operator opts in by POSTing /api/config with a
    // cron expression in `backupCron`.
    let mut backup_cron: Option<cron::Schedule> = None;
    let mut backup_cron_raw: Option<String> = None;
    if let Ok(Some(v)) = store.get_config_i64("update_lead_secs") {
        update_lead_secs = v;
    }
    if let Ok(Some(v)) = store.get_config_i64("restart_hour") {
        restart_hour = v as u32;
    }
    if let Ok(Some(v)) = store.get_config_i64("restart_minute") {
        restart_minute = v as u32;
    }
    if let Ok(Some(v)) = store.get_config_i64("restart_warning_frequency_secs") {
        restart_warning_frequency_secs = v as u64;
    }
    if let Ok(Some(v)) = store.get_config_i64("restart_warning_duration_secs") {
        restart_warning_duration_secs = v as u64;
    }
    if let Ok(Some(v)) = store.get_config_i64("welcome_package_enabled") {
        welcome_package_enabled = v != 0;
    }
    if let Ok(Some(v)) = store.get_config_i64("welcome_message_enabled") {
        welcome_message_enabled = v != 0;
    }
    if let Ok(Some(v)) = store.get_config("welcome_package_actions_json") {
        welcome_package_actions_json = v;
    } else if let Ok(Some(v)) = store.get_config("welcome_package_items_json") {
        welcome_package_actions_json = v;
    }
    if let Ok(Some(v)) = store.get_config("welcome_whisper_source_player") {
        welcome_whisper_source_player = v;
    }
    if let Ok(Some(v)) = store.get_config("welcome_message") {
        welcome_message = v;
    }
    if let Ok(Some(expr)) = store.get_config("backup_cron") {
        let trimmed = expr.trim();
        if !trimmed.is_empty() {
            match dune_server_service::scheduler::schedule::parse_cron(trimmed) {
                Ok(schedule) => {
                    backup_cron = Some(schedule);
                    backup_cron_raw = Some(trimmed.to_string());
                }
                Err(err) => {
                    tracing::warn!(stored = %trimmed, error = %err, "ignoring invalid stored backup_cron");
                }
            }
        }
    }
    let welcome_package_actions =
        match dune_server_service::tasks::welcome_package::parse_welcome_actions(
            &welcome_package_actions_json,
        ) {
            Ok(actions) => actions,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "ignoring invalid welcome_package_actions_json; welcome package disabled for this daemon run"
                );
                welcome_package_enabled = false;
                Vec::new()
            }
        };
    let mut effective_tz = cfg.time_zone;
    if let Ok(Some(tz_name)) = store.get_config("restart_tz") {
        match tz_name.parse::<chrono_tz::Tz>() {
            Ok(tz) => effective_tz = tz,
            Err(err) => {
                tracing::warn!(stored = %tz_name, error = %err, "ignoring invalid stored restart_tz, falling back to env");
            }
        }
    }
    tracing::info!(
        update_lead_secs,
        restart_hour,
        restart_minute,
        restart_warning_frequency_secs,
        restart_warning_duration_secs,
        backup_cron = backup_cron_raw.as_deref().unwrap_or("(disabled)"),
        welcome_package_enabled,
        welcome_message_enabled,
        welcome_package_version = %welcome_package_version,
        welcome_package_scan_secs = 2_u64,
        welcome_message_scan_secs = 60_u64,
        welcome_package_actions = welcome_package_actions.len(),
        welcome_whisper_source_player = %welcome_whisper_source_player,
        welcome_message_configured = !welcome_message.trim().is_empty(),
        tz = %effective_tz.name(),
        "task schedule resolved"
    );

    let env = Arc::new(TaskEnv {
        kubectl: kubectl.clone(),
        cluster: cluster.clone(),
        bg_cli,
        steamcmd,
        mq,
        pg,
        bin_dir: cfg.bin_dir.clone(),
        download_path,
        update_lead_secs,
        restart_hour,
        restart_minute,
        restart_warning_frequency_secs,
        restart_warning_duration_secs,
        restart_tz: effective_tz,
        backup_cron,
        backup_cron_raw,
        welcome_package_enabled,
        welcome_message_enabled,
        welcome_package_version,
        welcome_package_actions,
        welcome_package_actions_json,
        welcome_whisper_source_player,
        welcome_message,
    });

    let runner = Arc::new(TaskRunner::new(store.clone(), env.clone()));
    let mut scheduler = Scheduler::new(runner.clone(), effective_tz);
    for task in dune_server_service::tasks::build_all(env.clone()) {
        scheduler.add(task);
    }
    let cancel = scheduler.cancel_token();
    scheduler.start();

    let state = AppState::new(store, env, runner);
    let server_cancel = cancel.clone();

    let http_handle = tokio::spawn(async move {
        if let Err(err) = http::serve(&cfg, state, server_cancel).await {
            tracing::error!(error = %err, "http server exited with error");
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("ctrl-c received; shutting down");
        }
        _ = wait_sigterm() => {
            tracing::info!("SIGTERM received; shutting down");
        }
    }

    cancel.cancel();
    scheduler.shutdown().await;
    let _ = http_handle.await;
    tracing::info!("dune-server-service stopped");
    Ok(())
}

#[cfg(unix)]
async fn wait_sigterm() {
    use tokio::signal::unix::{signal, SignalKind};
    if let Ok(mut sig) = signal(SignalKind::terminate()) {
        sig.recv().await;
    } else {
        std::future::pending::<()>().await;
    }
}

#[cfg(not(unix))]
async fn wait_sigterm() {
    std::future::pending::<()>().await;
}
