//! Explicit host-owned safe views over the actual typed API contracts.
use interface_runtime::{
    DocumentOperationCodec, DocumentOperationDescriptor, DocumentOperationProjection,
    DocumentOperationRegistration, DocumentOperationSchemas, InterfaceContract, ProtocolBinding,
    RegistryCompilationError, RegistryCompiler,
};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct ApiEventOperationContext {
    pub interface_id: String,
    pub method: String,
    pub path: String,
}

pub(crate) trait ApiEventInput: InterfaceContract {
    /// None inherits the actual shared API document schema at serving composition.
    fn document_input_schema(_operation: &ApiEventOperationContext) -> Option<Value> {
        None
    }
    fn document_input_unavailable(
        _operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        None
    }
    fn project_document_input(
        &self,
        operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>>;
}
pub(crate) trait ApiEventOutput: InterfaceContract {
    fn document_output_schema(_operation: &ApiEventOperationContext) -> Option<Value> {
        None
    }
    fn document_output_unavailable(
        _operation: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        None
    }
    fn project_document_output(
        &self,
        operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>>;
}
// Ordinary JSON is an interface contract; observation only invokes its shared conversion.
impl<T: crate::openapi_interface::json_contract::JsonApiInput> ApiEventInput for T {
    fn project_document_input(
        &self,
        operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>> {
        crate::openapi_interface::json_contract::request(self, &operation.path)
    }
}
impl<T: crate::openapi_interface::json_contract::JsonApiOutput> ApiEventOutput for T {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        crate::openapi_interface::json_contract::result(self)
            .map_err(|_| Arc::from("api-json-output-serialization"))
    }
}

fn reason_code(reason: domain::event_center::ApiEventUnavailableReason) -> Arc<str> {
    let encoded = serde_json::to_value(reason).expect("owner reason serializes");
    Arc::from(encoded.as_str().expect("owner reason is a string"))
}
/// Explicit owner opt-in for an already safe DTO. Does not implement either codec trait.
pub(crate) fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, Arc<str>> {
    serde_json::to_value(value).map_err(|_| Arc::from("api-event-safe-dto-serialization-failed"))
}
pub(crate) fn schema<T: utoipa::ToSchema>() -> Value {
    let mut schemas = Vec::new();
    T::schemas(&mut schemas);
    schemas.push((T::name().into_owned(), T::schema()));
    let definitions = schemas
        .into_iter()
        .map(|(name, schema)| {
            (
                name,
                serde_json::to_value(schema).expect("typed schema serializes"),
            )
        })
        .collect::<serde_json::Map<String, Value>>();
    let source = serde_json::json!({"components":{"schemas":definitions}});
    let root = serde_json::to_value(T::schema()).expect("typed schema serializes");
    crate::openapi_interface::close_api_event_schema(&source, &root)
}

struct Codec<I, O> {
    operation: ApiEventOperationContext,
    marker: std::marker::PhantomData<fn(I) -> O>,
}
impl<I: ApiEventInput, O: ApiEventOutput> DocumentOperationCodec<I, O> for Codec<I, O> {
    fn project_input(&self, input: &I) -> Result<Value, Arc<str>> {
        input.project_document_input(&self.operation)
    }
    fn project_output(&self, output: &O) -> Result<Value, Arc<str>> {
        output.project_document_output(&self.operation)
    }
}
/// Called by typed family registration. Final composition replaces only document metadata
/// from the unique shared API-doc method/route correspondence before publishing the registry.
pub(crate) fn register_static_api_event<I: ApiEventInput, O: ApiEventOutput>(
    compiler: &mut RegistryCompiler,
    binding: &ProtocolBinding,
    generation: &str,
) -> Result<(), RegistryCompilationError> {
    let Some(route) = binding.projection().http_route() else {
        return Ok(());
    };
    let operation = ApiEventOperationContext {
        interface_id: binding
            .interface_identity()
            .interface_id()
            .as_str()
            .to_string(),
        method: route.method().to_string(),
        path: route.path().to_string(),
    };
    let request_unavailable = I::document_input_unavailable(&operation).map(reason_code);
    let result_unavailable = O::document_output_unavailable(&operation).map(reason_code);
    // Null is an unpublished inheritance marker, resolved from the unique real document in boot.
    let request = request_unavailable
        .is_none()
        .then(|| I::document_input_schema(&operation).unwrap_or(Value::Null));
    let result = result_unavailable
        .is_none()
        .then(|| O::document_output_schema(&operation).unwrap_or(Value::Null));
    let descriptor = DocumentOperationDescriptor {
        // Unpublished family snapshots use their canonical identity; the serving root replaces
        // this metadata from the sole API-doc catalog before execution admission opens.
        operation_id: Arc::from(operation.interface_id.as_str()),
        binding_id: binding.binding_id().clone(),
        interface_identity: binding.interface_identity().clone(),
        registration_generation: Arc::from(generation),
        target_identity: Arc::from(format!(
            "{}:{}:{}",
            operation.interface_id, operation.method, operation.path
        )),
        target_generation: Arc::from(generation),
        schemas: DocumentOperationSchemas {
            request,
            result,
            event: None,
            request_unavailable,
            result_unavailable,
        },
    };
    compiler.bind_document_operation(
        binding.binding_id(),
        DocumentOperationRegistration::static_operation(
            generation,
            DocumentOperationProjection {
                descriptor,
                codec: Arc::new(Codec::<I, O> {
                    operation,
                    marker: std::marker::PhantomData,
                }),
            },
        ),
    )
}

/// Refine only unpublished observation metadata using the sole static API document source.
pub(crate) fn refine_static_api_events(
    compiler: &mut RegistryCompiler,
    docs: &crate::openapi_docs::ApiDocsRegistry,
    snapshots: &[Arc<interface_runtime::CompiledInterfaceRegistry>],
) -> anyhow::Result<()> {
    let mut by_route = std::collections::BTreeMap::<(String, String), Vec<String>>::new();
    for category in &docs.catalog().categories {
        if let Some(category) = docs.category_operations(&category.id) {
            for operation in &category.operations {
                by_route
                    .entry(route_identity(&operation.method, &operation.path))
                    .or_default()
                    .push(operation.id.clone());
            }
        }
    }
    for snapshot in snapshots {
        for capability in snapshot
            .document_operations()
            .filter(|capability| !capability.dynamic)
        {
            let Some(route) = snapshot
                .binding(capability.binding_id)
                .and_then(|binding| binding.projection().http_route())
            else {
                continue;
            };
            let Some(operations) = by_route.get(&route_identity(route.method(), route.path()))
            else {
                let mut descriptor = capability
                    .static_descriptor
                    .expect("static capability owns descriptor")
                    .clone();
                descriptor.schemas.request = None;
                descriptor.schemas.result = None;
                descriptor.schemas.request_unavailable =
                    Some(Arc::from("missing_openapi_contract"));
                descriptor.schemas.result_unavailable = Some(Arc::from("missing_openapi_contract"));
                compiler.rebind_document_operation_descriptor(capability.binding_id, descriptor)?;
                continue;
            };
            if operations.len() != 1 {
                anyhow::bail!(
                    "ambiguous API document operation for {} {}",
                    route.method(),
                    route.path()
                );
            }
            let mut descriptor = capability
                .static_descriptor
                .expect("static capability owns descriptor")
                .clone();
            descriptor.operation_id = Arc::from(operations[0].as_str());
            let operation = docs
                .operation(&operations[0])
                .expect("indexed API operation exists");
            let view = docs
                .operation_spec_view(&operations[0])
                .ok_or_else(|| anyhow::anyhow!("API document contract is missing"))?;
            let authentic = crate::openapi_interface::catalog_entry_from_view(&operation, &view)
                .ok_or_else(|| anyhow::anyhow!("API document contract is missing"))?;
            if descriptor.schemas.request.as_ref() == Some(&Value::Null) {
                if authentic
                    .request_media_type
                    .as_deref()
                    .is_some_and(|media| !json_media(media))
                {
                    descriptor.schemas.request = None;
                    descriptor.schemas.request_unavailable = Some(Arc::from("binary_request"));
                } else {
                    descriptor.schemas.request =
                        Some(safe_document_request_schema(authentic.request_schema));
                }
            }
            if descriptor.schemas.result.as_ref() == Some(&Value::Null) {
                match authentic.response_media_type.as_deref() {
                    Some("text/event-stream") => {
                        descriptor.schemas.result = None;
                        descriptor.schemas.result_unavailable =
                            Some(Arc::from("missing_streaming_terminal"));
                    }
                    Some(media) if !json_media(media) => {
                        descriptor.schemas.result = None;
                        descriptor.schemas.result_unavailable = Some(Arc::from("binary_response"));
                    }
                    _ => descriptor.schemas.result = Some(authentic.response_schema),
                }
            }

            compiler.rebind_document_operation_descriptor(capability.binding_id, descriptor)?;
        }
    }
    Ok(())
}
pub(crate) fn route_identity(method: &str, path: &str) -> (String, String) {
    (
        method.to_ascii_uppercase(),
        path.split('/')
            .map(|part| {
                if part.starts_with(':') || (part.starts_with('{') && part.ends_with('}')) {
                    "{}"
                } else {
                    part
                }
            })
            .collect::<Vec<_>>()
            .join("/"),
    )
}

pub(crate) fn safe_document_request_schema(mut schema: Value) -> Value {
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        properties.remove("headers");
    }
    if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|value| value.as_str() != Some("headers"));
    }
    schema
}

fn json_media(media: &str) -> bool {
    media == "application/json" || media.ends_with("+json")
}
