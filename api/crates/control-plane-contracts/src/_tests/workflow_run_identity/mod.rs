use crate::{
    WorkflowCredentialIdentity, WorkflowRunIdentityRepository, WorkflowRunIdentitySnapshot,
};
use uuid::Uuid;

#[test]
fn workflow_identity_contract_round_trips_only_safe_frozen_identity_fields() {
    fn adapter_contract<T: WorkflowRunIdentityRepository + ?Sized>() {}
    adapter_contract::<dyn WorkflowRunIdentityRepository>();
    let identity = WorkflowCredentialIdentity {
        user_id: Uuid::from_u128(1),
        tenant_id: Uuid::from_u128(2),
        workspace_id: Uuid::from_u128(3),
        active_role_code: "workflow_executor".into(),
        session_version: 4,
    };
    let snapshot = WorkflowRunIdentitySnapshot {
        flow_run_id: Uuid::from_u128(5),
        creator: None,
        executor: Some(identity),
    };
    let encoded = serde_json::to_value(&snapshot).unwrap();
    assert!(encoded["creator"].is_null());
    assert_eq!(encoded["executor"].as_object().unwrap().len(), 5);
    assert_eq!(
        serde_json::from_value::<WorkflowRunIdentitySnapshot>(encoded.clone()).unwrap(),
        snapshot
    );
    let mut injected = encoded;
    injected["executor"]["permissions"] = serde_json::json!(["*"]);
    assert!(serde_json::from_value::<WorkflowRunIdentitySnapshot>(injected).is_err());
}
