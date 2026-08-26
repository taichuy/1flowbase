use crate::ports::runtime::{
    CommitFlowRunTerminalResult, DataSourceRuntimePort, LegacyRuntimeShadowSourceKind,
};
use domain::FlowRunStatus;
use serde_json::json;

#[test]
fn runtime_persistence_contracts_keep_canonical_values_and_ports() {
    let source_kind = LegacyRuntimeShadowSourceKind::CallbackResponse;
    let encoded = serde_json::to_value(source_kind).expect("source kind should serialize");
    let decoded: LegacyRuntimeShadowSourceKind =
        serde_json::from_value(encoded).expect("source kind should deserialize");
    assert_eq!(decoded, source_kind);
    assert_eq!(source_kind.as_str(), "callback_response");

    let result = CommitFlowRunTerminalResult::Failed {
        output_payload: json!({"partial": true}),
        error_payload: json!({"code": "runtime_failed"}),
    };
    assert_eq!(result.status(), FlowRunStatus::Failed);
    assert_eq!(result.output_payload(), &json!({"partial": true}));
    assert_eq!(
        result.error_payload(),
        Some(&json!({"code": "runtime_failed"}))
    );
    assert_eq!(result.flow_run_event_type(), "flow_run_failed");
    assert_eq!(result.runtime_event_type(), "flow_failed");

    fn accepts_data_source_runtime_port(_: Option<&dyn DataSourceRuntimePort>) {}
    accepts_data_source_runtime_port(None);
}
