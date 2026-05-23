//! Tests for display-name patch behavior.

use serde_json::json;

use crate::orchestration::{
    instance_management::{
        display_name_helpers::build_display_name_update,
        display_name_models::SetMapDisplayNameRequest, instance_map::InstanceMap,
        tests_fixtures::sample_battlegroup,
    },
    BattlegroupRef,
};

#[test]
fn display_name_adds_per_partition_pod_specs_for_dimension() {
    let mut bg = sample_battlegroup();
    bg["spec"]["database"]["template"]["spec"]["deployment"]["spec"]["worldPartitions"][0]
        ["partitions"] = json!([
        {"id":1,"dimension":0,"disable":false},
        {"id":29,"dimension":1,"disable":false}
    ]);
    bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["partitions"] = json!([1, 29]);
    bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["replicas"] = json!(2);
    let request = SetMapDisplayNameRequest::set(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::Survival1,
        0,
        "Bob",
    );

    let update = build_display_name_update(&bg, &request).unwrap();

    assert_eq!(update.partition_id, 1);
    assert!(update.patch_required);
    assert_eq!(
        update.patch_operations,
        vec![json!({
            "op": "add",
            "path": "/spec/serverGroup/template/spec/sets/0/podSpecs",
            "value": [{
                "index": 1,
                "arguments": ["-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"]
            }]
        })]
    );
}

#[test]
fn display_name_adds_new_pod_spec_without_touching_other_dimensions() {
    let mut bg = sample_battlegroup();
    bg["spec"]["database"]["template"]["spec"]["deployment"]["spec"]["worldPartitions"][0]
        ["partitions"] = json!([
        {"id":1,"dimension":0,"disable":false},
        {"id":29,"dimension":1,"disable":false}
    ]);
    bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] =
        json!([{"index":29,"arguments":["-SomeOtherArg=value"]}]);
    let request = SetMapDisplayNameRequest::set(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::Survival1,
        0,
        "Bob",
    );

    let update = build_display_name_update(&bg, &request).unwrap();

    assert_eq!(
        update.patch_operations,
        vec![json!({
            "op": "add",
            "path": "/spec/serverGroup/template/spec/sets/0/podSpecs/-",
            "value": {
                "index": 1,
                "arguments": ["-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"]
            }
        })]
    );
}

#[test]
fn display_name_replaces_existing_override() {
    let mut bg = sample_battlegroup();
    bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] = json!([{
        "index": 1,
        "arguments": [
            "-Other=value",
            "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Alice"
        ]
    }]);
    let request = SetMapDisplayNameRequest::set(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::Survival1,
        0,
        "Bob",
    );

    let update = build_display_name_update(&bg, &request).unwrap();

    assert_eq!(
        update.patch_operations,
        vec![json!({
            "op": "replace",
            "path": "/spec/serverGroup/template/spec/sets/0/podSpecs/0/arguments/1",
            "value": "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"
        })]
    );
}

#[test]
fn display_name_clear_removes_only_override_argument() {
    let mut bg = sample_battlegroup();
    bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] = json!([{
        "index": 1,
        "arguments": [
            "-Other=value",
            "-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Alice"
        ]
    }]);
    let request = SetMapDisplayNameRequest::clear(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::Survival1,
        0,
    );

    let update = build_display_name_update(&bg, &request).unwrap();

    assert_eq!(
        update.patch_operations,
        vec![json!({
            "op": "remove",
            "path": "/spec/serverGroup/template/spec/sets/0/podSpecs/0/arguments/1"
        })]
    );
}

#[test]
fn display_name_is_noop_when_value_is_current() {
    let mut bg = sample_battlegroup();
    bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0]["podSpecs"] = json!([{
        "index": 1,
        "arguments": ["-ini:engine:[ConsoleVariables]:Bgd.ServerDisplayName=Bob"]
    }]);
    let request = SetMapDisplayNameRequest::set(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::Survival1,
        0,
        "Bob",
    );

    let update = build_display_name_update(&bg, &request).unwrap();

    assert!(!update.patch_required);
    assert!(update.patch_operations.is_empty());
}

#[test]
fn display_name_rejects_missing_dimension() {
    let request = SetMapDisplayNameRequest::set(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::Survival1,
        9,
        "Bob",
    );

    let err = build_display_name_update(&sample_battlegroup(), &request).unwrap_err();

    assert!(err.message.contains("dimension 9"));
}
