//! Explicit codecs over one authorized immutable publication snapshot.
use super::*;
use control_plane::application_public_api::{
    mapping::WorkflowExtensionResponseMode,
    workflow_extension::{
        AuthorizedPublishedWorkflowOperation, WorkflowExtensionRunError,
        WorkflowExtensionRunService,
    },
    workflow_start_http_inputs::{parse_workflow_start_http_inputs, WorkflowStartHttpInputSource},
};
use interface_runtime::{
    DocumentOperationCodec, DocumentOperationDescriptor, DocumentOperationProjection,
    DocumentOperationSchemas, DocumentOperationSelector,
};
use serde_json::{json, Map, Value};
use std::{any::Any, sync::Arc};

pub(super) const REGISTRATION_GENERATION: &str = "workflow-extension.document-v1";
pub(super) struct Selector;
impl DocumentOperationSelector<WorkflowExtensionInput, WorkflowExtensionOutput> for Selector {
    fn select(
        &self,
        snapshot: &(dyn Any + Send + Sync),
    ) -> Result<
        DocumentOperationProjection<WorkflowExtensionInput, WorkflowExtensionOutput>,
        Arc<str>,
    > {
        let authorized = snapshot
            .downcast_ref::<AuthorizedPublishedWorkflowOperation>()
            .ok_or("workflow-operation-proof-missing")?;
        Ok(DocumentOperationProjection {
            descriptor: descriptor(authorized)?,
            codec: Arc::new(Codec(authorized.operation_snapshot())),
        })
    }
}
fn descriptor(
    authorized: &AuthorizedPublishedWorkflowOperation,
) -> Result<DocumentOperationDescriptor, Arc<str>> {
    let operation = authorized.operation();
    if operation.workspace_id != authorized.workspace_id() {
        return Err(Arc::from("workflow-operation-proof-mismatch"));
    }
    descriptor_for_operation(operation, authorized.generation())
}
fn descriptor_for_operation(
    operation:&control_plane::application_public_api::published_workflow_operation::PublishedWorkflowOperation,
    generation: &str,
) -> Result<DocumentOperationDescriptor, Arc<str>> {
    let publication = &operation.publication;
    if operation.publication_version_id != publication.id
        || operation.application_id != publication.application_id
        || operation.workspace_id != publication.workspace_id
        || operation.interface_id
            != format!("published_workflow_operation:{}", operation.application_id)
    {
        return Err(Arc::from("workflow-operation-proof-mismatch"));
    }
    let accepted = json!({"type":"object","required":["run_id","status"],"properties":{"run_id":{"type":"string","format":"uuid"},"status":{"type":"string"}}});
    let result = if operation.response_mode == WorkflowExtensionResponseMode::Async {
        accepted
    } else {
        json!({"anyOf":[operation.result_schema,accepted]})
    };
    let mut request = operation.parameter_schema.clone();
    if let Some(properties) = request.get_mut("properties").and_then(Value::as_object_mut) {
        if let Some(form) = properties.remove("form") {
            let body = properties
                .entry("body")
                .or_insert_with(|| json!({"type":"object","properties":{},"required":[]}));
            if let Some(fields) = form.get("properties").and_then(Value::as_object) {
                body["properties"]
                    .as_object_mut()
                    .ok_or("workflow-body-schema-invalid")?
                    .extend(fields.clone());
            }
            if let Some(required) = form.get("required").and_then(Value::as_array) {
                body["required"]
                    .as_array_mut()
                    .ok_or("workflow-body-schema-invalid")?
                    .extend(required.clone());
            }
        }
    }
    Ok(DocumentOperationDescriptor {
        operation_id: Arc::from(operation.interface_id.as_str()),
        binding_id: BindingId::new(BINDING_ID).expect("static binding"),
        interface_identity: InterfaceIdentity::new(
            InterfaceId::new(INTERFACE_ID).expect("static interface"),
            InterfaceVersion::new("1").expect("static version"),
        ),
        registration_generation: Arc::from(REGISTRATION_GENERATION),
        target_identity: Arc::from(format!(
            "workspace:{}/application:{}",
            operation.workspace_id, operation.application_id
        )),
        target_generation: Arc::from(generation),
        schemas: DocumentOperationSchemas {
            request: Some(request),
            result: Some(result),
            event: None,
            request_unavailable: None,
            result_unavailable: None,
        },
    })
}
struct Codec(Arc<control_plane::application_public_api::published_workflow_operation::PublishedWorkflowOperation>);
impl DocumentOperationCodec<WorkflowExtensionInput, WorkflowExtensionOutput> for Codec {
    fn project_input(&self, input: &WorkflowExtensionInput) -> Result<Value, Arc<str>> {
        let operation = self.0.as_ref();
        if operation.method != input.method {
            return Err(Arc::from("workflow-method-mismatch"));
        }
        let contract = parse_workflow_start_http_inputs(&operation.publication.document_snapshot)
            .map_err(|_| Arc::<str>::from("workflow-input-contract-invalid"))?;
        let prepared = self
            .0
            .prepare_input(&input.parameters, &input.request_path)
            .map_err(|_| Arc::<str>::from("workflow-input-invalid"))?;
        let values = prepared
            .get(contract.start_node_id())
            .and_then(Value::as_object)
            .ok_or("workflow-input-values-invalid")?;
        let mut projected = Map::new();
        for field in contract.fields() {
            let Some(value) = values.get(field.key()) else {
                continue;
            };
            let location = match field.source() {
                WorkflowStartHttpInputSource::Path => "path",
                WorkflowStartHttpInputSource::Query => "query",
                WorkflowStartHttpInputSource::Body | WorkflowStartHttpInputSource::Form => "body",
            };
            projected
                .entry(location.to_string())
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .expect("object literal")
                .insert(field.key().to_string(), value.clone());
        }
        Ok(Value::Object(projected))
    }
    fn project_output(&self, output: &WorkflowExtensionOutput) -> Result<Value, Arc<str>> {
        match output {
            WorkflowExtensionOutput::Accepted { run_id, status } => {
                Ok(json!({"run_id":run_id,"status":status.as_str()}))
            }
            WorkflowExtensionOutput::Completed(detail)
                if self.0.response_mode == WorkflowExtensionResponseMode::Sync
                    && detail.flow_run.status == domain::FlowRunStatus::Succeeded =>
            {
                Ok(detail.flow_run.output_payload.clone())
            }
            WorkflowExtensionOutput::Completed(_) => {
                Err(Arc::from("workflow-http-result-not-successful"))
            }
        }
    }
}

pub(crate) async fn document_descriptor_for_entry(
    dependencies: &crate::openapi_interface::OpenApiCapabilityCatalogDependencies,
    actor: &domain::ActorContext,
    entry: &crate::openapi_interface::OpenApiCapabilityCatalogEntry,
) -> Result<Option<DocumentOperationDescriptor>, crate::error_response::ApiError> {
    if entry.source != crate::openapi_interface::OpenApiCapabilitySource::PublishedWorkflow {
        return Ok(None);
    }
    let authorized = match WorkflowExtensionRunService::new(dependencies.store.clone())
        .authorize_operation_id(actor, &entry.interface.operation_id)
        .await
    {
        Ok(operation) => operation,
        Err(WorkflowExtensionRunError::Forbidden) => {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "workflow_extension_forbidden",
            )
            .into())
        }
        Err(WorkflowExtensionRunError::Internal(error)) => return Err(error.into()),
        Err(_) => return Ok(None),
    };
    if authorized.operation().method.as_str() != entry.interface.method
        || authorized.operation().public_path() != entry.interface.path
    {
        return Ok(None);
    }
    descriptor(&authorized).map(Some).map_err(|_| {
        control_plane::errors::ControlPlaneError::Conflict("workflow_event_descriptor").into()
    })
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/document_observation.rs"]
mod tests;
