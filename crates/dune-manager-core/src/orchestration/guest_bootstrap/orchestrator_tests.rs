use std::{cell::RefCell, rc::Rc};

use crate::orchestration::{CreatedWorld, VecOperationSink, WorldManifestRequest};

use super::*;

#[derive(Default)]
struct MockGuestBootstrap {
    calls: Rc<RefCell<Vec<&'static str>>>,
}

impl GuestBootstrapProvider for MockGuestBootstrap {
    fn validate_and_resize_root_disk(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("disk");
        Ok(())
    }

    fn ensure_server_payload(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("payload");
        Ok(())
    }

    fn start_k3s_and_wait(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("k3s");
        Ok(())
    }

    fn import_core_images(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("core_images");
        Ok(())
    }

    fn scale_core_deployments(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("core_scale");
        Ok(())
    }

    fn update_operator_crds(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("operator_crds");
        Ok(())
    }

    fn patch_operator_images(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("operator_images");
        Ok(())
    }

    fn scale_operator_deployments(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("operator_scale");
        Ok(())
    }

    fn install_battlegroup_helper(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("helper");
        Ok(())
    }

    fn create_world(&self, request: &WorldManifestRequest) -> CommandResult<CreatedWorld> {
        self.calls.borrow_mut().push("world");
        Ok(CreatedWorld {
            namespace: format!("funcom-seabass-{}", request.world_unique_name),
            battlegroup_name: request.world_unique_name.clone(),
        })
    }

    fn import_battlegroup_images(&self) -> CommandResult<()> {
        self.calls.borrow_mut().push("bg_images");
        Ok(())
    }

    fn patch_battlegroup_images(
        &self,
        _namespace: &str,
        _battlegroup_name: &str,
    ) -> CommandResult<()> {
        self.calls.borrow_mut().push("bg_patch");
        Ok(())
    }

    fn apply_default_user_settings(
        &self,
        _namespace: &str,
        _battlegroup_name: &str,
    ) -> CommandResult<()> {
        self.calls.borrow_mut().push("defaults");
        Ok(())
    }
}

#[test]
fn orchestrates_guest_bootstrap_sequence() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let provider = MockGuestBootstrap {
        calls: calls.clone(),
    };
    let orchestrator = GuestBootstrapOrchestrator::new(provider);
    let mut sink = VecOperationSink::default();
    let result = orchestrator
        .run(
            &GuestBootstrapPlan {
                player_ip: "10.0.0.4".to_string(),
                world_name: "Adain".to_string(),
                world_region: "Europe".to_string(),
                self_host_token: "token".to_string(),
                host_id: "abc123".to_string(),
                world_suffix: "abcdef".to_string(),
            },
            &mut sink,
        )
        .unwrap();

    assert_eq!(result.namespace, "funcom-seabass-sh-abc123-abcdef");
    assert_eq!(
        calls.borrow().as_slice(),
        &[
            "disk",
            "payload",
            "k3s",
            "core_images",
            "core_scale",
            "operator_crds",
            "operator_images",
            "operator_scale",
            "helper",
            "world",
            "bg_images",
            "bg_patch",
            "defaults",
        ]
    );
    assert!(sink
        .events
        .iter()
        .any(|event| event.step_id == "guest-world.create"));
}
