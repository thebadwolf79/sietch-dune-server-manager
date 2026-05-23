use super::flow_models::{FlowSpec, ProviderKind};
use super::hyperv_initial_setup_flow_part2::host_and_vm_setup_steps;
use super::hyperv_initial_setup_flow_part3::guest_bootstrap_steps;

/// Returns the native replacement map for the vendor initial setup scripts.
pub fn hyperv_initial_setup_flow() -> FlowSpec {
    let mut steps = host_and_vm_setup_steps();
    steps.extend(guest_bootstrap_steps());
    FlowSpec {
        id: "vendor.initial-setup.hyperv",
        title: "Initial Hyper-V setup",
        provider: ProviderKind::HyperV,
        source_scripts: &[
            "initial-setup.bat",
            "internal-scripts/initial-setup.ps1",
            "internal-scripts/bootstrap/setup",
            "battlegroup-management/initial-setup.ps1",
            "battlegroup-management/bootstrap/setup",
            "download/scripts/setup.sh",
            "download/scripts/setup/k3s.sh",
            "download/scripts/setup/system.sh",
            "download/scripts/setup/world.sh",
            "download/scripts/battlegroup.sh update-from-downloads",
            "download/scripts/battlegroup.sh apply-default-usersettings",
        ],
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_setup_flow_covers_vendor_milestones() {
        let flow = hyperv_initial_setup_flow();
        let ids = flow.steps.iter().map(|step| step.id).collect::<Vec<_>>();
        for required in [
            "host.check-hyperv-module",
            "hyperv.import-vm",
            "hyperv.choose-or-create-switch",
            "guest.write-settings-conf",
            "guest.upload-bootstrap",
            "guest.k3s.import-core-images",
            "guest.system.install-battlegroup-helper",
            "guest.world.generate-name",
            "guest.images.patch-battlegroup",
            "guest.defaults.copy-user-settings",
        ] {
            assert!(ids.contains(&required), "missing step {required}");
        }
    }

    #[test]
    fn world_name_generation_preserves_vendor_lowercase_suffix_requirement() {
        let flow = hyperv_initial_setup_flow();
        let step = flow
            .steps
            .iter()
            .find(|step| step.id == "guest.world.generate-name")
            .expect("world generation step");
        assert!(step.native_strategy.contains("lowercase-only"));
        assert!(step.source.contains("six lowercase random letters"));
    }
}
