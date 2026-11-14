use super::*;
use crate::openapi_interface::contract::{
    schema, to_value, ApiInput, ApiOperationContext, ApiOutput,
};
use crate::openapi_interface::schema_projection::{self as de, query_fields as q};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiOutput for SystemInterfaceOutput {
    fn document_output_schema(c: &ApiOperationContext) -> Option<Value> {
        match c.interface_id.as_str() {
            "system.runtime_profile.view" => {
                let mut s = schema::<SystemRuntimeProfileResponse>();
                let locale = s["properties"]["locale_meta"].take();
                s["properties"]["locale_meta"] = de::without_fields(locale, &["requested_locale"]);
                Some(s)
            }
            "system.runtime_processes.view" => Some(de::replace_property(
                schema::<SystemRuntimeProcessListResponse>(),
                "processes",
                de::array_schema(de::without_fields(
                    schema::<SystemProcessResponse>(),
                    &["command"],
                )),
            )),
            _ => None,
        }
    }
    fn project_document_output(&self, _: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::ReleaseStatus(v) => to_value(v),
            Self::ProcessTermination(v) => to_value(v),
            Self::RuntimeProfile(v) => {
                let mut out = to_value(v)?;
                out["locale_meta"] =
                    de::without_values(out["locale_meta"].take(), &["requested_locale"]);
                Ok(out)
            }
            Self::ProcessList(v) => {
                let mut out = to_value(v)?;
                out["processes"] = Value::Array(
                    v.processes
                        .iter()
                        .map(|p| to_value(p).map(|v| de::without_values(v, &["command"])))
                        .collect::<Result<_, _>>()?,
                );
                Ok(out)
            }
        }
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/document_event.rs"]
mod tests;
