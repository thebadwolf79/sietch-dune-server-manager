use serde::Serialize;

/// Execution provider family used by a documented vendor-flow step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    /// Windows Hyper-V host provider.
    HyperV,
    /// OpenSSH guest provider.
    Ssh,
    /// Container runtime provider.
    Docker,
    /// Kubernetes API provider.
    Kubernetes,
}

/// Functional domain for a setup or management step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepDomain {
    /// Windows host checks and filesystem operations.
    Host,
    /// Hyper-V VM and switch operations.
    HyperV,
    /// SteamCMD package installation or update.
    Steam,
    /// SSH connectivity and transfer work.
    Ssh,
    /// Guest OS configuration.
    Guest,
    /// Kubernetes resource operations.
    Kubernetes,
    /// Filesystem reads, writes, and template rendering.
    Files,
    /// Browser-opened URLs.
    Browser,
    /// User choice or terminal interaction.
    Interactive,
}

/// Action category for a flow step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepAction {
    /// Verify a prerequisite.
    Check,
    /// Discover existing state.
    Detect,
    /// Ask the caller to select an option.
    Choose,
    /// Create a new resource.
    Create,
    /// Import packaged data or resources.
    Import,
    /// Configure an existing resource.
    Configure,
    /// Start a service, VM, or workload.
    Start,
    /// Stop a service, VM, or workload.
    Stop,
    /// Restart a service or workload.
    Restart,
    /// Wait for readiness.
    Wait,
    /// Upload local data to a remote target.
    Upload,
    /// Download package data.
    Download,
    /// Patch a live resource.
    Patch,
    /// Export data to the caller.
    Export,
    /// Open a URL or shell.
    Open,
    /// Start an interactive shell.
    Shell,
    /// Finish a flow.
    Complete,
}

/// One documented step from a vendor flow and its native replacement strategy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowStep {
    /// Stable step id.
    pub id: &'static str,
    /// Human-readable step label.
    pub label: &'static str,
    /// Step domain.
    pub domain: StepDomain,
    /// Step action category.
    pub action: StepAction,
    /// Provider boundary responsible for the native implementation.
    pub provider: ProviderKind,
    /// Vendor script or behavior this step replaces.
    pub source: &'static str,
    /// Native library strategy for the replacement.
    pub native_strategy: &'static str,
    /// Whether this step requires elevated host privileges.
    pub requires_admin: bool,
    /// Whether this step is optional or conditional.
    pub optional: bool,
}

/// Documented end-to-end vendor flow with native replacement steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSpec {
    /// Stable flow id.
    pub id: &'static str,
    /// Human-readable flow title.
    pub title: &'static str,
    /// Primary provider family.
    pub provider: ProviderKind,
    /// Vendor scripts used as source references.
    pub source_scripts: &'static [&'static str],
    /// Ordered replacement steps.
    pub steps: Vec<FlowStep>,
}

/// Vendor battlegroup menu command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BattlegroupCommand {
    /// List all battlegroups.
    List,
    /// Show status for a battlegroup.
    Status,
    /// Start a battlegroup.
    Start,
    /// Restart a battlegroup.
    Restart,
    /// Stop a battlegroup.
    Stop,
    /// Apply a downloaded update.
    Update,
    /// Edit battlegroup settings.
    EditBattlegroup,
    /// Advanced manual battlegroup YAML editing.
    EditBattlegroupAdvanced,
    /// Enable vendor experimental swap profile.
    EnableExperimentalSwap,
    /// Back up the battlegroup database.
    BackupDatabase,
    /// Import a battlegroup database backup.
    ImportDatabase,
    /// Export battlegroup logs.
    LogsExport,
    /// Export operator logs.
    OperatorLogsExport,
    /// Open the file browser service.
    OpenFileBrowser,
    /// Open the Director service.
    OpenDirector,
    /// Open an SSH shell on the VM.
    ShellVm,
    /// Open a shell inside a pod.
    ShellPod,
    /// Start the VM.
    StartVm,
    /// Stop the VM.
    StopVm,
    /// Exit the menu.
    Quit,
}

/// Native replacement plan for a vendor battlegroup command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BattlegroupCommandSpec {
    /// Command enum value.
    pub command: BattlegroupCommand,
    /// Vendor menu name.
    pub menu_name: &'static str,
    /// Human-readable command description.
    pub description: &'static str,
    /// Steps used to implement the command natively.
    pub steps: Vec<FlowStep>,
}

impl BattlegroupCommandSpec {
    pub(super) fn new(
        command: BattlegroupCommand,
        menu_name: &'static str,
        description: &'static str,
        steps: Vec<FlowStep>,
    ) -> Self {
        Self {
            command,
            menu_name,
            description,
            steps,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StepFlags {
    pub(super) requires_admin: bool,
    pub(super) optional: bool,
}

impl StepFlags {
    pub(super) const fn new(requires_admin: bool, optional: bool) -> Self {
        Self {
            requires_admin,
            optional,
        }
    }
}

pub(super) fn step(
    id: &'static str,
    label: &'static str,
    domain: StepDomain,
    action: StepAction,
    source: &'static str,
    native_strategy: &'static str,
    flags: StepFlags,
) -> FlowStep {
    FlowStep {
        id,
        label,
        domain,
        action,
        provider: ProviderKind::HyperV,
        source,
        native_strategy,
        requires_admin: flags.requires_admin,
        optional: flags.optional,
    }
}
