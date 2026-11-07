//! Finite synthetic managed-service peer. All authorization and storage remain in the Host.
use base64::{engine::general_purpose::STANDARD, Engine};
use runtime_extension_sdk::{
    serve_duplex, CapabilityStdioRequest, MultiplexEmitter, MultiplexHostService, MultiplexInput,
    MultiplexResourceLease, PluginDataFilter, PluginDataFilterOperator, PluginDataOperation,
    PluginDataOperationResult, PluginDataOrder, PluginDataOrderDirection, PluginDataPage,
    PluginDataRequest, PluginDataResponse, PluginDataTarget, PluginDataValue,
};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    io::Write,
    rc::Rc,
};

struct Trace(&'static str);
fn trace(event: &str) {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(exe.with_extension("trace"))
        {
            let _ = writeln!(file, "{event}");
        }
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        trace(self.0);
    }
}
type Sessions = Rc<RefCell<BTreeMap<String, (String, MultiplexResourceLease)>>>;
fn target() -> PluginDataTarget {
    PluginDataTarget::OwnedCollection {
        collection_code: "records".into(),
    }
}
fn filter(record: &str) -> Vec<PluginDataFilter> {
    vec![PluginDataFilter {
        field: "id".into(),
        operator: PluginDataFilterOperator::Equal,
        value: Some(PluginDataValue::Uuid(record.into())),
    }]
}
async fn data(
    emitter: &MultiplexEmitter,
    operations: Vec<PluginDataOperation>,
) -> Result<Vec<PluginDataOperationResult>, String> {
    trace("plugin_data_callback_start");
    let response = emitter
        .callback(
            MultiplexHostService::PluginDataV1,
            serde_json::to_value(PluginDataRequest {
                idempotency_key: None,
                operations,
            })
            .unwrap(),
        )
        .await
        .map_err(|_| {
            trace("plugin_data_callback_transport_error");
            "fixture_callback_transport".to_owned()
        })?;
    if response.get("error").is_some() {
        // Codes are Host classifications; never print callback payloads or credentials.
        let code = response
            .pointer("/error/code")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        trace(&format!("plugin_data_callback_error:{code}"));
        return Err("fixture_data_denied".into());
    }
    trace("plugin_data_callback_ok");
    let response: PluginDataResponse = serde_json::from_value(response["result"].clone())
        .map_err(|_| "fixture_callback_contract".to_owned())?;
    Ok(response.results)
}
fn rows(result: &PluginDataOperationResult) -> Vec<Value> {
    let PluginDataOperationResult::Rows { rows } = result else {
        return vec![];
    };
    rows.iter().map(|r|json!({"id":match &r.values["id"] {PluginDataValue::Uuid(s)=>s,_=>panic!("UUID row identity")},"name":match &r.values["name"] {PluginDataValue::String(s)=>s,_=>panic!("text field")}})).collect()
}
async fn execute(
    request: CapabilityStdioRequest,
    emitter: MultiplexEmitter,
    mut input: MultiplexInput,
    sessions: Sessions,
    sequence: Rc<Cell<u64>>,
) -> Result<Value, String> {
    let handler = request.input["handler"].as_str().ok_or("fixture_handler")?;
    let payload = &request.input["input_payload"];
    let record = payload
        .pointer("/path/record_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    trace(handler);
    match handler {
        "list" | "unmapped" => {
            let limit = payload
                .pointer("/query/limit")
                .and_then(Value::as_u64)
                .unwrap_or(100) as u32;
            let offset = payload
                .pointer("/query/offset")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let result = data(
                &emitter,
                vec![
                    PluginDataOperation::Find {
                        target: target(),
                        fields: vec!["id".into(), "name".into()],
                        filters: vec![],
                        order: vec![PluginDataOrder {
                            field: "name".into(),
                            direction: PluginDataOrderDirection::Ascending,
                        }],
                        page: PluginDataPage { limit, offset },
                    },
                    PluginDataOperation::Count {
                        target: target(),
                        filters: vec![],
                    },
                ],
            )
            .await?;
            let PluginDataOperationResult::Count { count } = result[1] else {
                return Err("fixture_count".into());
            };
            Ok(json!({"items":rows(&result[0]),"total":count}))
        }
        "create" => {
            let result = data(
                &emitter,
                vec![PluginDataOperation::Insert {
                    target: target(),
                    values: BTreeMap::from([(
                        "name".into(),
                        PluginDataValue::String(
                            payload["body"]["name"]
                                .as_str()
                                .ok_or("fixture_name")?
                                .into(),
                        ),
                    )]),
                }],
            )
            .await?;
            Ok(json!({"result":result}))
        }
        "get" | "resource" => {
            let result = data(
                &emitter,
                vec![PluginDataOperation::Find {
                    target: target(),
                    fields: vec!["id".into(), "name".into()],
                    filters: filter(record),
                    order: vec![],
                    page: PluginDataPage {
                        limit: 1,
                        offset: 0,
                    },
                }],
            )
            .await?;
            let row = rows(&result[0])
                .into_iter()
                .next()
                .ok_or("fixture_record_absent")?;
            if handler == "resource" {
                // Include non-UTF-8 bytes so import replay must preserve a real binary payload.
                let mut bytes = vec![0, 0xff];
                bytes.extend_from_slice(row["name"].as_str().ok_or("fixture_name")?.as_bytes());
                Ok(json!({
                    "metadata":{"record_id":row["id"]},
                    "resource":{"uri":format!("fixture://record/{record}"),"mime_type":"text/plain","text":row["name"]},
                    "binary_resource":{"uri":format!("fixture://record/{record}/content.bin"),"mime_type":"application/octet-stream","blob":STANDARD.encode(bytes)}
                }))
            } else {
                Ok(row)
            }
        }
        "update" => Ok(
            json!({"result":data(&emitter,vec![PluginDataOperation::Update {target:target(),filters:filter(record),values:BTreeMap::from([("name".into(),PluginDataValue::String(payload["body"]["name"].as_str().ok_or("fixture_name")?.into()))])}]).await?}),
        ),
        "delete" => Ok(
            json!({"result":data(&emitter,vec![PluginDataOperation::Delete {target:target(),filters:filter(record)}]).await?}),
        ),
        "open" => {
            sequence.set(sequence.get() + 1);
            let session = format!("fixture-session-{}", sequence.get());
            let lease = emitter
                .retain_resource()
                .await
                .map_err(|_| "fixture_retention".to_owned())?;
            sessions
                .borrow_mut()
                .insert(session.clone(), (record.into(), lease));
            Ok(json!({"session_id":session}))
        }
        "command" => {
            let session = payload["path"]["session_id"]
                .as_str()
                .ok_or("fixture_session")?;
            let all = sessions.borrow();
            let (record, _) = all.get(session).ok_or("fixture_session_absent")?;
            Ok(json!({"record_id":record,"executed":true}))
        }
        "stream" => {
            let _cleanup = Trace("stream_cleanup");
            emitter
                .event(json!({"ready":true,"record_id":record}))
                .await
                .map_err(|_| "fixture_stream".to_owned())?;
            while input.recv().await.is_some() {}
            Ok(json!({"closed":true}))
        }
        _ => Err("fixture_unknown_handler".into()),
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sessions = Rc::new(RefCell::new(BTreeMap::new()));
    let sequence = Rc::new(Cell::new(0));
    serve_duplex(move |request, emitter, input| {
        let sessions = sessions.clone();
        let sequence = sequence.clone();
        async move {
            let outcome = match serde_json::from_value::<CapabilityStdioRequest>(request) {
                Ok(request) => execute(request, emitter, input, sessions, sequence).await,
                Err(_) => Err("fixture_input_contract".into()),
            };
            match outcome {
                Ok(result) => json!({"ok":true,"result":result}),
                Err(message) => json!({"ok":false,"result":null,"error":{"message":message}}),
            }
        }
    })
    .await?;
    Ok(())
}
