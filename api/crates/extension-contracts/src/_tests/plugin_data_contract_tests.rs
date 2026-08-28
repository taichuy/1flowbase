use std::collections::BTreeMap;

use serde_json::json;

use crate::{
    PluginDataOperation, PluginDataRequest, PluginDataTarget, PluginDataValue,
    RuntimeHostWorkerFrame, PLUGIN_DATA_SERVICE_V1, RUNTIME_HOST_CALL_PROTOCOL_V1,
};

#[test]
fn pdp_001_pdp_006_typed_request_has_bounded_operations_and_values() {
    let request = PluginDataRequest {
        idempotency_key: Some("invocation-1".to_string()),
        operations: vec![PluginDataOperation::Upsert {
            target: PluginDataTarget::OwnedCollection {
                collection_code: "affinity".to_string(),
            },
            identity: BTreeMap::from([(
                "conversation_id".to_string(),
                PluginDataValue::Uuid("00000000-0000-0000-0000-000000000001".to_string()),
            )]),
            values: BTreeMap::from([(
                "provider_id".to_string(),
                PluginDataValue::String("provider-a".to_string()),
            )]),
        }],
    };
    request.validate().unwrap();
    let serialized = serde_json::to_value(&request).unwrap();
    assert!(serialized.get("sql").is_none());
    assert!(serialized.get("connection").is_none());
}

#[test]
fn pdp_003_worker_frame_cannot_smuggle_trusted_binding_identity() {
    let spoofed = json!({
        "frame": "host_call",
        "protocol": RUNTIME_HOST_CALL_PROTOCOL_V1,
        "call_id": "call-1",
        "service": PLUGIN_DATA_SERVICE_V1,
        "plugin_id": "attacker",
        "workspace_id": "attacker",
        "request": {
            "operations": [{
                "operation": "count",
                "target": {"kind": "owned_collection", "collection_code": "affinity"}
            }]
        }
    });
    assert!(serde_json::from_value::<RuntimeHostWorkerFrame>(spoofed).is_err());
}

#[test]
fn pdp_009_runtime_host_call_v1_golden_is_additive_and_correlated() {
    let raw = r#"{"frame":"host_call","protocol":"runtime_host_call/v1","call_id":"call-1","service":"plugin_data/v1","request":{"operations":[{"operation":"count","target":{"kind":"owned_collection","collection_code":"affinity"}}]}}"#;
    let frame: RuntimeHostWorkerFrame = serde_json::from_str(raw).unwrap();
    assert_eq!(serde_json::to_string(&frame).unwrap(), raw);
}
