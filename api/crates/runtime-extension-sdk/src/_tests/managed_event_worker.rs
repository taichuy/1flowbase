//! Real SDK process used by Root #2007 AC-005/007. No host identity is accepted in worker output.
use runtime_extension_sdk::{serve_managed_event, ManagedEventOutcome, ManagedEventPublication};
use std::io::{Read, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut raw = Vec::new();
    std::io::stdin()
        .take((extension_contracts::MANAGED_EVENT_MAX_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut raw)?;
    let request: extension_contracts::ManagedEventHostFrame = serde_json::from_slice(&raw)?;
    if let Some(attack) = request.handler.strip_prefix("attack.") {
        let mut response = serde_json::json!({"protocol":extension_contracts::MANAGED_EVENT_PROTOCOL_V2, "call_id":request.call_id, "result":{"outcome":"acknowledged"}});
        match attack {
            "identity" => response["publisher"] = "forged".into(),
            "workspace" => response["workspace_id"] = "foreign".into(),
            "correlation" => response["call_id"] = "foreign".into(),
            "version" => response["protocol"] = "1flowbase.managed-event/v1".into(),
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
    if request.handler == "fixture_barrier" {
        std::fs::write(executable.with_extension("started"), "started")?;
        while !executable.with_extension("release").is_file() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis() as i64;
            if now >= request.deadline_unix_ms {
                return Err("fixture event deadline expired".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    serve_managed_event(raw.as_slice(), std::io::stdout().lock(), |frame| {
        use extension_contracts::{PluginDataOperation, PluginDataTarget, PluginDataValue};
        let payload = &frame.delivery.payload;
        if frame.handler == "publish" {
            ManagedEventOutcome::Publish {
                publication: ManagedEventPublication {
                    contract_id: extension_contracts::MANAGED_PROCESSED_EVENT_ID.into(),
                    contract_version: "1".into(),
                    payload: serde_json::json!({"model_id":payload["model_id"],"status":"processed","result_reference":format!("processed_models/{}",payload["model_id"].as_str().unwrap())}),
                },
            }
        } else if frame.handler == "publish.shipment" {
            ManagedEventOutcome::Publish {
                publication: ManagedEventPublication {
                    contract_id: "orion.shipments.created".into(),
                    contract_version: "1".into(),
                    payload: serde_json::json!({"shipment_id":payload["model_id"],"destination_code":"NYC","item_count":3}),
                },
            }
        } else if frame.handler == "publish.audit" {
            ManagedEventOutcome::Publish {
                publication: ManagedEventPublication {
                    contract_id: "lyra.audit-log.recorded".into(),
                    contract_version: "1".into(),
                    payload: serde_json::json!({"audit_ref":payload["shipment_id"],"accepted":true,"route_code":payload["destination_code"]}),
                },
            }
        } else if frame.handler == "apply.audit" {
            ManagedEventOutcome::ApplyOwned {
                operations: vec![PluginDataOperation::Upsert {
                    target: PluginDataTarget::OwnedCollection {
                        collection_code: "audit_records".into(),
                    },
                    identity: [(
                        "audit_ref".into(),
                        PluginDataValue::String(payload["audit_ref"].as_str().unwrap().into()),
                    )]
                    .into_iter()
                    .collect(),
                    values: [
                        (
                            "accepted".into(),
                            PluginDataValue::Boolean(payload["accepted"].as_bool().unwrap()),
                        ),
                        (
                            "route_code".into(),
                            PluginDataValue::String(payload["route_code"].as_str().unwrap().into()),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }],
            }
        } else if frame.handler == "apply_processed" {
            ManagedEventOutcome::ApplyOwned {
                operations: vec![PluginDataOperation::Upsert {
                    target: PluginDataTarget::OwnedCollection {
                        collection_code: "processed_models".into(),
                    },
                    identity: [(
                        "model_id".into(),
                        PluginDataValue::Uuid(payload["model_id"].as_str().unwrap().into()),
                    )]
                    .into_iter()
                    .collect(),
                    values: [
                        ("status".into(), PluginDataValue::String("processed".into())),
                        (
                            "result_reference".into(),
                            payload["result_reference"]
                                .as_str()
                                .map(|s| PluginDataValue::String(s.into()))
                                .unwrap_or(PluginDataValue::Null),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }],
            }
        } else {
            ManagedEventOutcome::Acknowledged
        }
    })?;
    Ok(())
}
