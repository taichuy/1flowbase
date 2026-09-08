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
