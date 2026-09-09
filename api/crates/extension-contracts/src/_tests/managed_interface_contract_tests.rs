//! Root #2014 AC-001/003/004. Run only in the frozen Root Test Batch.
use crate::*;
use serde_json::json;

fn contract() -> ManagedProjectionContract {
    ManagedProjectionContract {
        contract_id: "shipment.request".into(),
        contract_version: "1".into(),
        schema: json!({"type":"object", "properties": {
            "shipment_id":{"type":"string","maxLength":64},
            "item_count":{"type":"integer","minimum":0,"maximum":100}
        },"required":["shipment_id","item_count"],"additionalProperties":false}),
    }
}

#[test]
fn root_2014_managed_schema_rejects_unknown_fields_and_oversize() {
    let schema = contract().compile().unwrap();
    schema
        .validate(&json!({"shipment_id":"s1","item_count":2}))
        .unwrap();
    for value in [
        json!({"shipment_id":"s1","item_count":2,"password":"secret-sentinel"}),
        json!({"shipment_id":"x".repeat(65),"item_count":2}),
        json!({"shipment_id":"s1","item_count":101}),
        json!({"shipment_id":"s1","item_count":"2"}),
        json!({"item_count":2}),
    ] {
        let error = schema.validate(&value).unwrap_err().to_string();
        assert!(!error.contains("secret-sentinel"));
    }
    for schema in [
        json!({"$ref":"https://example.invalid/credential-schema"}),
        json!({"type":"integer","properties":{"x":{"$ref":"https://example.invalid/hidden"}}}),
        json!({"type":"string","maxLength":32,"items":{"$ref":"https://example.invalid/hidden"}}),
        json!({"type":"object"}),
        json!({"type":"array","items":{"type":"integer"}}),
        json!({"type":"string"}),
        json!({"type":"object","properties":{},"additionalProperties":true}),
    ] {
        assert!(ManagedProjectionContract {
            schema,
            ..contract()
        }
        .compile()
        .is_err());
    }
    // Object key insertion order must not change the frozen schema identity.
    let mut reordered = contract();
    reordered.schema = serde_json::from_str(r#"{"additionalProperties":false,"required":["shipment_id","item_count"],"properties":{"item_count":{"maximum":100,"minimum":0,"type":"integer"},"shipment_id":{"maxLength":64,"type":"string"}},"type":"object"}"#).unwrap();
    assert_eq!(
        schema.fingerprint(),
        reordered.compile().unwrap().fingerprint()
    );
}

pub(super) fn request(input: ManagedInterfaceInput) -> ManagedInterfaceHostFrame {
    ManagedInterfaceHostFrame {
        protocol: MANAGED_INTERFACE_PROTOCOL_V1.into(),
        call_id: "call-1".into(),
        handler: "audit".into(),
        interface_id: "shipments.create".into(),
        interface_version: "1".into(),
        input,
        context: ManagedHookHostContext {
            invocation: ManagedHookInvocation {
                invocation_id: "invocation-1".into(),
                registry_fingerprint: "registry-1".into(),
                graph_fingerprint: "graph-1".into(),
                authority_revision: 1,
            },
            execution_identity: ManagedExecutionIdentity::new(
                ManagedInstallationId::new("installation").unwrap(),
                ManagedWorkspaceId::new("workspace").unwrap(),
                ContributionId::new("auditor").unwrap(),
                ManagedArtifactFingerprint::from_bytes(b"artifact"),
                ManagedBindingFingerprint::from_bytes(b"binding"),
            ),
            generation: std::num::NonZeroU64::new(1).unwrap(),
            deadline_unix_ms: 1000,
            actor_id: None,
        },
    }
}

#[test]
fn root_2014_managed_observers_cannot_deny_patch_or_replace_result() {
    let view = ManagedInterfaceView {
        contract: contract(),
        value: json!({"shipment_id":"s1","item_count":2}),
    };
    for input in [
        ManagedInterfaceInput::Authorization {
            input: view.clone(),
        },
        ManagedInterfaceInput::Admission {
            input: view.clone(),
        },
        ManagedInterfaceInput::Before {
            input: view.clone(),
        },
        ManagedInterfaceInput::After {
            output: view.clone(),
        },
        ManagedInterfaceInput::Failure {
            classification: "target.failed".into(),
        },
        ManagedInterfaceInput::Completion {
            terminal: ManagedHookTerminal::Cancelled,
        },
    ] {
        let host = request(input);
        host.validate().unwrap();
        let veto = matches!(
            host.input.phase(),
            HookPhase::Authorization | HookPhase::Admission
        );
        let mut response = ManagedInterfaceWorkerFrame {
            protocol: MANAGED_INTERFACE_PROTOCOL_V1.into(),
            call_id: host.call_id.clone(),
            phase: host.input.phase(),
            outcome: ManagedHookOutcome::Deny {
                classification: "policy.denied".into(),
            },
        };
        assert_eq!(response.validate_for(&host).is_ok(), veto);
        response.outcome = if veto {
            ManagedHookOutcome::Continue
        } else {
            ManagedHookOutcome::Observed
        };
        response.validate_for(&host).unwrap();
        for field in [
            "actor_id",
            "workspace_id",
            "execution_identity",
            "patch",
            "result",
        ] {
            let mut wire = serde_json::to_value(&response).unwrap();
            wire[field] = json!("forged");
            assert!(serde_json::from_value::<ManagedInterfaceWorkerFrame>(wire).is_err());
        }
        let mut wire = serde_json::to_value(&response).unwrap();
        wire["outcome"]["result"] = json!("forged");
        assert!(serde_json::from_value::<ManagedInterfaceWorkerFrame>(wire).is_err());
        response.call_id = "another-invocation".into();
        assert!(response.validate_for(&host).is_err());
    }
}
