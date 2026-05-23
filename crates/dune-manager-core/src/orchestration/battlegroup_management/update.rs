use crate::{
    models::CommandResult,
    orchestration::{GuestBootstrapProvider, OperationSink, StepAction},
};

use super::lifecycle::emit;
use super::models::BattlegroupRef;

/// Updates a BattleGroup from downloaded guest payload files.
pub struct BattlegroupUpdateOrchestrator<B> {
    bootstrap: B,
}

impl<B> BattlegroupUpdateOrchestrator<B>
where
    B: GuestBootstrapProvider,
{
    /// Creates an update orchestrator around a guest bootstrap provider.
    pub fn new(bootstrap: B) -> Self {
        Self { bootstrap }
    }

    /// Imports downloaded images and patches the live BattleGroup image revisions.
    pub fn update_from_downloads(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(
            sink,
            "bg.update.import-images",
            "Importing downloaded battlegroup images.",
            StepAction::Import,
        );
        self.bootstrap.import_battlegroup_images()?;
        emit(
            sink,
            "bg.update.patch-images",
            "Patching battlegroup image revisions.",
            StepAction::Patch,
        );
        self.bootstrap
            .patch_battlegroup_images(&battlegroup.namespace, &battlegroup.name)
    }

    /// Downloads the latest guest payload, imports images, and patches the BattleGroup.
    pub fn update_from_steam(
        &self,
        battlegroup: &BattlegroupRef,
        sink: &mut impl OperationSink,
    ) -> CommandResult<()> {
        battlegroup.validate()?;
        emit(
            sink,
            "bg.update.download-payload",
            "Checking and downloading guest server payload.",
            StepAction::Download,
        );
        self.bootstrap.ensure_server_payload()?;
        self.update_from_downloads(battlegroup, sink)
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::orchestration::VecOperationSink;

    use super::*;

    #[derive(Default)]
    struct MockBootstrap {
        calls: Rc<RefCell<Vec<&'static str>>>,
    }

    impl GuestBootstrapProvider for MockBootstrap {
        fn validate_and_resize_root_disk(&self) -> CommandResult<()> {
            Ok(())
        }

        fn ensure_server_payload(&self) -> CommandResult<()> {
            self.calls.borrow_mut().push("ensure_server_payload");
            Ok(())
        }

        fn start_k3s_and_wait(&self) -> CommandResult<()> {
            Ok(())
        }

        fn import_core_images(&self) -> CommandResult<()> {
            Ok(())
        }

        fn scale_core_deployments(&self) -> CommandResult<()> {
            Ok(())
        }

        fn update_operator_crds(&self) -> CommandResult<()> {
            Ok(())
        }

        fn patch_operator_images(&self) -> CommandResult<()> {
            Ok(())
        }

        fn scale_operator_deployments(&self) -> CommandResult<()> {
            Ok(())
        }

        fn install_battlegroup_helper(&self) -> CommandResult<()> {
            Ok(())
        }

        fn create_world(
            &self,
            _request: &crate::orchestration::WorldManifestRequest,
        ) -> CommandResult<crate::orchestration::CreatedWorld> {
            unreachable!("update does not create worlds")
        }

        fn import_battlegroup_images(&self) -> CommandResult<()> {
            self.calls.borrow_mut().push("import_battlegroup_images");
            Ok(())
        }

        fn patch_battlegroup_images(
            &self,
            _namespace: &str,
            _battlegroup_name: &str,
        ) -> CommandResult<()> {
            self.calls.borrow_mut().push("patch_battlegroup_images");
            Ok(())
        }

        fn apply_default_user_settings(
            &self,
            _namespace: &str,
            _battlegroup_name: &str,
        ) -> CommandResult<()> {
            Ok(())
        }
    }

    #[test]
    fn update_imports_and_patches_battlegroup_images() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let updater = BattlegroupUpdateOrchestrator::new(MockBootstrap {
            calls: calls.clone(),
        });
        let mut sink = VecOperationSink::default();
        updater
            .update_from_downloads(
                &BattlegroupRef {
                    namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                    name: "sh-host-abcdef".to_string(),
                },
                &mut sink,
            )
            .unwrap();

        assert_eq!(
            calls.borrow().as_slice(),
            &["import_battlegroup_images", "patch_battlegroup_images"]
        );
        assert!(sink
            .events
            .iter()
            .any(|event| event.step_id == "bg.update.patch-images"));
    }

    #[test]
    fn full_update_downloads_before_importing_and_patching() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let updater = BattlegroupUpdateOrchestrator::new(MockBootstrap {
            calls: calls.clone(),
        });
        let mut sink = VecOperationSink::default();
        updater
            .update_from_steam(
                &BattlegroupRef {
                    namespace: "funcom-seabass-sh-host-abcdef".to_string(),
                    name: "sh-host-abcdef".to_string(),
                },
                &mut sink,
            )
            .unwrap();

        assert_eq!(
            calls.borrow().as_slice(),
            &[
                "ensure_server_payload",
                "import_battlegroup_images",
                "patch_battlegroup_images"
            ]
        );
        assert!(sink
            .events
            .iter()
            .any(|event| event.step_id == "bg.update.download-payload"));
    }
}
