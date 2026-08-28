use std::{collections::BTreeMap, io::Cursor};

use extension_contracts::{
    PluginDataOperation, PluginDataOperationResult, PluginDataRequest, PluginDataResponse,
    PluginDataTarget, RuntimeHostFrame, RUNTIME_HOST_CALL_PROTOCOL_V1,
};

use crate::{PluginDataClient, PluginDataHostSimulator};

fn count_request() -> PluginDataRequest {
    PluginDataRequest {
        idempotency_key: None,
        operations: vec![PluginDataOperation::Count {
            target: PluginDataTarget::OwnedCollection {
                collection_code: "affinity".to_string(),
            },
            filters: vec![],
        }],
    }
}

#[test]
fn pdp_011_sdk_emits_typed_correlated_host_call() {
    let host_result = serde_json::to_string(&RuntimeHostFrame::HostResult {
        protocol: RUNTIME_HOST_CALL_PROTOCOL_V1.to_string(),
        call_id: "sdk-1".to_string(),
        result: Some(PluginDataResponse {
            results: vec![PluginDataOperationResult::Count { count: 2 }],
            replayed: false,
        }),
        error: None,
    })
    .unwrap();
    let mut client = PluginDataClient::new(
        Cursor::new(format!("{host_result}\n")),
        Cursor::new(Vec::new()),
    );
    let response = client.execute(count_request()).unwrap();
    assert_eq!(
        response.results,
        vec![PluginDataOperationResult::Count { count: 2 }]
    );
    let (_, output) = client.into_inner();
    let worker_frame: serde_json::Value = serde_json::from_slice(output.get_ref()).unwrap();
    assert_eq!(worker_frame["frame"], "host_call");
    assert!(worker_frame.get("workspace_id").is_none());
    assert!(worker_frame.get("plugin_id").is_none());
}

#[test]
fn pdp_012_simulator_round_trips_sdk_wire_without_host_internals() {
    let mut simulator = PluginDataHostSimulator::new(|_| {
        Ok(PluginDataResponse {
            results: vec![PluginDataOperationResult::Count { count: 3 }],
            replayed: false,
        })
    });
    let request = serde_json::json!({
        "frame": "host_call",
        "protocol": "runtime_host_call/v1",
        "call_id": "fixture-1",
        "service": "plugin_data/v1",
        "request": count_request(),
    });
    let response = simulator.accept_worker_line(&request.to_string()).unwrap();
    assert!(response.contains("\"call_id\":\"fixture-1\""));
    assert!(response.contains("\"count\":3"));
}

#[test]
fn public_fixture_does_not_require_untyped_maps() {
    let request = count_request();
    assert!(request.operations.len() == 1);
    let _typed_placeholder: BTreeMap<String, extension_contracts::PluginDataValue> =
        BTreeMap::new();
}
