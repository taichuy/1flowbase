use super::*;
use crate::event_center::api_projection::{
    schema, to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::document_event::{self as de, query_fields as q};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::routes::host_infrastructure::MemoryEntryMetadataResponse;
// Reveal payloads, raw keys, opaque handles and unstructured entry metadata stay with the owner.
const ENTRY_HIDDEN: &[&str] = &["key", "entry_ref", "owner", "metadata"];
const REVEAL_HIDDEN: &[&str] = &["value", "value_preview"];
fn entry_schema() -> Value {
    de::without_fields(schema::<MemoryEntryMetadataResponse>(), ENTRY_HIDDEN)
}
fn entry_value(v: Value) -> Value {
    de::without_values(v, ENTRY_HIDDEN)
}
impl ApiEventInput for MemoryInspectionInput {
    fn document_input_schema(c: &ApiEventOperationContext) -> Option<Value> {
        if c.interface_id == "host_infrastructure.memory.entry.reveal" {
            Some(de::path_body_schema(
                json!({"type":"object","properties":{"contract_code":{"type":"string"}},"required":["contract_code"]}),
                de::without_fields(schema::<MemoryEntryRevealBody>(), &["entry_ref"]),
            ))
        } else {
            None
        }
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Overview | Self::StatsOverview => json!({}),
            Self::Entries {
                contract_code,
                query,
            }
            | Self::Tree {
                contract_code,
                query,
            } => {
                json!({"path":{"contract_code":contract_code},"query":q(json!({"path":query.path,"cursor":query.cursor,"limit":query.limit,"byte_limit":query.byte_limit}))})
            }
            Self::Stats {
                contract_code,
                query,
            } => {
                json!({"path":{"contract_code":contract_code},"query":q(json!({"path":query.path}))})
            }
            Self::Search {
                contract_code,
                query,
            } => {
                json!({"path":{"contract_code":contract_code},"query":q(json!({"q":query.q,"path":query.path,"cursor":query.cursor,"limit":query.limit,"byte_limit":query.byte_limit}))})
            }
            Self::Reveal {
                contract_code,
                body,
            } => {
                json!({"path":{"contract_code":contract_code},"body":{"reveal_mode":body.reveal_mode}})
            }
        })
    }
}
impl ApiEventOutput for MemoryInspectionOutput {
    fn document_output_schema(c: &ApiEventOperationContext) -> Option<Value> {
        match c.interface_id.as_str() {
            "host_infrastructure.memory.entries.list"
            | "host_infrastructure.memory.entries.search" => Some(de::replace_property(
                schema::<MemoryEntriesResponse>(),
                "entries",
                de::array_schema(entry_schema()),
            )),
            "host_infrastructure.memory.entry.reveal" => Some(de::replace_property(
                de::without_fields(schema::<MemoryEntryValueResponse>(), REVEAL_HIDDEN),
                "metadata",
                entry_schema(),
            )),
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Overview(v) => to_value(v),
            Self::StatsOverview(v) => to_value(v),
            Self::Stats(v) => to_value(v),
            Self::Tree(v) => to_value(v),
            Self::Entries(v) => {
                let mut out = to_value(v)?;
                out["entries"] = Value::Array(
                    v.entries
                        .iter()
                        .map(|m| to_value(m).map(entry_value))
                        .collect::<Result<_, _>>()?,
                );
                Ok(out)
            }
            Self::Revealed(v) => {
                let mut out = de::without_values(to_value(v)?, REVEAL_HIDDEN);
                out["metadata"] = entry_value(to_value(&v.metadata)?);
                Ok(out)
            }
        }
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
