//! Real SDK process used by Root #2007 AC-005. No host identity is accepted in worker output.
use runtime_extension_sdk::{
    serve_managed_event, ManagedEventOutcome, ManagedEventPublication, ManagedEventStatus,
};
use std::io::{Read, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = Vec::new();
    std::io::stdin()
        .take((extension_contracts::MANAGED_EVENT_MAX_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut raw)?;
    let request: extension_contracts::ManagedEventHostFrame = serde_json::from_slice(&raw)?;
    if let Some(attack) = request.handler.strip_prefix("attack.") {
        let mut response = serde_json::json!({"protocol":extension_contracts::MANAGED_EVENT_PROTOCOL_V1, "call_id":request.call_id, "result":{"outcome":"acknowledged"}});
        match attack {
            "identity" => response["publisher"] = "forged".into(),
            "workspace" => response["workspace_id"] = "foreign".into(),
            "correlation" => response["call_id"] = "foreign".into(),
            "version" => response["protocol"] = "1flowbase.managed-event/v2".into(),
            "flood" => {
                std::io::stdout().write_all(&vec![
                    b'x';
                    extension_contracts::MANAGED_EVENT_MAX_FRAME_BYTES
                        + 1
                ])?;
                std::io::stdout().flush()?;
                std::thread::sleep(std::time::Duration::from_secs(30));
                return Ok(());
            }
            _ => return Err("unknown attack".into()),
        }
        serde_json::to_writer(std::io::stdout(), &response)?;
        return Ok(());
    }
    let executable = std::env::current_exe()?;
    let mut trace = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(executable.with_extension("trace"))?;
    serde_json::to_writer(&mut trace, &request)?;
    trace.write_all(b"\n")?;
    trace.flush()?;
    serve_managed_event(raw.as_slice(), std::io::stdout().lock(), |frame| {
        if frame.handler == "publish" {
            let mut payload = frame.delivery.payload.clone();
            payload.status = ManagedEventStatus::Processed;
            payload.result_reference = Some(format!("processed_models/{}", payload.model_id));
            ManagedEventOutcome::Publish {
                publication: ManagedEventPublication {
                    contract_id: extension_contracts::MANAGED_PROCESSED_EVENT_ID.into(),
                    contract_version: "1".into(),
                    payload,
                },
            }
        } else {
            ManagedEventOutcome::Acknowledged
        }
    })?;
    Ok(())
}
