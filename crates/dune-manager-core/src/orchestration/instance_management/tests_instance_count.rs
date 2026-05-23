//! Tests for world-partition / instance-count behavior.

use serde_json::json;

use crate::orchestration::{
    instance_management::{
        count_models::SetMapInstancesRequest,
        instance_map::InstanceMap,
        orchestrator_helpers::{build_world_partition_update, deep_desert_pvp_ids},
        tests_fixtures::sample_battlegroup,
    },
    BattlegroupRef,
};

#[test]
fn preserves_single_deep_desert_partition_on_dimension_zero() {
    let update =
        build_world_partition_update(&sample_battlegroup(), InstanceMap::DeepDesert, 1).unwrap();

    assert_eq!(update.partition_ids, vec![8]);
    assert!(!update.patch_required);
    assert!(update.patch_operations.is_empty());
}

#[test]
fn rejects_multiple_deep_desert_world_partitions() {
    let request = SetMapInstancesRequest::new(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::DeepDesert,
        2,
    );

    assert!(request.validate().is_err());
}

#[test]
fn derives_deep_desert_pvp_ids_from_instance_count() {
    assert_eq!(deep_desert_pvp_ids(&[8], 0), Vec::<i64>::new());
    assert_eq!(deep_desert_pvp_ids(&[8], 1), vec![8]);
}

#[test]
fn rejects_pvp_instance_count_for_survival() {
    let mut request = SetMapInstancesRequest::new(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::Survival1,
        2,
    );
    request.pvp_instance_count = Some(1);

    assert!(request.validate().is_err());
}

#[test]
fn rejects_pvp_instance_count_above_deep_desert_total() {
    let mut request = SetMapInstancesRequest::new(
        BattlegroupRef {
            namespace: "funcom-seabass-sh-host-abcdef".to_string(),
            name: "sh-host-abcdef".to_string(),
        },
        InstanceMap::DeepDesert,
        1,
    );
    request.pvp_instance_count = Some(2);

    assert!(request.validate().is_err());
}

#[test]
fn shrinks_survival_partitions_by_dimension_order() {
    let mut bg = sample_battlegroup();
    bg["spec"]["database"]["template"]["spec"]["deployment"]["spec"]["worldPartitions"][0]
        ["partitions"] = json!([
        {"id":1,"dimension":0,"disable":false},
        {"id":30,"dimension":2,"disable":false},
        {"id":29,"dimension":1,"disable":false}
    ]);

    let update = build_world_partition_update(&bg, InstanceMap::Survival1, 2).unwrap();

    assert_eq!(update.partition_ids, vec![1, 29]);
    assert!(update.patch_required);
    assert_eq!(
        update.patch_operations[1],
        json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/replicas","value":2})
    );
    assert_eq!(
        update.patch_operations[2],
        json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/partitions","value":[1,29]})
    );
}

#[test]
fn adds_survival_partitions_with_new_dimensions() {
    let update =
        build_world_partition_update(&sample_battlegroup(), InstanceMap::Survival1, 3).unwrap();

    assert_eq!(update.partition_ids, vec![1, 9, 10]);
    assert_eq!(
        update.patch_operations[0]["value"],
        json!([
            {"id":1,"dimension":0,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1},
            {"id":9,"dimension":1,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1},
            {"id":10,"dimension":2,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1}
        ])
    );
    assert_eq!(
        update.patch_operations[1],
        json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/replicas","value":3})
    );
    assert_eq!(
        update.patch_operations[2],
        json!({"op":"replace","path":"/spec/serverGroup/template/spec/sets/0/partitions","value":[1,9,10]})
    );
}

#[test]
fn leaves_survival_server_group_when_count_is_current() {
    let update =
        build_world_partition_update(&sample_battlegroup(), InstanceMap::Survival1, 1).unwrap();

    assert_eq!(update.partition_ids, vec![1]);
    assert!(!update.patch_required);
    assert!(update.patch_operations.is_empty());
}

#[test]
fn adds_survival_partitions_field_when_missing() {
    let mut bg = sample_battlegroup();
    bg["spec"]["serverGroup"]["template"]["spec"]["sets"][0] =
        json!({"map":"Survival_1","replicas":1});

    let update = build_world_partition_update(&bg, InstanceMap::Survival1, 1).unwrap();

    assert_eq!(
        update.patch_operations[0],
        json!({"op":"add","path":"/spec/serverGroup/template/spec/sets/0/partitions","value":[1]})
    );
}
