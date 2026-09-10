//! Real process fixture and minimal author example for Root #2007 AC-003/004.
//! Build once with `cargo build -p runtime-extension-sdk --example managed_hook_worker`.
use runtime_extension_sdk::{serve_managed_hook, ManagedCreateHookInput, ManagedHookOutcome};
use std::io::{Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = Vec::new();
    std::io::stdin()
        .take((extension_contracts::MANAGED_INTERFACE_MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut raw)?;
    let protocol: serde_json::Value = serde_json::from_slice(&raw)?;
    match protocol["protocol"].as_str() {
        Some(extension_contracts::MANAGED_INTERFACE_PROTOCOL_V1) => return serve_interface(&raw),
        Some(extension_contracts::MANAGED_INTERFACE_PROTOCOL_V2) => return serve_reference(&raw),
        Some(extension_contracts::MANAGED_HOOK_PROTOCOL_V1) => {}
        _ => return Err("unsupported explicit fixture protocol".into()),
    }
    let request: extension_contracts::ManagedHookHostFrame = serde_json::from_slice(&raw)?;
    if request.handler.starts_with("trace.") {
        let executable = std::env::current_exe()?;
        let mut trace = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(executable.with_extension("trace"))?;
        serde_json::to_writer(&mut trace, &request)?;
        trace.write_all(b"\n")?;
        trace.flush()?;
    }
    // Explicit malicious peers exercise host validation; ordinary exchanges always use the SDK.
    if let Some(attack) = request.handler.strip_prefix("attack.") {
        let mut response = serde_json::json!({"protocol": extension_contracts::MANAGED_HOOK_PROTOCOL_V1, "call_id":request.call_id, "phase":request.input.phase(), "outcome":{"decision":"continue"}});
        match attack {
            "identity" => response["actor_id"] = serde_json::json!("forged"),
            "patch" => response["outcome"]["patch"] = serde_json::json!({"code":"forged"}),
            "observed_patch" => {
                response["outcome"] =
                    serde_json::json!({"decision":"observed", "patch":{"code":"forged"}})
            }
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
        if let Some(phase) = frame.handler.strip_prefix("trace.") {
            let executable = std::env::current_exe().unwrap();
            let mode =
                std::fs::read_to_string(executable.with_extension("mode")).unwrap_or_default();
            if mode.trim() == format!("deny.{phase}") {
                return ManagedHookOutcome::Deny {
                    classification: "fixture.denied".into(),
                };
            }
            if mode.trim() == format!("fail.{phase}") {
                return ManagedHookOutcome::Failed {
                    classification: "fixture.failed".into(),
                };
            }
            if mode.trim() == format!("timeout.{phase}") {
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            if mode.trim() == format!("barrier.{phase}") {
                std::fs::write(executable.with_extension("started"), "started").unwrap();
                while !executable.with_extension("release").is_file() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
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

/// Versioned generic peer exercises the same executable/process boundary as legacy fixtures.
fn serve_interface(raw: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use extension_contracts::{
        HookPhase, ManagedInterfaceHostFrame, MANAGED_INTERFACE_PROTOCOL_V1,
    };
    let request: ManagedInterfaceHostFrame = serde_json::from_slice(raw)?;
    let executable = std::env::current_exe()?;
    if request.handler.starts_with("trace.") {
        let mut trace = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(executable.with_extension("trace"))?;
        serde_json::to_writer(&mut trace, &request)?;
        trace.write_all(b"\n")?;
    }
    if let Some(attack) = request.handler.strip_prefix("attack.") {
        let mut response = serde_json::json!({"protocol":MANAGED_INTERFACE_PROTOCOL_V1,
            "call_id":request.call_id,"phase":request.input.phase(),"outcome":{"decision":"observed"}});
        match attack {
            "identity" => response["actor_id"] = serde_json::json!("forged"),
            "patch" | "observed_patch" => {
                response["outcome"]["patch"] = serde_json::json!({"code":"forged"})
            }
            "correlation" => response["call_id"] = serde_json::json!("forged"),
            "observer_deny" => {
                response["outcome"] =
                    serde_json::json!({"decision":"deny","classification":"denied"})
            }
            "phase" => response["phase"] = serde_json::json!("authorization"),
            _ => return Err("unknown interface fixture attack".into()),
        }
        serde_json::to_writer(std::io::stdout(), &response)?;
        return Ok(());
    }
    runtime_extension_sdk::serve_managed_interface_hook(raw, std::io::stdout().lock(), |frame| {
        let phase = frame
            .handler
            .strip_prefix("trace.")
            .unwrap_or(&frame.handler);
        let mode = std::fs::read_to_string(executable.with_extension("mode")).unwrap_or_default();
        if mode.trim() == format!("deny.{phase}") {
            return ManagedHookOutcome::Deny {
                classification: "fixture.denied".into(),
            };
        }
        if mode.trim() == format!("fail.{phase}") {
            return ManagedHookOutcome::Failed {
                classification: "fixture.failed".into(),
            };
        }
        if frame.handler == "sleep" || mode.trim() == format!("timeout.{phase}") {
            std::fs::write(
                executable.with_extension("pid"),
                std::process::id().to_string(),
            )
            .unwrap();
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        if mode.trim() == format!("barrier.{phase}") {
            std::fs::write(executable.with_extension("started"), "started").unwrap();
            while !executable.with_extension("release").is_file() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        if frame.handler == "crash" {
            std::process::exit(7);
        }
        match frame.input.phase() {
            HookPhase::Authorization | HookPhase::Admission => ManagedHookOutcome::Continue,
            _ => ManagedHookOutcome::Observed,
        }
    })?;
    Ok(())
}

fn serve_reference(raw: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use extension_contracts::{
        HookPhase, ManagedInterfaceReferenceHostFrame, MANAGED_INTERFACE_PROTOCOL_V2,
    };
    let request: ManagedInterfaceReferenceHostFrame = serde_json::from_slice(raw)?;
    let executable = std::env::current_exe()?;
    if request.handler.starts_with("trace.") {
        let mut trace = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(executable.with_extension("trace"))?;
        serde_json::to_writer(&mut trace, &request)?;
        trace.write_all(b"\n")?;
        let mut pids = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(executable.with_extension("pids"))?;
        writeln!(pids, "{}", std::process::id())?;
    }
    if let Some(attack) = request.handler.strip_prefix("attack.") {
        let mut response = serde_json::json!({"protocol":MANAGED_INTERFACE_PROTOCOL_V2,
            "call_id":request.call_id,"phase":request.input.phase(),"outcome":{"decision":"observed"}});
        match attack {
            "identity" => response["actor_id"] = serde_json::json!("forged"),
            "patch" | "observed_patch" => {
                response["outcome"]["patch"] = serde_json::json!({"code":"forged"})
            }
            "correlation" => response["call_id"] = serde_json::json!("forged"),
            "observer_deny" => {
                response["outcome"] =
                    serde_json::json!({"decision":"deny","classification":"denied"})
            }
            "phase" => response["phase"] = serde_json::json!("authorization"),
            "protocol" => {
                response["protocol"] =
                    serde_json::json!(extension_contracts::MANAGED_INTERFACE_PROTOCOL_V1)
            }
            "fingerprint" => response["schema_fingerprint"] = serde_json::json!("forged"),
            "flood" => {
                std::io::stdout().write_all(&vec![
                    b'x';
                    extension_contracts::MANAGED_INTERFACE_MAX_REPLY_BYTES
                        + 1
                ])?;
                return Ok(());
            }
            _ => return Err("unknown interface fixture attack".into()),
        }
        serde_json::to_writer(std::io::stdout(), &response)?;
        return Ok(());
    }
    runtime_extension_sdk::serve_managed_interface_reference_hook(
        raw,
        std::io::stdout().lock(),
        |frame| {
            let phase = frame
                .handler
                .strip_prefix("trace.")
                .unwrap_or(&frame.handler);
            let mode =
                std::fs::read_to_string(executable.with_extension("mode")).unwrap_or_default();
            if mode.trim() == format!("deny.{phase}") {
                return ManagedHookOutcome::Deny {
                    classification: "fixture.denied".into(),
                };
            }
            if mode.trim() == format!("fail.{phase}") {
                return ManagedHookOutcome::Failed {
                    classification: "fixture.failed".into(),
                };
            }
            if frame.handler == "sleep" || mode.trim() == format!("timeout.{phase}") {
                std::fs::write(
                    executable.with_extension("pid"),
                    std::process::id().to_string(),
                )
                .unwrap();
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
            if mode.trim() == format!("barrier.{phase}") {
                std::fs::write(executable.with_extension("started"), "started").unwrap();
                while !executable.with_extension("release").is_file() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            if frame.handler == "crash" {
                std::process::exit(7);
            }
            match frame.input.phase() {
                HookPhase::Authorization | HookPhase::Admission => ManagedHookOutcome::Continue,
                _ => ManagedHookOutcome::Observed,
            }
        },
    )?;
    Ok(())
}
