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
}

/// All task implementations registered for the scheduler.
pub fn build_all(env: Arc<TaskEnv>) -> Vec<Arc<dyn crate::scheduler::Task>> {
    vec![
        Arc::new(backup::BackupTask) as Arc<dyn crate::scheduler::Task>,
        Arc::new(update_check::UpdateCheckTask),
        Arc::new(update_apply::UpdateApplyTask),
        Arc::new(restart_notice::RestartNoticeTask::new(env.clone())),
        Arc::new(restart::RestartTask::new(env)),
    ]
}
