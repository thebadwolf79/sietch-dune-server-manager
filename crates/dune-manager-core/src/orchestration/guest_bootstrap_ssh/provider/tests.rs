use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use super::SshGuestBootstrapProvider;
use crate::orchestration::guest_bootstrap_ssh::scripts::download_script;
use crate::{
    models::CommandResult,
    orchestration::{GuestBootstrapProvider, RemoteCommandRunner, WorldManifestRequest},
};

#[derive(Clone, Default)]
struct MockRemote {
    outputs: Rc<RefCell<VecDeque<String>>>,
    scripts: Rc<RefCell<Vec<String>>>,
}

impl MockRemote {
    fn with_outputs(outputs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            outputs: Rc::new(RefCell::new(outputs.into_iter().map(Into::into).collect())),
            scripts: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl RemoteCommandRunner for MockRemote {
    fn run(&self, command: &str) -> CommandResult<String> {
        self.run_script(command)
    }

    fn run_script(&self, script: &str) -> CommandResult<String> {
        self.scripts.borrow_mut().push(script.to_string());
        Ok(self.outputs.borrow_mut().pop_front().unwrap_or_default())
    }
}

#[test]
fn create_world_returns_structured_json_only() {
    let remote = MockRemote::with_outputs([
        r#"{"namespace":"funcom-seabass-sh-host-abcdef","battlegroupName":"sh-host-abcdef"}"#,
    ]);
    let scripts = remote.scripts.clone();
    let provider = SshGuestBootstrapProvider::new(remote);
    let world = provider
        .create_world(&WorldManifestRequest {
            world_name: "Adain".to_string(),
            world_region: "Europe".to_string(),
            player_ip: "203.0.113.10".to_string(),
            world_unique_name: "sh-host-abcdef".to_string(),
            self_host_token: "header.payload.signature".to_string(),
        })
        .unwrap();

    assert_eq!(world.namespace, "funcom-seabass-sh-host-abcdef");
    let script = scripts.borrow().first().cloned().unwrap();
    assert!(script.contains("printf '{\"namespace\":\"%s\",\"battlegroupName\":\"%s\"}"));
    assert!(script.contains("kubectl create ns \"$NS\" >/dev/null"));
    assert!(script.contains("kubectl apply -n \"$NS\""));
    assert!(script.contains("DB_PASSWORD=$(openssl rand -hex 32)"));
    assert!(script.contains("s/{WORLD_DUNE_PASS}/$(escape_sed \"$DB_PASSWORD\")/g"));
    assert!(script.contains("s/{WORLD_POSTGRES_PASS}/$(escape_sed \"$DB_SUPER_PASSWORD\")/g"));
    assert!(script.contains("s/{WORLD_IMAGE_TAG}/0-0-shipping/g"));
    assert!(script.contains("HOST_DATACENTER_IP_ADDRESS"));
    assert!(script.contains("PLAYER_IP=$(cat <<"));
    assert!(!script.contains(
        "WORLD_IMAGE_TAG=$(cat \"$G_SPEC_PATH/download/images/battlegroup/version.txt\")"
    ));
}

#[test]
fn create_world_patches_full_title_after_template_creation() {
    let remote = MockRemote::with_outputs([
        r#"{"namespace":"funcom-seabass-sh-host-abcdef","battlegroupName":"sh-host-abcdef"}"#,
    ]);
    let scripts = remote.scripts.clone();
    let provider = SshGuestBootstrapProvider::new(remote);
    provider
        .create_world(&WorldManifestRequest {
            world_name: "Great Banana".to_string(),
            world_region: "Europe".to_string(),
            player_ip: "203.0.113.10".to_string(),
            world_unique_name: "sh-host-abcdef".to_string(),
            self_host_token: "header.payload.signature".to_string(),
        })
        .unwrap();

    let script = scripts.borrow().first().cloned().unwrap();
    assert!(script.contains("\"title\":\"Great Banana\""));
    assert!(script.contains("kubectl patch battlegroup \"$WORLD_UNIQUE_NAME\""));
}

#[test]
fn provider_splits_vendor_k3s_work_into_explicit_phases() {
    let remote = MockRemote::default();
    let scripts = remote.scripts.clone();
    let provider = SshGuestBootstrapProvider::new(remote);

    provider.start_k3s_and_wait().unwrap();
    provider.import_core_images().unwrap();
    provider.scale_core_deployments().unwrap();

    let scripts = scripts.borrow();
    assert!(scripts[0].contains("rc-service k3s restart"));
    assert!(scripts[1].contains("coredns-coredns.tar"));
    assert!(scripts[1].contains("restart_k3s_and_wait_until_ready"));
    assert!(scripts[2].contains("scale_deployment kube-system coredns 1"));
}

#[test]
fn operator_update_includes_vendor_database_concurrency_patch() {
    let remote = MockRemote::default();
    let scripts = remote.scripts.clone();
    let provider = SshGuestBootstrapProvider::new(remote);

    provider.patch_operator_images().unwrap();

    let script = scripts.borrow().first().cloned().unwrap();
    assert!(script.contains("patch_database_operator_concurrency"));
    assert!(script.contains("dbutil-max-concurrent=2"));
    assert!(script.contains("dbutil-max-concurrent=1"));
    assert!(script.contains("kubectl_retry rollout -n funcom-operators status"));
}

#[test]
fn helper_install_links_battlegroup_and_bg_util() {
    let remote = MockRemote::default();
    let scripts = remote.scripts.clone();
    let provider = SshGuestBootstrapProvider::new(remote);

    provider.install_battlegroup_helper().unwrap();

    let script = scripts.borrow().first().cloned().unwrap();
    assert!(script.contains("/home/dune/.dune/bin/battlegroup"));
    assert!(script.contains("/home/dune/.dune/bin/bg-util"));
    assert!(script.contains("chmod +x /home/dune/.dune/download/scripts/bg-util"));
}

#[test]
fn guest_download_uses_validating_app_update() {
    let script = download_script();
    assert!(script.contains("+app_update 4754530 validate"));
}

#[test]
fn guest_download_retries_without_interactive_prompts() {
    let script = download_script();
    assert!(script.contains("+@ShutdownOnFailedCommand 1"));
    assert!(script.contains("+@NoPromptForPassword 1"));
    assert!(script.contains("< /dev/null"));
    assert!(script.contains("max_attempts=5"));
    assert!(script.contains("retrying in ${sleep_seconds}s"));
}

#[test]
fn battlegroup_image_patch_uses_rust_built_json_patch_without_jq() {
    let remote = MockRemote::with_outputs([
        "1952287-0-shipping",
        "",
        r#"{
          "metadata":{"name":"sh-host-abcdef"},
          "spec":{
            "serverSets":[
              {"image":"registry.funcom.com/funcom/self-hosting/seabass-server:old"},
              {"image":"registry.funcom.com/funcom/self-hosting/other:old"}
            ],
            "nested":{"image":"registry.funcom.com/funcom/self-hosting/seabass-server-gateway:old"}
          }
        }"#,
        r#"{"metadata":{"name":"sh-host-abcdef"}}"#,
    ]);
    let scripts = remote.scripts.clone();
    let provider = SshGuestBootstrapProvider::new(remote);
    provider
        .patch_battlegroup_images("funcom-seabass-sh-host-abcdef", "sh-host-abcdef")
        .unwrap();

    let scripts = scripts.borrow();
    assert!(scripts[0].contains("version.txt"));
    assert!(scripts[1].contains("ALTER ROLE"));
    assert!(scripts[1].contains("superPassword"));
    assert!(scripts[2].contains("kubectl get battlegroup"));
    assert!(scripts[3].contains("kubectl patch battlegroup"));
    assert!(scripts[3].contains("--type=json"));
    assert!(scripts[3].contains("1952287-0-shipping"));
    assert!(scripts[3].contains("seabass-server-gateway"));
    assert!(!scripts.join("\n").contains("jq"));
}

#[test]
fn rejects_invalid_world_manifest_before_script_execution() {
    let remote = MockRemote::default();
    let scripts = remote.scripts.clone();
    let provider = SshGuestBootstrapProvider::new(remote);
    let result = provider.create_world(&WorldManifestRequest {
        world_name: "Adain".to_string(),
        world_region: "Mars".to_string(),
        player_ip: "203.0.113.10".to_string(),
        world_unique_name: "sh-host-abcdef".to_string(),
        self_host_token: "token".to_string(),
    });

    assert!(result.is_err());
    assert!(scripts.borrow().is_empty());
}
