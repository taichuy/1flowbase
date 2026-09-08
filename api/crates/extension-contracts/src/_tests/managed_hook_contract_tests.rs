//! Root #2007 AC-003/004: finite phase/outcome matrix and worker impersonation negatives.
use crate::*;
use serde_json::json;

fn request(input: ManagedCreateHookInput) -> ManagedHookHostFrame {
    ManagedHookHostFrame {
        protocol: MANAGED_HOOK_PROTOCOL_V1.into(),
        call_id: "host-call".into(),
        handler: "hook".into(),
        context: ManagedHookHostContext {
            invocation: ManagedHookInvocation {
                invocation_id: "invocation".into(),
                registry_fingerprint: "registry-g1".into(),
                graph_fingerprint: "graph-g1".into(),
                authority_revision: 7,
            },
            execution_identity: ManagedExecutionIdentity::new(
                ManagedInstallationId::new("installation").unwrap(),
                ManagedWorkspaceId::new("workspace").unwrap(),
                ContributionId::new("hook").unwrap(),
                ManagedArtifactFingerprint::from_bytes(b"v1"),
                ManagedBindingFingerprint::from_bytes(b"before"),
            ),
            generation: std::num::NonZeroU64::new(1).unwrap(),
            deadline_unix_ms: 1_000,
            actor_id: Some("actor".into()),
        },
        input,
    }
}

fn create() -> ManagedCreateView {
    ManagedCreateView {
        code: "model".into(),
        template_provider: "core".into(),
        template_code: "general".into(),
        template_version: "1".into(),
    }
}

#[test]
fn root_2007_ac_003_004_hook_transport_phase_and_identity_contract() {
    for input in [
        ManagedCreateHookInput::Authorization { create: create() },
        ManagedCreateHookInput::Admission { create: create() },
        ManagedCreateHookInput::Before { create: create() },
        ManagedCreateHookInput::After {
            model_id: "model-id".into(),
        },
        ManagedCreateHookInput::Failure {
            classification: "failed".into(),
        },
        ManagedCreateHookInput::Completion {
            terminal: ManagedHookTerminal::Cancelled,
        },
    ] {
        let host = request(input);
        host.validate().unwrap();
        let mut response = ManagedHookWorkerFrame {
            protocol: MANAGED_HOOK_PROTOCOL_V1.into(),
            call_id: host.call_id.clone(),
            phase: host.input.phase(),
            outcome: ManagedHookOutcome::Continue,
        };
        assert_eq!(
            response.validate_for(&host).is_ok(),
            !host.input.is_observer()
        );
        response.outcome = ManagedHookOutcome::Observed;
        assert_eq!(
            response.validate_for(&host).is_ok(),
            host.input.is_observer()
        );
        response.outcome = ManagedHookOutcome::Deny {
            classification: "plugin.denied".into(),
        };
        assert_eq!(
            response.validate_for(&host).is_ok(),
            !host.input.is_observer()
        );
        response.outcome = ManagedHookOutcome::Failed {
            classification: "plugin.failed".into(),
        };
        response.validate_for(&host).unwrap();
        response.call_id = "forged".into();
        assert!(response.validate_for(&host).is_err());
        for field in [
            "actor_id",
            "workspace_id",
            "execution_identity",
            "generation",
            "patch",
            "code",
            "template_code",
        ] {
            let mut raw = serde_json::to_value(&response).unwrap();
            raw[field] = json!("forged");
            assert!(
                serde_json::from_value::<ManagedHookWorkerFrame>(raw).is_err(),
                "{field}"
            );
        }
    }
    // Exercise each outcome's raw map: validating an already-decoded unit value would
    // miss serde discarding attacker-controlled fields inside the outcome object (QA1 B03).
    for outcome in [
        ManagedHookOutcome::Continue,
        ManagedHookOutcome::Observed,
        ManagedHookOutcome::Deny {
            classification: "plugin.denied".into(),
        },
        ManagedHookOutcome::Failed {
            classification: "plugin.failed".into(),
        },
    ] {
        let encoded = serde_json::to_value(&outcome).unwrap();
        assert_eq!(
            serde_json::from_value::<ManagedHookOutcome>(encoded.clone()).unwrap(),
            outcome
        );
        let expected_keys = if matches!(
            outcome,
            ManagedHookOutcome::Continue | ManagedHookOutcome::Observed
        ) {
            vec!["decision"]
        } else {
            vec!["classification", "decision"]
        };
        let mut keys = encoded
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(keys, expected_keys, "normal wire shape: {outcome:?}");
        for field in [
            "actor_id",
            "workspace_id",
            "execution_identity",
            "generation",
            "patch",
            "code",
            "template_code",
            "call_id",
        ] {
            for value in [json!("forged"), json!(null), json!({"code":"forged"})] {
                let mut raw = encoded.clone();
                raw[field] = value;
                assert!(
                    serde_json::from_value::<ManagedHookOutcome>(raw.clone()).is_err(),
                    "outcome={outcome:?}, extra={field}"
                );
                let frame = json!({"protocol":MANAGED_HOOK_PROTOCOL_V1, "call_id":"host-call", "phase":"before", "outcome":raw});
                assert!(
                    serde_json::from_value::<ManagedHookWorkerFrame>(frame).is_err(),
                    "frame outcome={outcome:?}, extra={field}"
                );
            }
        }
        if matches!(
            outcome,
            ManagedHookOutcome::Continue | ManagedHookOutcome::Observed
        ) {
            let mut raw = encoded;
            raw["classification"] = json!("unexpected");
            assert!(
                serde_json::from_value::<ManagedHookOutcome>(raw).is_err(),
                "unit outcome={outcome:?} must have no payload"
            );
        }
    }
    for raw in [
        json!({"phase":"authentication_adapter"}),
        json!({"phase":"before", "create": {"code":"x", "template_provider":"core", "template_code":"general", "template_version":"1", "actor_id":"forged"}}),
        json!({"phase":"before", "create": create(), "patch":{"code":"forged"}}),
    ] {
        assert!(serde_json::from_value::<ManagedCreateHookInput>(raw).is_err());
    }
}
