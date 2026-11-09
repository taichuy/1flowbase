//! The runtime-model owner freezes the model/template used by both execution and observation.
use super::*;
use control_plane::ports::ModelDefinitionRepository;
use interface_runtime::{
    BindingId, DocumentOperationCodec, DocumentOperationDescriptor, DocumentOperationProjection,
    DocumentOperationRegistration, DocumentOperationSchemas, DocumentOperationSelector,
    InterfaceId, InterfaceIdentity, InterfaceVersion, RegistryCompiler,
};
use sha2::{Digest, Sha256};
use std::any::Any;

const GENERATION: &str = "runtime-model-document-codec-v1";
struct AuthorizedOperation {
    descriptor: DocumentOperationDescriptor,
    model_id: Uuid,
    model_code: String,
    workspace_id: Uuid,
    path_parameters: std::collections::BTreeMap<String, String>,
}
struct Selector {
    interface_id: InterfaceId,
    binding_id: BindingId,
}
struct Codec {
    snapshot: Arc<AuthorizedOperation>,
}
impl
    DocumentOperationSelector<
        interface::RuntimeModelOperationInput,
        interface::RuntimeModelOperationOutput,
    > for Selector
{
    fn select(
        &self,
        snapshot: &(dyn Any + Send + Sync),
    ) -> Result<
        DocumentOperationProjection<
            interface::RuntimeModelOperationInput,
            interface::RuntimeModelOperationOutput,
        >,
        Arc<str>,
    > {
        let snapshot = snapshot
            .downcast_ref::<Arc<AuthorizedOperation>>()
            .ok_or("runtime-model-snapshot-type")?;
        let identity = &snapshot.descriptor.interface_identity;
        let parsed =
            crate::runtime_data_model_docs::parse_operation_id(&snapshot.descriptor.operation_id)
                .map_err(Arc::from)?;
        if identity.interface_id() != &self.interface_id
            || snapshot.descriptor.binding_id != self.binding_id
            || snapshot.descriptor.registration_generation.as_ref() != GENERATION
            || parsed
                .as_ref()
                .is_none_or(|(id, _)| *id != snapshot.model_id)
            || snapshot.model_code.is_empty()
        {
            return Err("runtime-model-snapshot-identity".into());
        }
        Ok(DocumentOperationProjection {
            descriptor: snapshot.descriptor.clone(),
            codec: Arc::new(Codec {
                snapshot: Arc::clone(snapshot),
            }),
        })
    }
}
impl
    DocumentOperationCodec<
        interface::RuntimeModelOperationInput,
        interface::RuntimeModelOperationOutput,
    > for Codec
{
    fn project_input(
        &self,
        input: &interface::RuntimeModelOperationInput,
    ) -> Result<Value, Arc<str>> {
        if input.model_code != self.snapshot.model_code {
            return Err("runtime-model-input-target".into());
        }
        let schema = self
            .snapshot
            .descriptor
            .schemas
            .request
            .as_ref()
            .ok_or("runtime-model-request-contract")?;
        let mut locations = serde_json::Map::new();
        if let Some(path) = schema
            .pointer("/properties/path/properties")
            .and_then(Value::as_object)
        {
            let values = path
                .keys()
                .map(|key| {
                    self.snapshot
                        .path_parameters
                        .get(key)
                        .map(|value| (key.clone(), Value::String(value.clone())))
                        .ok_or_else(|| Arc::<str>::from("runtime-model-path-value"))
                })
                .collect::<Result<serde_json::Map<String, Value>, Arc<str>>>()?;
            locations.insert("path".into(), Value::Object(values));
        }
        if let Some(query_schema) = schema
            .pointer("/properties/query/properties")
            .and_then(Value::as_object)
        {
            let raw = parse_runtime_query(input.query.as_deref());
            let mut values = serde_json::Map::new();
            for (key, field) in query_schema {
                if let Some(raw) = raw.get(key) {
                    values.insert(key.clone(), query_value(raw, field)?);
                }
            }
            locations.insert("query".into(), Value::Object(values));
        }
        if schema.pointer("/properties/body").is_some() && !input.body.is_empty() {
            locations.insert(
                "body".into(),
                serde_json::from_slice(&input.body)
                    .map_err(|_| Arc::<str>::from("runtime-model-json-input"))?,
            );
        }
        Ok(Value::Object(locations))
    }
    fn project_output(
        &self,
        output: &interface::RuntimeModelOperationOutput,
    ) -> Result<Value, Arc<str>> {
        Ok(output.data.clone())
    }
}
fn query_value(raw: &str, schema: &Value) -> Result<Value, Arc<str>> {
    match schema.get("type").and_then(Value::as_str) {
        Some("integer") => raw
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| "runtime-model-integer-query".into()),
        Some("number") => serde_json::from_str::<Value>(raw)
            .ok()
            .filter(Value::is_number)
            .ok_or_else(|| "runtime-model-number-query".into()),
        Some("boolean") => raw
            .parse::<bool>()
            .map(Value::from)
            .map_err(|_| "runtime-model-boolean-query".into()),
        Some("object" | "array") => {
            serde_json::from_str(raw).map_err(|_| "runtime-model-json-query".into())
        }
        _ => Ok(Value::String(raw.to_owned())),
    }
}

pub(super) fn register(
    compiler: &mut RegistryCompiler,
    binding_id: &str,
    interface_id: &InterfaceId,
) -> Result<(), interface_runtime::RegistryCompilationError> {
    let binding_id = BindingId::new(binding_id).expect("declared model binding");
    compiler.bind_document_operation::<interface::RuntimeModelOperationInput, interface::RuntimeModelOperationOutput>(
        &binding_id, DocumentOperationRegistration::dynamic(GENERATION, Arc::new(Selector { interface_id: interface_id.clone(), binding_id: binding_id.clone() })),
    )
}
fn binding_for_method(method: &str) -> Option<(&'static str, String)> {
    interface::binding_declarations()
        .into_iter()
        .find(|(candidate, _, _)| candidate.as_str() == method)
        .map(|(_, name, binding)| (binding, format!("runtime.models.invoke.{name}")))
}
fn descriptor(
    model: &domain::ModelDefinitionRecord,
    catalog: &runtime_core::data_model_template_registry::DataModelTemplateCatalog,
    operation_code: &str,
) -> Result<DocumentOperationDescriptor, ApiError> {
    let operation = crate::runtime_data_model_docs::build_category_operations(
        std::slice::from_ref(model),
        catalog,
    )
    .operations
    .into_iter()
    .find(|operation| {
        operation.id == crate::runtime_data_model_docs::operation_id(model.id, operation_code)
    })
    .ok_or(control_plane::errors::ControlPlaneError::NotFound(
        "runtime_operation",
    ))?;
    let spec =
        crate::runtime_data_model_docs::build_operation_openapi(model, operation_code, catalog)
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "runtime_operation_contract",
            ))?;
    let api = crate::openapi_interface::catalog_entry_from_operation(&operation, &spec).ok_or(
        control_plane::errors::ControlPlaneError::NotFound("runtime_operation_contract"),
    )?;
    let (binding, interface_id) = binding_for_method(&api.method).ok_or(
        control_plane::errors::ControlPlaneError::NotFound("runtime_method"),
    )?;
    // Hash only this immutable owner snapshot, never a second registry/directory lookup.
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(model)?);
    hash.update(serde_json::to_vec(&spec)?);
    let generation = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let scope_kind = match model.scope_kind {
        domain::DataModelScopeKind::Workspace => "workspace",
        domain::DataModelScopeKind::System => "system",
    };
    Ok(DocumentOperationDescriptor {
        operation_id: api.operation_id.into(),
        binding_id: BindingId::new(binding).expect("model binding"),
        interface_identity: InterfaceIdentity::new(
            InterfaceId::new(interface_id).expect("model interface"),
            InterfaceVersion::new("1").expect("model version"),
        ),
        registration_generation: GENERATION.into(),
        target_identity: format!(
            "model:{}:{}:{}:{}",
            model.id, scope_kind, model.scope_id, operation_code
        )
        .into(),
        target_generation: generation.into(),
        schemas: DocumentOperationSchemas {
            request: Some(api.request_schema),
            result: Some(api.response_schema),
            event: None,
            request_unavailable: None,
            result_unavailable: None,
        },
    })
}
async fn model_for_id(
    store: &MainDurableStore,
    actor: &domain::ActorContext,
    id: Uuid,
) -> Result<Option<domain::ModelDefinitionRecord>, ApiError> {
    let mut model = store
        .get_model_definition(actor.current_workspace_id, id)
        .await?;
    if model.is_none() && actor.current_workspace_id != domain::SYSTEM_SCOPE_ID {
        model = store
            .get_model_definition(domain::SYSTEM_SCOPE_ID, id)
            .await?;
    }
    Ok(model.filter(|model| {
        model.status == domain::DataModelStatus::Published
            && (model.scope_kind == domain::DataModelScopeKind::System
                || model.scope_id == actor.current_workspace_id)
    }))
}
async fn authorize(
    store: &MainDurableStore,
    actor: &domain::ActorContext,
    model: &domain::ModelDefinitionRecord,
    operation_code: &str,
    catalog: &runtime_core::data_model_template_registry::DataModelTemplateCatalog,
) -> Result<(), ApiError> {
    let identity = extension_contracts::DataModelTemplateIdentity {
        provider: model.template_provider.clone(),
        code: model.template_code.clone(),
        version: model.template_version.clone(),
    };
    let template = catalog
        .resolve(&identity)
        .map_err(|_| control_plane::errors::ControlPlaneError::NotFound("runtime_template"))?;
    let operation = template.operation(operation_code).ok_or(
        control_plane::errors::ControlPlaneError::NotFound("runtime_operation"),
    )?;
    let action = runtime_core::general_data_model_template::runtime_data_action(
        &operation.permission_action,
    )
    .ok_or(control_plane::errors::ControlPlaneError::PermissionDenied(
        "runtime_action",
    ))?;
    let grant = control_plane::model_definition::ModelDefinitionService::new(store.clone())
        .load_runtime_scope_grant(actor, model.id, action)
        .await?;
    runtime_core::runtime_acl::resolve_access_scope(actor, action, model.id, grant.as_ref())
        .map_err(|error| map_runtime_error(error.into()))?;
    Ok(())
}
/// Discovery uses the same published model/template owner and action grants as execution.
pub(crate) async fn document_descriptor_for_entry(
    dependencies: &crate::openapi_interface::OpenApiCapabilityCatalogDependencies,
    actor: &domain::ActorContext,
    entry: &crate::openapi_interface::OpenApiCapabilityCatalogEntry,
) -> Result<Option<DocumentOperationDescriptor>, ApiError> {
    let Some((id, operation)) = crate::runtime_data_model_docs::parse_operation_id(
        &entry.interface.operation_id,
    )
    .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("runtime_operation_id"))?
    else {
        return Ok(None);
    };
    let Some(model) = model_for_id(&dependencies.store, actor, id).await? else {
        return Ok(None);
    };
    authorize(
        &dependencies.store,
        actor,
        &model,
        &operation,
        &dependencies.template_catalog,
    )
    .await?;
    let descriptor = descriptor(&model, &dependencies.template_catalog, &operation)?;
    let (Some((binding, _)), Some(registry)) = (
        binding_for_method(&entry.interface.method),
        dependencies.interface_registry.as_deref(),
    ) else {
        return Ok(None);
    };
    let Some(capability) =
        registry.document_operation(&BindingId::new(binding).expect("model binding"))
    else {
        return Ok(None);
    };
    if !capability.dynamic
        || capability.registration_generation != GENERATION
        || descriptor.operation_id.as_ref() != entry.interface.operation_id
    {
        return Ok(None);
    }
    Ok(Some(descriptor))
}

pub(super) async fn prepare(
    adapter: &RuntimeModelOperationAdapter,
    actor: &domain::ActorContext,
    input: &interface::RuntimeModelOperationInput,
    resolved: &ResolvedRuntimeOperation,
) -> Result<Arc<dyn Any + Send + Sync>, ApiError> {
    let metadata = adapter
        .runtime_engine
        .load_available_metadata(&input.model_code, actor.current_workspace_id)
        .map_err(map_runtime_error)?;
    let model = model_for_id(&adapter.store, actor, metadata.model_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "runtime_model",
        ))?;
    // A DB publication refresh must not attach new docs to an older execution snapshot.
    if model.code != metadata.model_code
        || model
            .fields
            .iter()
            .filter(|field| field.availability_status.is_healthy())
            .ne(metadata.fields.iter())
        || model.scope_kind != metadata.scope_kind
        || model.physical_table_name != metadata.physical_table_name
        || model.source_kind != metadata.source_kind
        || model.data_source_instance_id != metadata.data_source_instance_id
        || model.external_resource_key != metadata.external_resource_key
        || model.scope_id != metadata.scope_id
        || model.template_provider != metadata.template_provider
        || model.template_code != metadata.template_code
        || model.template_version != metadata.template_version
    {
        return Err(control_plane::errors::ControlPlaneError::Conflict(
            "runtime_model_snapshot_changed",
        )
        .into());
    }
    let descriptor = descriptor(
        &model,
        adapter.runtime_engine.template_catalog(),
        &resolved.operation_code,
    )?;
    let snapshot = Arc::new(AuthorizedOperation {
        descriptor,
        model_id: model.id,
        model_code: model.code,
        workspace_id: actor.current_workspace_id,
        path_parameters: resolved.path_parameters.clone(),
    });
    // The selector accepts only the sealed Arc created by this authorized execution owner.
    Ok(Arc::new(snapshot))
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/document_event_projection.rs"]
mod tests;
