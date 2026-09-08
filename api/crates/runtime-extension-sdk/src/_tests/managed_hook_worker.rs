//! Real process fixture and minimal author example for Root #2007 AC-003/004.
//! Build once with `cargo build -p runtime-extension-sdk --example managed_hook_worker`.
use runtime_extension_sdk::{serve_managed_hook, ManagedCreateHookInput, ManagedHookOutcome};
use std::io::{Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = Vec::new();
    std::io::stdin()
        .take((extension_contracts::MANAGED_HOOK_MAX_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut raw)?;
    let request: extension_contracts::ManagedHookHostFrame = serde_json::from_slice(&raw)?;
    // Explicit malicious peers exercise host validation; ordinary exchanges always use the SDK.
    if let Some(attack) = request.handler.strip_prefix("attack.") {
        let mut response = serde_json::json!({"protocol": extension_contracts::MANAGED_HOOK_PROTOCOL_V1, "call_id":request.call_id, "phase":request.input.phase(), "outcome":{"decision":"continue"}});
        match attack {
            "identity" => response["actor_id"] = serde_json::json!("forged"),
            "patch" => response["outcome"]["patch"] = serde_json::json!({"code":"forged"}),
            "correlation" => response["call_id"] = serde_json::json!("forged"),
            "observer_deny" => {
                response["outcome"] =
                    serde_json::json!({"decision":"deny","classification":"denied"})
            }
            "flood" => {
                std::io::stdout().write_all(&vec![
                    b'x';
                    extension_contracts::MANAGED_HOOK_MAX_FRAME_BYTES
                        + 1
                ])?;
                std::io::stdout().flush()?;
                std::thread::sleep(std::time::Duration::from_secs(30));
                return Ok(());
            }
            _ => return Err("unknown fixture attack".into()),
        }
        serde_json::to_writer(std::io::stdout(), &response)?;
        return Ok(());
    }
    serve_managed_hook(raw.as_slice(), std::io::stdout().lock(), |frame| {
        match frame.handler.as_str() {
            "sleep" => {
                std::fs::write(
                    std::env::current_exe().unwrap().with_extension("pid"),
                    std::process::id().to_string(),
                )
                .unwrap();
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            "crash" => std::process::exit(7),
            "verify_context" => {
                let context = &frame.context;
                assert_eq!(context.invocation.invocation_id, "invocation-1");
                assert_eq!(context.invocation.registry_fingerprint, "registry-g1");
                assert_eq!(context.invocation.graph_fingerprint, "graph-g1");
                assert_eq!(context.invocation.authority_revision, 7);
                assert_eq!(
                    context.execution_identity.workspace_id().as_str(),
                    "workspace-1"
                );
                assert_eq!(
                    context.execution_identity.installation_id().as_str(),
                    "installation-1"
                );
                assert_eq!(
                    context.execution_identity.artifact_fingerprint(),
                    &extension_contracts::ManagedArtifactFingerprint::from_bytes(b"artifact-1")
                );
                assert_eq!(
                    context.execution_identity.binding_fingerprint(),
                    &extension_contracts::ManagedBindingFingerprint::from_bytes(b"binding-1")
                );
                assert_eq!(context.actor_id.as_deref(), Some("actor-1"));
                assert!(context.generation.get() > 0);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;
                assert!(context.deadline_unix_ms > now);
                assert!(frame.call_id.starts_with("hook-"));
            }
            _ => {}
        }
        if let ManagedCreateHookInput::Before { create } = &frame.input {
            if create.code == "identify" {
                return ManagedHookOutcome::Deny {
                    classification: format!("fixture.{}", frame.handler),
                };
            }
            if create.code == "fixture_barrier" {
                let marker = std::env::current_exe().unwrap();
                std::fs::write(marker.with_extension("started"), "started").unwrap();
                while !marker.with_extension("release").is_file() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
        match &frame.input {
            ManagedCreateHookInput::Authorization { create }
            | ManagedCreateHookInput::Admission { create }
            | ManagedCreateHookInput::Before { create }
                if create.code == "denied" =>
            {
                ManagedHookOutcome::Deny {
                    classification: "fixture.denied".into(),
                }
            }
            input if input.is_observer() => ManagedHookOutcome::Observed,
            _ => ManagedHookOutcome::Continue,
        }
    })?;
    Ok(())
}
