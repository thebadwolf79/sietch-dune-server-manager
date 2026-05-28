use std::path::PathBuf;
use std::sync::Arc;

use chrono_tz::Tz;

use crate::admin::MqPublisher;
use crate::kubectl::{BattlegroupCli, ClusterCache, KubectlClient, SteamCmd};
use crate::postgres::PgClient;

pub mod backup;
pub mod restart;
pub mod restart_notice;
pub mod update_apply;
pub mod update_check;
pub mod welcome_package;

/// Heavy-weight resources shared by all scheduled tasks. Constructed once in
/// `main.rs` from [`crate::config::ServiceConfig`] and dropped into the
/// scheduler so each `Task::run` call can borrow what it needs.
pub struct TaskEnv {
    pub kubectl: KubectlClient,
    pub cluster: ClusterCache,
    pub bg_cli: BattlegroupCli,
    pub steamcmd: SteamCmd,
    pub mq: Arc<MqPublisher>,
    pub pg: Arc<PgClient>,
    pub bin_dir: PathBuf,
    pub download_path: PathBuf,
    /// Lead time before a downloaded update is applied (default 1800s = 30 min).
    pub update_lead_secs: i64,
    /// Restart-notice + restart wall-clock target (default 05:00).
    pub restart_hour: u32,
    pub restart_minute: u32,
    /// Restart broadcast frequency / declared shutdown duration.
    pub restart_warning_frequency_secs: u64,
    pub restart_warning_duration_secs: u64,
    pub restart_tz: Tz,
    /// Operator-supplied cron expression that drives automatic battlegroup
    /// backups, evaluated in `restart_tz`. `None` disables the scheduled
    /// backup loop; manual triggers via `/api/tasks/trigger` still run.
    /// Defaults to None — operators opt in to a cadence that suits their
    /// player traffic, since vendor backups stall server I/O.
    pub backup_cron: Option<cron::Schedule>,
    /// User-typed form of the cron expression, kept verbatim so the UI
    /// echoes exactly what the operator entered.
    pub backup_cron_raw: Option<String>,
    /// Enables the opt-in new-player welcome-package worker.
    pub welcome_package_enabled: bool,
    /// Enables the welcome whisper worker independently from item/package
    /// grants.
    pub welcome_message_enabled: bool,
    /// When enabled, package actions wait until the player's backpack is
    /// empty. Welcome whispers are not gated by this.
    pub welcome_package_require_empty_backpack: bool,
    /// Operator-controlled package version. Changing it grants the package
    /// again because the ledger key is `(player_id, package_version)`.
    pub welcome_package_version: String,
    /// Poll cadence for the welcome-package worker.
    pub welcome_package_poll_secs: u64,
    /// How long a pending player must remain Online before the worker grants.
    /// This avoids publishing during the login/loading handoff where MQ
    /// accepts the message but no active player controller applies it.
    pub welcome_package_online_grace_secs: u64,
    /// JSON config for welcome-package actions. Kept as parsed data in the env
    /// so scheduled fires don't re-parse sqlite state.
    pub welcome_package_actions: Vec<welcome_package::WelcomePackageAction>,
    /// Verbatim JSON string for UI echo/restart-required checks.
    pub welcome_package_actions_json: String,
    /// Player lookup used as the visible sender for welcome whispers. Empty
    /// falls back to the recipient for self-sourced whispers.
    pub welcome_whisper_source_player: String,
    /// Welcome whisper text used by the automated action and manual send.
    pub welcome_message: String,
}

/// All task implementations registered for the scheduler.
pub fn build_all(env: Arc<TaskEnv>) -> Vec<Arc<dyn crate::scheduler::Task>> {
    vec![
        Arc::new(backup::BackupTask::new(env.clone())) as Arc<dyn crate::scheduler::Task>,
        Arc::new(update_check::UpdateCheckTask),
        Arc::new(update_apply::UpdateApplyTask),
        Arc::new(restart_notice::RestartNoticeTask::new(env.clone())),
        Arc::new(restart::RestartTask::new(env.clone())),
        Arc::new(welcome_package::WelcomePackageTask::new(env)),
    ]
}
