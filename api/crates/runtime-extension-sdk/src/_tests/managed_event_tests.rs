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

#[test]
fn root_2007_ac_007_processed_effect_is_finite_and_matches_delivery() {
    let payload = ManagedEventPayload {
        model_id: "model-1".into(),
        status: ManagedEventStatus::Processed,
        result_reference: Some("processed_models/model-1".into()),
    };
    let host = ManagedEventHostFrame {
        protocol: MANAGED_EVENT_PROTOCOL_V1.into(),
        call_id: "effect-call".into(),
        handler: "apply_processed".into(),
        execution_identity: ManagedExecutionIdentity::new(
            ManagedInstallationId::new("installation").unwrap(),
            ManagedWorkspaceId::new("workspace").unwrap(),
            ContributionId::new("acme.composition-b.events").unwrap(),
            ManagedArtifactFingerprint::from_bytes(b"artifact"),
            ManagedBindingFingerprint::from_bytes(b"binding"),
        ),
        generation: std::num::NonZeroU64::new(1).unwrap(),
        graph_fingerprint: "graph".into(),
        authority_revision: 1,
        deadline_unix_ms: 10,
        delivery: ManagedEventDelivery {
            event_id: "event".into(),
            contract_id: MANAGED_PROCESSED_EVENT_ID.into(),
            contract_version: "1".into(),
            workspace_id: "workspace".into(),
            causation_id: "cause".into(),
            correlation_id: "cause".into(),
            payload,
        },
    };
    let raw = serde_json::to_vec(&host).unwrap();
    let mut output = Vec::new();
    crate::serve_managed_event(raw.as_slice(), &mut output, |frame| {
        ManagedEventOutcome::ApplyProcessed {
            effect: frame.delivery.payload.clone(),
        }
    })
    .unwrap();
    let response: ManagedEventWorkerFrame = serde_json::from_slice(&output).unwrap();
    response.validate_for(&host).unwrap();
    for field in [
        "owner_id",
        "table",
        "workspace_id",
        "operations",
        "idempotency_key",
        "actor_id",
    ] {
        let mut changed = serde_json::to_value(&response).unwrap();
        changed["result"][field] = "forged".into();
        assert!(serde_json::from_value::<ManagedEventWorkerFrame>(changed).is_err());
    }
    let mut changed = response.clone();
    let ManagedEventOutcome::ApplyProcessed { effect } = &mut changed.result else {
        panic!("processed effect expected")
    };
    effect.result_reference = Some("processed_models/other".into());
    assert!(changed.validate_for(&host).is_err());
    let mut wrong_contract = host;
    wrong_contract.delivery.contract_id = MANAGED_CREATE_EVENT_ID.into();
    wrong_contract.delivery.contract_version = "v1".into();
    wrong_contract.delivery.payload.status = ManagedEventStatus::Committed;
    assert!(response.validate_for(&wrong_contract).is_err());
}
