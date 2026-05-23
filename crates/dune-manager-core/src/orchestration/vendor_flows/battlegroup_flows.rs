use super::flow_models::{
    step, BattlegroupCommandSpec, FlowSpec, FlowStep, ProviderKind, StepAction, StepDomain,
    StepFlags,
};

/// Returns the native replacement map for the vendor battlegroup menu shell.
pub fn battlegroup_management_flow() -> FlowSpec {
    FlowSpec {
        id: "vendor.battlegroup.hyperv",
        title: "Battlegroup management",
        provider: ProviderKind::HyperV,
        source_scripts: &[
            "battlegroup.bat",
            "internal-scripts/battlegroup.ps1",
            "battlegroup-management/battlegroup.ps1",
            "download/scripts/battlegroup.sh",
        ],
        steps: vec![
            step(
                "bg.host.require-admin",
                "Require elevated host privileges",
                StepDomain::Host,
                StepAction::Check,
                "battlegroup.ps1 #Requires -RunAsAdministrator",
                "Split into admin VM operations and non-admin guest operations",
                StepFlags::new(true, false),
            ),
            step(
                "bg.hyperv.get-vm",
                "Load vendor VM",
                StepDomain::HyperV,
                StepAction::Detect,
                "Get-VM -Name <vendor-vm-name>",
                "Hyper-V provider get_vm",
                StepFlags::new(true, false),
            ),
            step(
                "bg.ssh.prepare-key",
                "Prepare active or bootstrap SSH key",
                StepDomain::Ssh,
                StepAction::Configure,
                "%LOCALAPPDATA% active key, falling back to bundled bootstrap key",
                "Rust key candidate manager with Windows ACL helper",
                StepFlags::new(false, false),
            ),
            step(
                "bg.hyperv.get-ip-if-running",
                "Read VM IPv4 when running",
                StepDomain::HyperV,
                StepAction::Detect,
                "Get-VMNetworkAdapter IPAddresses",
                "Hyper-V provider vm_ipv4",
                StepFlags::new(true, true),
            ),
            step(
                "bg.menu.dispatch",
                "Dispatch selected command",
                StepDomain::Interactive,
                StepAction::Choose,
                "Read-Host menu",
                "Typed command enum",
                StepFlags::new(false, false),
            ),
        ],
    }
}

pub(super) fn battlegroup_kubernetes_step(
    id: &'static str,
    description: &'static str,
    action: StepAction,
    source: &'static str,
    native_strategy: &'static str,
) -> FlowStep {
    let mut flow_step = step(
        id,
        description,
        StepDomain::Kubernetes,
        action,
        source,
        native_strategy,
        StepFlags::new(false, false),
    );
    flow_step.provider = ProviderKind::Kubernetes;
    flow_step
}

/// Returns the catalog of supported battlegroup management commands.
pub fn battlegroup_command_catalog() -> Vec<BattlegroupCommandSpec> {
    let mut catalog = super::battlegroup_flows_part2::core_command_specs();
    catalog.extend(super::battlegroup_flows_part3::extended_command_specs());
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battlegroup_catalog_matches_vendor_menu_names() {
        let names = battlegroup_command_catalog()
            .into_iter()
            .map(|command| command.menu_name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "list",
                "status",
                "start",
                "restart",
                "stop",
                "update",
                "edit-battlegroup",
                "edit-battlegroup-advanced",
                "enable-experimental-swap",
                "backup",
                "import",
                "logs-export",
                "operator-logs-export",
                "open-file-browser",
                "open-director",
                "shell-vm",
                "shell-pod",
                "start-vm",
                "stop-vm",
                "quit",
            ]
        );
    }

    #[test]
    fn battlegroup_catalog_points_to_native_replacements() {
        let catalog = battlegroup_command_catalog();
        let text = catalog
            .iter()
            .flat_map(|command| command.steps.iter())
            .map(|step| format!("{} {}", step.source, step.native_strategy))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!text.contains("/home/dune/.dune/bin/battlegroup command"));
        assert!(!text.contains("until replaced"));
        assert!(!text.contains("sed | replace"));
        for required in [
            "StructuredBattlegroupOps::list",
            "StructuredBattlegroupOps::status",
            "BattlegroupManagementOrchestrator::start_and_wait_director",
            "BattlegroupUpdateOrchestrator::update_from_downloads",
            "StructuredBattlegroupOps::patch_region",
            "StructuredBattlegroupOps::export_namespace_logs",
            "StructuredBattlegroupOps::export_operator_logs",
            "Hyper-V provider start_vm",
            "Hyper-V provider stop_vm",
        ] {
            assert!(
                text.contains(required),
                "missing native strategy {required}"
            );
        }
    }
}
