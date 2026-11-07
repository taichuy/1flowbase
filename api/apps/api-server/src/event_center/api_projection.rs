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
    fn document_input_schema(_operation: &ApiEventOperationContext) -> Option<Value> {
        None
    }
    fn project_document_input(
        &self,
        _operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>> {
        Err(Arc::from("api-event-input-codec-missing"))
    }
}
pub(crate) trait ApiEventOutput: InterfaceContract {
    fn document_output_schema(_operation: &ApiEventOperationContext) -> Option<Value> {
        None
    }
    fn project_document_output(
        &self,
        _operation: &ApiEventOperationContext,
    ) -> Result<Value, Arc<str>> {
        Err(Arc::from("api-event-output-codec-missing"))
    }
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
    let request = I::document_input_schema(&operation);
    let result = O::document_output_schema(&operation);
    if request.is_none() && result.is_none() {
        return Ok(());
    }
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
