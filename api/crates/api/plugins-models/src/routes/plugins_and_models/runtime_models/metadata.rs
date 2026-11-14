//! Read projection for runtime consumers. Never serialize storage/provider metadata here.
use super::*;
use runtime_core::runtime_acl::{resolve_access_scope, RuntimeDataAction};

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeDataModelField {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    #[schema(value_type = String)]
    pub field_kind: domain::ModelFieldKind,
    pub is_required: bool,
    pub api_required: bool,
    pub is_writable: bool,
    pub default_value: Option<Value>,
    pub display_interface: Option<String>,
    pub display_options: Value,
    pub relation_target_model_id: Option<Uuid>,
    pub relation_target_model_code: Option<String>,
    pub relation_options: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeDataModelRecordCapabilities {
    pub can_list: bool,
    pub can_get: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeDataModelMetadata {
    pub model_id: Uuid,
    pub model_code: String,
    #[schema(value_type = String)]
    pub scope_kind: domain::DataModelScopeKind,
    pub scope_id: Uuid,
    pub record_id_field: Option<String>,
    pub fields: Vec<RuntimeDataModelField>,
    pub record_capabilities: RuntimeDataModelRecordCapabilities,
}

impl interface_runtime::InterfaceContract for RuntimeDataModelMetadata {
    const CONTRACT_ID: &'static str = "runtime-model-metadata-output";
    const CONTRACT_VERSION: &'static str = "1";

    fn managed_projection_schema() -> Option<Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([
            ("model_id", mp::text_schema()),
            ("model_code", mp::text_schema()),
            ("field_count", mp::count_schema()),
        ]))
    }

    fn project_for_managed_hook(&self) -> Option<Value> {
        Some(serde_json::json!({
            "model_id": self.model_id,
            "model_code": self.model_code,
            "field_count": self.fields.len(),
        }))
    }
}

impl crate::openapi_interface::contract::ApiOutput for RuntimeDataModelMetadata {
    fn project_document_output(
        &self,
        _: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<Value, Arc<str>> {
        crate::openapi_interface::contract::to_value(self)
    }
}

pub(super) async fn load(
    adapter: &RuntimeModelOperationAdapter,
    actor: &domain::ActorContext,
    model_code: &str,
) -> Result<RuntimeDataModelMetadata, ApiError> {
    let metadata = adapter
        .runtime_engine
        .load_available_metadata(model_code, actor.current_workspace_id)
        .map_err(map_runtime_error)?;
    let read_grant =
        load_runtime_scope_grant(adapter, actor, metadata.model_id, RuntimeDataAction::View)
            .await?;
    resolve_access_scope(
        actor,
        RuntimeDataAction::View,
        metadata.model_id,
        read_grant.as_ref(),
    )
    .map_err(|error| map_runtime_error(error.into()))?;
    if !metadata.record_capabilities.can_list && !metadata.record_capabilities.can_get {
        return Err(map_runtime_error(
            runtime_core::runtime_engine::RuntimeModelError::record_action_not_allowed(
                model_code, "view",
            )
            .into(),
        ));
    }
    let record_id_field = record_id_field(&metadata);
    let capabilities = &metadata.record_capabilities;
    let record_capabilities = RuntimeDataModelRecordCapabilities {
        can_list: capabilities.can_list,
        can_get: capabilities.can_get && record_id_field.is_some(),
        can_create: capabilities.can_create
            && action_allowed(adapter, actor, metadata.model_id, RuntimeDataAction::Create).await?,
        can_update: capabilities.can_update
            && record_id_field.is_some()
            && action_allowed(adapter, actor, metadata.model_id, RuntimeDataAction::Update).await?,
        can_delete: capabilities.can_delete
            && record_id_field.is_some()
            && action_allowed(adapter, actor, metadata.model_id, RuntimeDataAction::Delete).await?,
    };
    let mut relation_codes = std::collections::HashMap::new();
    let targets = metadata
        .fields
        .iter()
        .filter_map(|field| field.relation_target_model_id)
        .collect::<std::collections::HashSet<_>>();
    for target in adapter
        .runtime_engine
        .registry()
        .list_available_for_workspace(actor.current_workspace_id)
    {
        if !targets.contains(&target.model_id)
            || (!target.record_capabilities.can_list && !target.record_capabilities.can_get)
        {
            continue;
        }
        // Resolve by code again: a workspace model shadows a same-code system model.
        if !adapter
            .runtime_engine
            .load_available_metadata(&target.model_code, actor.current_workspace_id)
            .is_ok_and(|resolved| resolved.model_id == target.model_id)
        {
            continue;
        }
        if action_allowed(adapter, actor, target.model_id, RuntimeDataAction::View).await? {
            relation_codes.insert(target.model_id, target.model_code);
        }
    }
    let fields = metadata
        .fields
        .into_iter()
        .map(|field| RuntimeDataModelField {
            relation_target_model_code: field
                .relation_target_model_id
                .and_then(|id| relation_codes.get(&id).cloned()),
            id: field.id,
            code: field.code,
            title: field.title,
            description: field.description,
            field_kind: field.field_kind,
            is_required: field.is_required,
            api_required: field.api_required,
            is_writable: field.is_writable,
            default_value: field.default_value,
            display_interface: field.display_interface,
            display_options: field.display_options,
            relation_target_model_id: field.relation_target_model_id,
            relation_options: field.relation_options,
        })
        .collect();
    Ok(RuntimeDataModelMetadata {
        model_id: metadata.model_id,
        model_code: metadata.model_code,
        scope_kind: metadata.scope_kind,
        scope_id: metadata.scope_id,
        record_id_field,
        fields,
        record_capabilities,
    })
}

async fn action_allowed(
    adapter: &RuntimeModelOperationAdapter,
    actor: &domain::ActorContext,
    model_id: Uuid,
    action: RuntimeDataAction,
) -> Result<bool, ApiError> {
    let grant = load_runtime_scope_grant(adapter, actor, model_id, action).await?;
    Ok(resolve_access_scope(actor, action, model_id, grant.as_ref()).is_ok())
}

fn record_id_field(metadata: &runtime_core::model_metadata::ModelMetadata) -> Option<String> {
    if metadata.source_kind == domain::DataModelSourceKind::ExternalSource {
        // Resource mapping persists descriptor.primary_key as the unique field. Missing or
        // ambiguous source identity must not silently become the platform's native UUID id.
        let mut keys = metadata.fields.iter().filter(|field| field.is_unique);
        let key = keys.next()?;
        return keys.next().is_none().then(|| key.code.clone());
    }
    // Native runtime CRUD generates an id even for models without declared fields.
    Some(
        metadata
            .fields
            .iter()
            .find(|field| field.physical_column_name == "id")
            .map(|field| field.code.clone())
            .unwrap_or_else(|| "id".into()),
    )
}
