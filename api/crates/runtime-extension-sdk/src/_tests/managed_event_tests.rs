//! Root #2007 AC-005: finite wire, exact contracts, and no worker-asserted authority.
use extension_contracts::*;

#[test]
fn root_2007_ac_005_event_authority_wire_rejects_identity_and_wrong_version() {
    let request = ManagedEventPublication {
        contract_id: MANAGED_PROCESSED_EVENT_ID.into(),
        contract_version: "1".into(),
        payload: ManagedEventPayload {
            model_id: "model-1".into(),
            status: ManagedEventStatus::Processed,
            result_reference: Some("processed_models/model-1".into()),
        },
    };
    request.validate().unwrap();
    let mut wrong = request.clone();
    wrong.contract_version = "2".into();
    assert!(wrong.validate().is_err());
    for field in ["publisher", "workspace_id", "actor_id", "causation_id"] {
        let mut value = serde_json::to_value(&request).unwrap();
        value[field] = "forged".into();
        assert!(serde_json::from_value::<ManagedEventPublication>(value).is_err());
    }
}
