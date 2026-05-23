//! Shared test fixtures for instance management tests.

use serde_json::{json, Value};

pub(super) fn sample_battlegroup() -> Value {
    json!({
        "spec": {
            "database": {
                "template": {
                    "spec": {
                        "deployment": {
                            "spec": {
                                "worldPartitions": [
                                    {"map":"Survival_1","partitions":[{"id":1,"dimension":0,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1}]},
                                    {"map":"Other","partitions":[{"id":2,"dimension":0,"disable":false}]},
                                    {"map":"DeepDesert_1","partitions":[{"id":8,"dimension":0,"disable":false,"minX":0,"minY":0,"maxX":1,"maxY":1}]}
                                ]
                            }
                        }
                    }
                }
            },
            "serverGroup": {
                "template": {
                    "spec": {
                        "sets": [
                            {"map":"Survival_1","replicas":1,"partitions":[1]},
                            {"map":"Overmap","replicas":1,"partitions":[2]},
                            {"map":"DeepDesert_1","replicas":0}
                        ]
                    }
                }
            }
        }
    })
}
