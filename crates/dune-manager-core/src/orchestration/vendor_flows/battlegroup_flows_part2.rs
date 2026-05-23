use super::battlegroup_flows::battlegroup_kubernetes_step;
use super::flow_models::{
    step, BattlegroupCommand, BattlegroupCommandSpec, StepAction, StepDomain, StepFlags,
};

pub(super) fn core_command_specs() -> Vec<BattlegroupCommandSpec> {
    vec![
        BattlegroupCommandSpec::new(
            BattlegroupCommand::List,
            "list",
            "Lists all available battlegroups",
            vec![battlegroup_kubernetes_step(
                "bg.command.list",
                "List battlegroups",
                StepAction::Detect,
                "kubectl get battlegroups -A -o json",
                "StructuredBattlegroupOps::list",
            )],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::Status,
            "status",
            "Shows the status of the selected battlegroup",
            vec![battlegroup_kubernetes_step(
                "bg.command.status",
                "Read battlegroup status snapshot",
                StepAction::Detect,
                "kubectl get battlegroup/pods/services -o json",
                "StructuredBattlegroupOps::status",
            )],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::Start,
            "start",
            "Starts the selected battlegroup",
            vec![
                battlegroup_kubernetes_step(
                    "bg.command.start",
                    "Start battlegroup",
                    StepAction::Start,
                    "kubectl patch battlegroup spec.stop=false",
                    "BattlegroupManagementOrchestrator::start_and_wait_director",
                ),
                step(
                    "bg.director.wait-port-after-start",
                    "Wait for Director NodePort",
                    StepDomain::Kubernetes,
                    StepAction::Wait,
                    "kubectl get svc port 11717 nodePort",
                    "Kubernetes service discovery",
                    StepFlags::new(false, false),
                ),
            ],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::Restart,
            "restart",
            "Restarts the selected battlegroup",
            vec![
                battlegroup_kubernetes_step(
                    "bg.command.restart.stop",
                    "Stop battlegroup for restart",
                    StepAction::Stop,
                    "kubectl patch battlegroup spec.stop=true",
                    "BattlegroupManagementOrchestrator::restart_and_wait_director",
                ),
                battlegroup_kubernetes_step(
                    "bg.command.restart.start",
                    "Start battlegroup after restart",
                    StepAction::Start,
                    "kubectl patch battlegroup spec.stop=false",
                    "BattlegroupManagementOrchestrator::restart_and_wait_director",
                ),
                step(
                    "bg.director.wait-port-after-restart",
                    "Wait for Director NodePort",
                    StepDomain::Kubernetes,
                    StepAction::Wait,
                    "kubectl get svc port 11717 nodePort",
                    "Kubernetes service discovery",
                    StepFlags::new(false, false),
                ),
            ],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::Stop,
            "stop",
            "Stops the selected battlegroup",
            vec![battlegroup_kubernetes_step(
                "bg.command.stop",
                "Stop battlegroup",
                StepAction::Stop,
                "kubectl patch battlegroup spec.stop=true",
                "BattlegroupManagementOrchestrator::stop",
            )],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::Update,
            "update",
            "Checks for new versions and applies them",
            vec![
                step(
                    "bg.command.update.import-images",
                    "Import downloaded battlegroup images",
                    StepDomain::Guest,
                    StepAction::Import,
                    "ctr -n k8s.io images import downloaded battlegroup tars",
                    "BattlegroupUpdateOrchestrator::update_from_downloads",
                    StepFlags::new(false, false),
                ),
                battlegroup_kubernetes_step(
                    "bg.command.update.patch-images",
                    "Patch battlegroup image revisions",
                    StepAction::Patch,
                    "kubectl patch battlegroup image tags",
                    "BattlegroupUpdateOrchestrator::update_from_downloads",
                ),
            ],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::EditBattlegroup,
            "edit-battlegroup",
            "Edit settings of the battlegroup",
            vec![
                step(
                    "bg.edit.discover-namespace",
                    "Discover battlegroup namespace",
                    StepDomain::Kubernetes,
                    StepAction::Detect,
                    "kubectl get ns grep funcom-seabass",
                    "Kubernetes namespace list",
                    StepFlags::new(false, false),
                ),
                step(
                    "bg.edit.region",
                    "Patch region settings",
                    StepDomain::Kubernetes,
                    StepAction::Patch,
                    "kubectl get battlegroup -o json; Rust JSON patch; kubectl patch",
                    "StructuredBattlegroupOps::patch_region",
                    StepFlags::new(false, false),
                ),
            ],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::EditBattlegroupAdvanced,
            "edit-battlegroup-advanced",
            "Manually edit the live battlegroup YAML",
            vec![step(
                "bg.edit-advanced.open",
                "Open advanced battlegroup YAML editor",
                StepDomain::Kubernetes,
                StepAction::Shell,
                "kubectl edit battlegroup",
                "Future guarded native YAML/diff editor; currently vendor capability metadata",
                StepFlags::new(false, true),
            )],
        ),
        BattlegroupCommandSpec::new(
            BattlegroupCommand::EnableExperimentalSwap,
            "enable-experimental-swap",
            "Enable experimental swap memory profile",
            vec![step(
                "bg.swap.enable",
                "Enable guest swap and patch BattleGroup memory",
                StepDomain::Guest,
                StepAction::Configure,
                "/home/dune/.dune/bin/battlegroup enable-experimental-swap",
                "ExperimentalSwapOrchestrator::enable",
                StepFlags::new(false, true),
            )],
        ),
    ]
}
