//! Managed hooks receive counts only; source bodies and remote destinations stay private.
use super::*;
use crate::extension_bus::managed_projection as mp;
use crate::routes::application_public_api::agent_logs::event_projection as ep;
impl InterfaceContract for ExchangeInput {
    const CONTRACT_ID: &'static str = "console-application-log-exchange-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn event_projection_schema() -> Option<Value> {
        Some(ep::object_schema([
            ("application_id", json!({"type":"string","format":"uuid"})),
            ("action", json!({"type":"string","const":"otlp_import"})),
            ("batches", ep::array_schema(ep::batch_schema())),
        ]))
    }
    fn project_for_event(&self) -> Option<Value> {
        let Self::OtlpImport {
            application_id,
            body,
        } = self
        else {
            return None;
        };
        // Only parsed, typed source facts can become fields; arbitrary OTLP attributes stay private.
        let request =
            serde_json::from_value::<crate::agent_logs_protocol::ExportTraceServiceRequest>(
                body.otlp.clone(),
            )
            .ok()?;
        let batches = crate::agent_logs_protocol::traces_to_batches(&request).ok()?;
        Some(
            json!({"application_id":application_id,"action":"otlp_import","batches":batches.iter().map(ep::batch_value).collect::<Vec<_>>()}),
        )
    }
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([
            ("application_id", mp::text_schema()),
            ("action", mp::text_schema()),
            (
                "selected_items",
                mp::union_schema(vec![mp::count_schema(), json!({"type":"null"})]),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        let count = match self {
            Self::OtlpExport { body, .. } => {
                selection_count(&body.record_ids, &body.log_conversation_ids)
            }
            Self::OtlpImport { .. } => Value::Null,
        };
        Some(
            json!({"application_id":self.identity().0,"action":self.action(),"selected_items":count}),
        )
    }
}
impl InterfaceContract for ExchangeOutput {
    const CONTRACT_ID: &'static str = "console-application-log-exchange-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn event_projection_schema() -> Option<Value> {
        Some(ep::receipt_schema())
    }
    fn project_for_event(&self) -> Option<Value> {
        let Self::Imported(receipt) = self else {
            return None;
        };
        Some(
            json!({"accepted_events":receipt.accepted_events,"duplicate_events":receipt.duplicate_events,"record_ids":receipt.record_ids}),
        )
    }
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([
            ("action", mp::text_schema()),
            (
                "records",
                mp::union_schema(vec![mp::count_schema(), json!({"type":"null"})]),
            ),
            ("events", mp::count_schema()),
            ("duplicates", mp::count_schema()),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(match self {
            Self::Otlp(value) => {
                let events = value
                    .get("resourceSpans")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|resource| resource.get("scopeSpans").and_then(Value::as_array))
                    .flatten()
                    .filter_map(|scope| scope.get("spans").and_then(Value::as_array))
                    .map(Vec::len)
                    .sum::<usize>();
                json!({"action":"otlp_export","records":null,"events":events,"duplicates":0})
            }
            Self::Imported(receipt) => {
                json!({"action":"import","records":receipt.record_ids.len(),"events":receipt.accepted_events,"duplicates":receipt.duplicate_events})
            }
        })
    }
}

fn selection_count(records: &Option<Vec<Uuid>>, conversations: &Option<Vec<Uuid>>) -> Value {
    match (records, conversations) {
        (Some(ids), None) | (None, Some(ids)) if !ids.is_empty() => json!(ids.len()),
        _ => Value::Null,
    }
}
