use super::*;
use crate::event_center::api_projection::{
    schema, to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::settings::document_event as de;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::routes::settings::host_infrastructure::CacheEntryMetadataResponse;
fn metadata_schema() -> Value {
    de::without_fields(schema::<CacheEntryMetadataResponse>(), &["key"])
}
fn metadata_value(v: Value) -> Value {
    de::without_values(v, &["key"])
}
impl ApiEventInput for CacheInspectionInput {
    fn document_input_schema(c: &ApiEventOperationContext) -> Option<Value> {
        if matches!(
            c.interface_id.as_str(),
            "host_infrastructure.cache.entry.reveal" | "host_infrastructure.cache.entry.clear"
        ) {
            Some(de::path_body_schema(
                json!({"type":"object","properties":{"domain_code":{"type":"string"}},"required":["domain_code"]}),
                de::without_fields(schema::<CacheEntryKeyBody>(), &["key"]),
            ))
        } else {
            None
        }
    }
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Overview => json!({}),
            Self::Entries { domain_code } | Self::ClearDomain { domain_code } => {
                json!({"path":{"domain_code":domain_code}})
            }
            Self::Reveal { domain_code, .. } | Self::ClearEntry { domain_code, .. } => {
                json!({"path":{"domain_code":domain_code},"body":{}})
            }
        })
    }
}
impl ApiEventOutput for CacheInspectionOutput {
    fn document_output_schema(c: &ApiEventOperationContext) -> Option<Value> {
        match c.interface_id.as_str() {
            "host_infrastructure.cache.entries.list" => Some(de::replace_property(
                schema::<CacheEntriesResponse>(),
                "entries",
                de::array_schema(metadata_schema()),
            )),
            "host_infrastructure.cache.entry.reveal" => Some(de::replace_property(
                de::without_fields(schema::<CacheEntryValueResponse>(), &["value"]),
                "metadata",
                metadata_schema(),
            )),
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Overview(v) => to_value(v),
            Self::Entries(v) => {
                let mut out = to_value(v)?;
                out["entries"] = Value::Array(
                    v.entries
                        .iter()
                        .map(|m| to_value(m).map(metadata_value))
                        .collect::<Result<_, _>>()?,
                );
                Ok(out)
            }
            Self::Revealed(v) => {
                let mut out = de::without_values(to_value(v)?, &["value"]);
                out["metadata"] = metadata_value(to_value(&v.metadata)?);
                Ok(out)
            }
            Self::EntryCleared(v) => to_value(v),
            Self::DomainCleared(v) => to_value(v),
        }
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
