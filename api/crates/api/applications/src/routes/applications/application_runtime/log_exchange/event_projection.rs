//! Safe document-shaped OTLP views. Arbitrary OTLP attributes are opaque carriers
//! for headers, credentials and original source facts. Their contents are never
//! included by this API codec. The existing typed source-fact producer remains
//! the owner of semantic Agent Logs events.
use super::*;
use crate::openapi_interface::contract::{self as ep, ApiInput, ApiOperationContext, ApiOutput};
use crate::routes::applications_group::event_projection as application;

pub(crate) use api_host_kernel::openapi_interface::otlp_document::{document, OtlpDocument};
#[derive(Serialize, utoipa::ToSchema)]
struct ImportBody {
    otlp: OtlpDocument,
}

impl ApiInput for ExchangeInput {
    fn document_input_schema(operation: &ApiOperationContext) -> Option<Value> {
        (operation.interface_id == "applications.logs.otlp.import")
            .then(|| application::application_body_schema(operation, ep::schema::<ImportBody>()))
    }

    fn project_document_input(&self, operation: &ApiOperationContext) -> Result<Value, Arc<str>> {
        let (application_id, body) = match self {
            Self::OtlpExport {
                application_id,
                body,
            } => (application_id, ep::to_value(body)?),
            Self::OtlpImport {
                application_id,
                body,
            } => (
                application_id,
                ep::to_value(&ImportBody {
                    otlp: document(&body.otlp)?,
                })?,
            ),
        };
        Ok(application::request(
            operation,
            json!({"application_id":application_id}),
            json!({}),
            body,
        ))
    }
}

impl ApiOutput for ExchangeOutput {
    fn document_output_schema(operation: &ApiOperationContext) -> Option<Value> {
        (operation.interface_id == "applications.logs.otlp.export").then(ep::schema::<OtlpDocument>)
    }

    fn project_document_output(&self, _operation: &ApiOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Otlp(value) => ep::to_value(&document(value)?),
            Self::Imported(receipt) => ep::to_value(receipt),
        }
    }
}

#[cfg(test)]
#[path = "_tests/document_event_projection.rs"]
mod tests;
