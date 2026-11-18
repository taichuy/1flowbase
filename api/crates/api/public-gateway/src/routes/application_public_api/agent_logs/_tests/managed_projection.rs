use super::{Input, Output, TargetError};
use control_plane::ports::{AgentLogsBatch, AgentLogsReceipt, AGENT_LOGS_SCHEMA_VERSION};
use extension_contracts::ManagedProjectionContract;
use interface_runtime::InterfaceContract;
use serde_json::json;

fn assert_registered_kind_only_projection<T: InterfaceContract>(value: T) {
    let contract = ManagedProjectionContract {
        contract_id: T::CONTRACT_ID.into(),
        contract_version: T::CONTRACT_VERSION.into(),
        schema: T::managed_projection_schema().unwrap(),
    };
    let compiled = contract.compile_for_registration().unwrap();
    let projected = value.project_for_managed_hook().unwrap();
    assert_eq!(projected, json!({"kind": T::CONTRACT_ID}));
    compiled.validate(&projected).unwrap();
    for rejected in [
        json!({"kind": "wrong-contract"}),
        json!({}),
        json!({"kind": T::CONTRACT_ID, "content": "must not be exposed"}),
    ] {
        assert!(
            compiled.validate(&rejected).is_err(),
            "{}: {rejected}",
            T::CONTRACT_ID
        );
    }

    let mut old = contract.clone();
    old.schema["properties"]["kind"] = json!({"const": T::CONTRACT_ID});
    assert!(
        old.compile_for_registration().is_err(),
        "missing type must reject {}",
        T::CONTRACT_ID
    );
    old.schema["properties"]["kind"] = json!({"type": "string", "const": T::CONTRACT_ID});
    assert!(
        old.compile_for_registration().is_err(),
        "missing maxLength must reject {}",
        T::CONTRACT_ID
    );
}

#[test]
fn all_agent_logs_contracts_register_and_enforce_kind_only_safe_views() {
    assert_registered_kind_only_projection(Input(AgentLogsBatch {
        schema_version: AGENT_LOGS_SCHEMA_VERSION.into(),
        source_id: "collector".into(),
        source_client: "test".into(),
        events: vec![],
    }));
    assert_registered_kind_only_projection(Output(AgentLogsReceipt {
        accepted_events: 0,
        duplicate_events: 0,
        record_ids: vec![],
    }));
    assert_registered_kind_only_projection(TargetError(
        control_plane::errors::ControlPlaneError::NotAuthenticated.into(),
    ));
}
