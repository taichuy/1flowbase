use crate::{
    MultiplexEnvelope, MultiplexHostService, MultiplexWorkerMessage, PluginExecutionGateRequest,
    PluginExecutionGateResponse,
};
use serde_json::json;
#[test]
fn execution_gate_wire_is_additive_typed_and_rejects_trusted_identity() {
    let envelope = MultiplexEnvelope::new(MultiplexWorkerMessage::Callback {
        call_id: "1".into(),
        callback_id: "2".into(),
        service: MultiplexHostService::PluginExecutionGateV1,
        request: serde_json::to_value(PluginExecutionGateRequest::Acquire {
            key: "entry".into(),
        })
        .unwrap(),
    });
    let value = serde_json::to_value(envelope).unwrap();
    assert_eq!(value["service"], "plugin_execution_gate_v1");
    assert_eq!(
        value["request"],
        json!({"operation":"acquire","key":"entry"})
    );
    assert_eq!(
        serde_json::to_value(PluginExecutionGateResponse::Release).unwrap(),
        json!({"operation":"release"})
    );
    assert!(serde_json::from_value::<PluginExecutionGateRequest>(
        json!({"operation":"acquire","key":"entry","installation_id":"forged"})
    )
    .is_err());
    assert!(serde_json::from_value::<PluginExecutionGateRequest>(
        json!({"operation":"resize","key":"entry","capacity":3})
    )
    .is_err());
    for key in ["", "new\nline"] {
        assert!(PluginExecutionGateRequest::Acquire { key: key.into() }
            .validate()
            .is_err());
    }
}
