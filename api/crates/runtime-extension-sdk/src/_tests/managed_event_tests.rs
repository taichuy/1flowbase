//! Root #2007 regressions through Root #2014 registered event v2.
use extension_contracts::*;
fn schema() -> ManagedEventSchema {
    ManagedEventSchema {
        contract_id: "orion.shipments.created".into(),
        contract_version: "1".into(),
        payload_schema: serde_json::json!({"type":"object", "additionalProperties":false,
            "properties":{"shipment_id":{"type":"string","maxLength":128}, "destination_code":{"type":"string","maxLength":32},"item_count":{"type":"integer"}},
            "required":["shipment_id","destination_code","item_count"]}),
    }
}
fn publication() -> ManagedEventPublication {
    ManagedEventPublication {
        contract_id: schema().contract_id,
        contract_version: "1".into(),
        payload: serde_json::json!({"shipment_id":"s1","destination_code":"NYC","item_count":2}),
    }
}
#[test]
fn root_2007_ac_005_event_authority_wire_rejects_identity_and_wrong_version() {
    let request = publication();
    request.validate_against(&schema()).unwrap();
    let mut wrong = request.clone();
    wrong.contract_version = "2".into();
    assert!(wrong.validate_against(&schema()).is_err());
    for field in ["publisher", "workspace_id", "actor_id", "causation_id"] {
        let mut value = serde_json::to_value(&request).unwrap();
        value[field] = "forged".into();
        assert!(serde_json::from_value::<ManagedEventPublication>(value).is_err());
    }
}
#[test]
fn root_2014_ac_005_006_event_registered_schema_rejects_unknown_fields() {
    let mut request = publication();
    request.payload["actor_id"] = "forged".into();
    assert!(request.validate_against(&schema()).is_err());
    request = publication();
    request.payload["item_count"] = "two".into();
    assert!(request.validate_against(&schema()).is_err());
    let mut invalid = schema();
    invalid.payload_schema["$ref"] = "https://invalid.test/schema".into();
    assert!(publication().validate_against(&invalid).is_err());
}
#[test]
fn root_2007_ac_007_processed_effect_is_finite_and_matches_delivery() {
    let request = publication();
    let host = ManagedEventHostFrame {
        protocol: MANAGED_EVENT_PROTOCOL_V2.into(),
        call_id: "effect-call".into(),
        handler: "apply_owned".into(),
        execution_identity: ManagedExecutionIdentity::new(
            ManagedInstallationId::new("installation").unwrap(),
            ManagedWorkspaceId::new("workspace").unwrap(),
            ContributionId::new("subscriber.events").unwrap(),
            ManagedArtifactFingerprint::from_bytes(b"artifact"),
            ManagedBindingFingerprint::from_bytes(b"binding"),
        ),
        generation: std::num::NonZeroU64::new(1).unwrap(),
        graph_fingerprint: "graph".into(),
        authority_revision: 1,
        deadline_unix_ms: 10,
        delivery: ManagedEventDelivery {
            event_id: "event".into(),
            contract_id: request.contract_id.clone(),
            contract_version: "1".into(),
            point_id: request.contract_id,
            point_contract_version: "1".into(),
            schema: schema(),
            workspace_id: "workspace".into(),
            causation_id: "cause".into(),
            correlation_id: "cause".into(),
            payload: request.payload,
        },
    };
    let operations = vec![PluginDataOperation::Upsert {
        target: PluginDataTarget::OwnedCollection {
            collection_code: "shipments".into(),
        },
        identity: [("shipment_id".into(), PluginDataValue::String("s1".into()))]
            .into_iter()
            .collect(),
        values: [("accepted".into(), PluginDataValue::Boolean(true))]
            .into_iter()
            .collect(),
    }];
    let mut output = Vec::new();
    crate::serve_managed_event(
        serde_json::to_vec(&host).unwrap().as_slice(),
        &mut output,
        |_| ManagedEventOutcome::ApplyOwned { operations },
    )
    .unwrap();
    let response: ManagedEventWorkerFrame = serde_json::from_slice(&output).unwrap();
    response.validate_for(&host).unwrap();
    for field in [
        "owner_id",
        "table",
        "workspace_id",
        "idempotency_key",
        "actor_id",
    ] {
        let mut changed = serde_json::to_value(&response).unwrap();
        changed["result"][field] = "forged".into();
        assert!(serde_json::from_value::<ManagedEventWorkerFrame>(changed).is_err());
    }
    let mut changed = response;
    let ManagedEventOutcome::ApplyOwned { operations } = &mut changed.result else {
        panic!("effect expected")
    };
    let PluginDataOperation::Upsert { target, .. } = &mut operations[0] else {
        panic!("upsert expected")
    };
    *target = PluginDataTarget::ExtensionProjection {
        target_table: "users".into(),
    };
    assert!(changed.validate_for(&host).is_err());
    let mut wrong = host;
    wrong.delivery.contract_version = "2".into();
    assert!(wrong.validate().is_err());
}
