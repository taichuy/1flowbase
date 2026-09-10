//! Root #2007 AC-003/004: author API validates typed host requests and correlates its own output.
use crate::{serve_managed_hook, ManagedHookOutcome};
use extension_contracts::*;
use std::io::Cursor;

#[test]
fn root_2007_ac_003_004_hook_transport_sdk() {
    let host = serde_json::json!({
        "protocol":MANAGED_HOOK_PROTOCOL_V1,"call_id":"host-call", "handler":"completion",
        "context": {
            "invocation":{"invocation_id":"call", "registry_fingerprint":"registry", "graph_fingerprint":"graph", "authority_revision":2},
            "execution_identity": ManagedExecutionIdentity::new(ManagedInstallationId::new("installation").unwrap(), ManagedWorkspaceId::new("workspace").unwrap(), ContributionId::new("completion").unwrap(), ManagedArtifactFingerprint::from_bytes(b"v1"), ManagedBindingFingerprint::from_bytes(b"binding")),
            "generation":1, "deadline_unix_ms":10, "actor_id":null
        },
        "input":{"phase":"completion", "terminal":"succeeded"}
    });
    let raw = serde_json::to_vec(&host).unwrap();
    let mut output = Vec::new();
    serve_managed_hook(Cursor::new(&raw), &mut output, |_| {
        ManagedHookOutcome::Observed
    })
    .unwrap();
    let response: ManagedHookWorkerFrame = serde_json::from_slice(&output).unwrap();
    response
        .validate_for(&serde_json::from_value(host).unwrap())
        .unwrap();
    assert_eq!(response.call_id, "host-call");
    output.clear();
    assert!(serve_managed_hook(Cursor::new(&raw), &mut output, |_| {
        ManagedHookOutcome::Continue
    })
    .is_err());
    assert!(output.is_empty());
    assert!(serve_managed_hook(
        Cursor::new(vec![b'x'; MANAGED_HOOK_MAX_FRAME_BYTES + 1]),
        Vec::new(),
        |_| panic!("oversized input must not reach author")
    )
    .is_err());
}

#[test]
fn root_2014_interface_sdk_correlates_and_rejects_observer_veto() {
    let host = serde_json::json!({
        "protocol":MANAGED_INTERFACE_PROTOCOL_V1,"call_id":"host-call", "handler":"completion",
        "interface_id":"host_infrastructure.providers.view", "interface_version":"1",
        "context": {
            "invocation":{"invocation_id":"call", "registry_fingerprint":"registry", "graph_fingerprint":"graph", "authority_revision":2},
            "execution_identity": ManagedExecutionIdentity::new(ManagedInstallationId::new("installation").unwrap(), ManagedWorkspaceId::new("workspace").unwrap(), ContributionId::new("completion").unwrap(), ManagedArtifactFingerprint::from_bytes(b"v1"), ManagedBindingFingerprint::from_bytes(b"binding")),
            "generation":1, "deadline_unix_ms":10, "actor_id":null
        },
        "input":{"phase":"completion", "terminal":"succeeded"}
    });
    let raw = serde_json::to_vec(&host).unwrap();
    let mut output = Vec::new();
    crate::serve_managed_interface_hook(Cursor::new(&raw), &mut output, |_| {
        ManagedHookOutcome::Observed
    })
    .unwrap();
    let response: ManagedInterfaceWorkerFrame = serde_json::from_slice(&output).unwrap();
    response
        .validate_for(&serde_json::from_value(host.clone()).unwrap())
        .unwrap();
    assert_eq!(response.call_id, "host-call");
    output.clear();
    assert!(
        crate::serve_managed_interface_hook(Cursor::new(&raw), &mut output, |_| {
            ManagedHookOutcome::Deny {
                classification: "denied".into(),
            }
        })
        .is_err()
    );
    assert!(output.is_empty());
    let mut forged = host;
    forged["protocol"] = serde_json::json!("unknown/version");
    assert!(crate::serve_managed_interface_hook(
        Cursor::new(serde_json::to_vec(&forged).unwrap()),
        Vec::new(),
        |_| panic!("invalid frame reached author")
    )
    .is_err());
}

/// Root #2014 AC-015/017/018: old public schema consumer and independent v2 author entry.
#[test]
fn root_2014_r3_probe_v1_schema_consumer_and_v2_reference() {
    let contract = ManagedProjectionContract {
        contract_id: "sdk.values".into(),
        contract_version: "1".into(),
        schema: serde_json::json!({"type":"array","items":{"type":"string","maxLength":16384},"maxItems":256}),
    };
    let compiled = contract.compile_for_registration().unwrap();
    let context = serde_json::json!({
        "invocation":{"invocation_id":"call", "registry_fingerprint":"registry", "graph_fingerprint":"graph", "authority_revision":2},
        "execution_identity":ManagedExecutionIdentity::new(ManagedInstallationId::new("installation").unwrap(), ManagedWorkspaceId::new("workspace").unwrap(), ContributionId::new("before").unwrap(), ManagedArtifactFingerprint::from_bytes(b"artifact"), ManagedBindingFingerprint::from_bytes(b"binding")),
        "generation":1,"deadline_unix_ms":10,"actor_id":null
    });
    let v1 = serde_json::json!({"protocol":MANAGED_INTERFACE_PROTOCOL_V1,"call_id":"same-call","handler":"before",
        "interface_id":"sdk.values","interface_version":"1","context":context,
        "input":{"phase":"before","input":{"contract":contract,"value":["payload"]}}});
    let mut output = Vec::new();
    crate::serve_managed_interface_hook(
        Cursor::new(serde_json::to_vec(&v1).unwrap()),
        &mut output,
        |frame| {
            let ManagedInterfaceInput::Before { input } = &frame.input else {
                panic!("wrong phase")
            };
            assert_eq!(input.contract.schema, contract.schema);
            assert_eq!(input.value, serde_json::json!(["payload"]));
            ManagedHookOutcome::Observed
        },
    )
    .unwrap();
    let old: ManagedInterfaceWorkerFrame = serde_json::from_slice(&output).unwrap();
    let value = serde_json::json!([
        "x".repeat(16384),
        "x".repeat(16384),
        "x".repeat(16384),
        "x".repeat(16368),
        ""
    ]);
    assert_eq!(
        serde_json::to_vec(&value).unwrap().len(),
        MANAGED_INTERFACE_MAX_PROJECTION_BYTES
    );
    let mut v2 = v1.clone();
    v2["protocol"] = MANAGED_INTERFACE_PROTOCOL_V2.into();
    v2["input"]["input"] = serde_json::json!({"contract":compiled.reference(),"value":value});
    assert!(serde_json::to_vec(&v2).unwrap().len() > MANAGED_INTERFACE_MAX_FRAME_BYTES);
    let mut served = false;
    output.clear();
    crate::serve_managed_interface_reference_hook(
        Cursor::new(serde_json::to_vec(&v2).unwrap()),
        &mut output,
        |frame| {
            served = true;
            let ManagedInterfaceReferenceInput::Before { input } = &frame.input else {
                panic!("wrong phase")
            };
            assert_eq!(input.contract, compiled.reference());
            assert_eq!(input.value, value);
            assert!(serde_json::to_value(input).unwrap()["contract"]
                .get("schema")
                .is_none());
            ManagedHookOutcome::Observed
        },
    )
    .unwrap();
    assert!(served);
    let reply: ManagedInterfaceReferenceWorkerFrame = serde_json::from_slice(&output).unwrap();
    reply
        .validate_for(&serde_json::from_value(v2.clone()).unwrap())
        .unwrap();
    assert_eq!(reply.outcome, old.outcome);
    assert_eq!(reply.call_id, old.call_id);
    assert_eq!(reply.phase, old.phase);
    assert!(output.len() <= MANAGED_INTERFACE_MAX_REPLY_BYTES);
    for mut invalid in [v1.clone(), v2.clone()] {
        // Each source is driven through the real selected SDK entry, never parse fallback.
        if invalid["protocol"] == MANAGED_INTERFACE_PROTOCOL_V1 {
            assert!(crate::serve_managed_interface_reference_hook(
                Cursor::new(serde_json::to_vec(&invalid).unwrap()),
                Vec::new(),
                |_| panic!("wrong protocol")
            )
            .is_err());
        } else {
            invalid["input"]["input"]["contract"]["schema_fingerprint"] =
                "not-a-fingerprint".into();
            assert!(crate::serve_managed_interface_reference_hook(
                Cursor::new(serde_json::to_vec(&invalid).unwrap()),
                Vec::new(),
                |_| panic!("bad reference")
            )
            .is_err());
        }
    }
    let mut invalid = v2.clone();
    invalid["input"]["input"]["value"][4] = "x".into();
    assert!(crate::serve_managed_interface_reference_hook(
        Cursor::new(serde_json::to_vec(&invalid).unwrap()),
        Vec::new(),
        |_| panic!("oversize value")
    )
    .is_err());
    invalid = v2.clone();
    invalid["input"]["input"]["contract"]["schema"] = contract.schema;
    assert!(crate::serve_managed_interface_reference_hook(
        Cursor::new(serde_json::to_vec(&invalid).unwrap()),
        Vec::new(),
        |_| panic!("schema injected")
    )
    .is_err());
    assert!(crate::serve_managed_interface_hook(
        Cursor::new(serde_json::to_vec(&v2).unwrap()),
        Vec::new(),
        |_| panic!("v2 reached old author")
    )
    .is_err());
    assert!(crate::serve_managed_interface_reference_hook(
        Cursor::new(vec![b' '; MANAGED_INTERFACE_MAX_REQUEST_BYTES + 1]),
        Vec::new(),
        |_| panic!("oversize request")
    )
    .is_err());
}
