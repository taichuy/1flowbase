use super::super::event_projection as ep;
use super::*;
use crate::extension_bus::managed_projection as mp;
use serde_json::{json, Value};

impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "application-agent-logs-otlp-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn event_projection_schema() -> Option<Value> {
        Some(ep::object_schema([
            (
                "format",
                json!({"type":"string","enum":["json","protobuf"]}),
            ),
            (
                "encoding",
                json!({"type":"string","enum":["identity","gzip"]}),
            ),
            ("batches", ep::array_schema(ep::batch_schema())),
        ]))
    }
    fn project_for_event(&self) -> Option<Value> {
        let prepared = self.prepared.as_ref().ok()?;
        Some(
            json!({"format":self.format.tag(),"encoding":self.encoding.tag(),"batches":prepared.batches.iter().map(ep::batch_value).collect::<Vec<_>>()}),
        )
    }
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([
            ("kind", mp::tag_schema(Self::CONTRACT_ID)),
            ("byte_count", mp::count_schema()),
            ("format", mp::text_schema()),
            ("encoding", mp::text_schema()),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            json!({"kind":Self::CONTRACT_ID,"byte_count":self.byte_count,"format":self.format.tag(),"encoding":self.encoding.tag()}),
        )
    }
}
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "application-agent-logs-otlp-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn event_projection_schema() -> Option<Value> {
        let mut schema = ep::receipt_schema();
        schema["properties"]["accepted_spans"] = json!({"type":"integer","minimum":0});
        schema["properties"]["rejected_spans"] = json!({"type":"integer","minimum":0});
        schema["required"]
            .as_array_mut()?
            .extend([json!("accepted_spans"), json!("rejected_spans")]);
        Some(schema)
    }
    fn project_for_event(&self) -> Option<Value> {
        let mut receipt = ep::receipt_value(&self.receipt);
        receipt["accepted_spans"] = json!(self.accepted_spans);
        receipt["rejected_spans"] = json!(self
            .response
            .partial_success
            .as_ref()
            .map_or(0, |partial| partial.rejected_spans));
        Some(receipt)
    }
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([
            ("kind", mp::tag_schema(Self::CONTRACT_ID)),
            ("accepted_spans", mp::count_schema()),
            ("rejected_spans", mp::count_schema()),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(
            json!({"kind":Self::CONTRACT_ID,"accepted_spans":self.accepted_spans,"rejected_spans":self.response.partial_success.as_ref().map_or(0,|partial| partial.rejected_spans)}),
        )
    }
}
impl InterfaceContract for TargetError {
    const CONTRACT_ID: &'static str = "application-agent-logs-otlp-error";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> {
        Some(mp::object_schema([(
            "kind",
            mp::tag_schema(Self::CONTRACT_ID),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(json!({"kind":Self::CONTRACT_ID}))
    }
}

impl crate::openapi_interface::contract::ApiInput for Input {
    fn document_input_schema(
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<Value> {
        Some(ep::object_schema([("body", crate::openapi_interface::contract::schema::<crate::routes::applications_group::application_runtime::log_exchange::event_projection::OtlpDocument>())]))
    }
    fn project_document_input(
        &self,
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<Value, Arc<str>> {
        let prepared = self
            .prepared
            .as_ref()
            .map_err(|code| Arc::<str>::from(*code))?;
        let document = prepared.document.as_ref().map_err(Arc::clone)?;
        Ok(json!({"body":crate::openapi_interface::contract::to_value(document)?}))
    }
}
impl crate::openapi_interface::contract::ApiOutput for Output {
    fn project_document_output(
        &self,
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<Value, Arc<str>> {
        // The standard OTLP acknowledgement is the actual HTTP result. The
        // semantic receipt remains separately owned by the existing producer.
        crate::openapi_interface::contract::to_value(&self.response)
    }
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/document_projection.rs"]
mod document_projection_tests;
