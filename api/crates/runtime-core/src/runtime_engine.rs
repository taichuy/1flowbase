use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use extension_contracts::data_source_contract::{
    DataSourceConfigInput, DataSourceCreateRecordInput, DataSourceCreateRecordOutput,
    DataSourceDeleteRecordInput, DataSourceDeleteRecordOutput,
    DataSourceExecuteModelOperationInput, DataSourceGetRecordInput, DataSourceGetRecordOutput,
    DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourceModelOperationActorContext,
    DataSourceModelOperationScopeContext, DataSourceRecordFilter, DataSourceRecordPage,
    DataSourceRecordScopeContext, DataSourceRecordSort, DataSourceUpdateRecordInput,
    DataSourceUpdateRecordOutput, NativeSqlExecutionOutput,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    capability_slots::{DefaultValueResolver, RecordValidator},
    model_metadata::ModelMetadata,
    runtime_acl::{resolve_access_scope, RuntimeDataAction, RuntimeScopeGrant},
    runtime_model_registry::{RegisteredRuntimeModel, RuntimeModelRegistry},
    runtime_record_repository::{OrderedTreeRuntimeRepository, RuntimeRecordRepository},
};

pub mod computed_fields;
mod file_upload;
pub use file_upload::RuntimeUploadedFileInput;

pub use crate::runtime_record_repository::{RuntimeListQuery, RuntimeListResult, RuntimeSortInput};
pub use storage_durable::runtime_model_availability::{
    ensure_runtime_model_available, RuntimeModelError,
};

#[derive(Debug, Clone)]
pub struct RuntimeListInput {
    pub actor: domain::ActorContext,
    pub model_code: String,
    pub scope_grant: Option<RuntimeScopeGrant>,
    pub filter: domain::ResourceFilterExpr,
    pub sorts: Vec<RuntimeSortInput>,
    pub expand_relations: Vec<String>,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct RuntimeGetInput {
    pub actor: domain::ActorContext,
    pub model_code: String,
    pub record_id: String,
    pub scope_grant: Option<RuntimeScopeGrant>,
}

#[derive(Debug, Clone)]
pub struct RuntimeCreateInput {
    pub actor: domain::ActorContext,
    pub model_code: String,
    pub payload: Value,
    pub scope_grant: Option<RuntimeScopeGrant>,
}

#[derive(Debug, Clone)]
pub struct RuntimeUpdateInput {
    pub actor: domain::ActorContext,
    pub model_code: String,
    pub record_id: String,
    pub payload: Value,
    pub scope_grant: Option<RuntimeScopeGrant>,
}

#[derive(Debug, Clone)]
pub struct RuntimeDeleteInput {
    pub actor: domain::ActorContext,
    pub model_code: String,
    pub record_id: String,
    pub scope_grant: Option<RuntimeScopeGrant>,
}

#[derive(Debug, Clone)]
pub struct RuntimeModelOperationInput {
    pub actor: domain::ActorContext,
    pub model_code: String,
    pub operation_code: String,
    pub payload: Value,
    pub path: Value,
    pub query: Value,
    pub scope_grant: Option<RuntimeScopeGrant>,
}

#[async_trait]
pub trait DataSourceRuntimeRecordBackend: Send + Sync {
    async fn execute_sql(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: &str,
        sql: &str,
    ) -> Result<NativeSqlExecutionOutput> {
        let _ = (workspace_id, data_source_instance_id, sql);
        Err(anyhow!(
            "native SQL is not implemented by this data source backend"
        ))
    }

    async fn list_records(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceListRecordsInput,
    ) -> Result<DataSourceListRecordsOutput>;

    async fn get_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceGetRecordInput,
    ) -> Result<DataSourceGetRecordOutput>;

    async fn create_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceCreateRecordInput,
    ) -> Result<DataSourceCreateRecordOutput>;

    async fn update_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceUpdateRecordInput,
    ) -> Result<DataSourceUpdateRecordOutput>;

    async fn delete_record(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceDeleteRecordInput,
    ) -> Result<DataSourceDeleteRecordOutput>;

    async fn execute_model_operation(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        expected_capabilities: extension_contracts::DataSourceCrudCapabilities,
        input: DataSourceExecuteModelOperationInput,
    ) -> Result<Value>;
}

fn operation_object(value: Value) -> Result<serde_json::Map<String, Value>> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| RuntimeModelError::InvalidOperationInput("payload").into())
}

fn take_optional_uuid(
    object: &mut serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<Uuid>> {
    match object.remove(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Uuid::parse_str(&value)
            .map(Some)
            .map_err(|_| RuntimeModelError::InvalidOperationField(field.to_owned()).into()),
        Some(_) => Err(RuntimeModelError::InvalidOperationField(field.to_owned()).into()),
    }
}

fn operation_uuid(value: &Value, field: &str) -> Result<Uuid> {
    value
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| RuntimeModelError::InvalidOperationField(field.to_owned()).into())
}

fn operation_u32(value: &Value, field: &str, default: u32) -> Result<u32> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Number(value)) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| RuntimeModelError::InvalidOperationField(field.to_owned()).into()),
        Some(Value::String(value)) => value
            .parse::<u32>()
            .map_err(|_| RuntimeModelError::InvalidOperationField(field.to_owned()).into()),
        Some(_) => Err(RuntimeModelError::InvalidOperationField(field.to_owned()).into()),
    }
}

fn operation_u64(value: &Value, field: &str) -> Result<u64> {
    let value = match value.get(field) {
        Some(Value::Number(value)) => value.as_u64(),
        Some(Value::String(value)) => value.parse::<u64>().ok(),
        _ => None,
    }
    .ok_or_else(|| RuntimeModelError::InvalidOperationField(field.to_owned()))?;
    if value == 0 {
        return Err(RuntimeModelError::InvalidOperationField(field.to_owned()).into());
    }
    Ok(value)
}

fn operation_bool(value: &Value, field: &str, default: bool) -> Result<bool> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::String(value)) => value
            .parse::<bool>()
            .map_err(|_| RuntimeModelError::InvalidOperationField(field.to_owned()).into()),
        Some(_) => Err(RuntimeModelError::InvalidOperationField(field.to_owned()).into()),
    }
}

fn operation_string(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| RuntimeModelError::InvalidOperationField(field.to_owned()).into())
}

fn reject_structure_fields(payload: &Value) -> Result<()> {
    let payload = payload
        .as_object()
        .ok_or(RuntimeModelError::InvalidOperationInput("payload"))?;
    for field in ["parent_id", "sibling_rank"] {
        if payload.contains_key(field) {
            return Err(RuntimeModelError::InvalidOperationField(field.to_owned()).into());
        }
    }
    Ok(())
}

fn ensure_runtime_record_action_allowed(
    metadata: &ModelMetadata,
    action: RuntimeDataAction,
) -> Result<()> {
    let allowed = match action {
        RuntimeDataAction::View => {
            metadata.record_capabilities.can_list || metadata.record_capabilities.can_get
        }
        RuntimeDataAction::Create => metadata.record_capabilities.can_create,
        RuntimeDataAction::Update => metadata.record_capabilities.can_update,
        RuntimeDataAction::Delete => metadata.record_capabilities.can_delete,
    };
    if allowed {
        Ok(())
    } else {
        Err(RuntimeModelError::record_action_not_allowed(
            &metadata.model_code,
            match action {
                RuntimeDataAction::View => "view",
                RuntimeDataAction::Create => "create",
                RuntimeDataAction::Update => "update",
                RuntimeDataAction::Delete => "delete",
            },
        )
        .into())
    }
}

fn ensure_create_api_required_fields(
    metadata: &ModelMetadata,
    template: &crate::data_model_template_registry::CompiledDataModelTemplate,
    payload: &Value,
) -> Result<()> {
    let missing_fields = metadata
        .fields
        .iter()
        .filter(|field| field.api_required && field.is_writable)
        .filter(|field| {
            !template
                .descriptor()
                .system_fields
                .iter()
                .any(|system_field| {
                    system_field.code == field.code
                        || system_field.code == field.physical_column_name
                })
        })
        .filter(|field| {
            payload
                .as_object()
                .is_none_or(|payload| !payload.contains_key(&field.code))
        })
        .map(|field| field.code.clone())
        .collect::<Vec<_>>();

    if missing_fields.is_empty() {
        return Ok(());
    }

    Err(
        RuntimeModelError::missing_create_required_fields(&metadata.model_code, missing_fields)
            .into(),
    )
}

fn ensure_runtime_payload_fields_writable(
    metadata: &ModelMetadata,
    template: &crate::data_model_template_registry::CompiledDataModelTemplate,
    payload: &Value,
) -> Result<()> {
    let Some(payload) = payload.as_object() else {
        return Ok(());
    };
    let is_system_field = |field_code: &str| {
        template
            .descriptor()
            .system_fields
            .iter()
            .any(|field| field.code == field_code)
    };

    for field_code in payload.keys() {
        if is_system_field(field_code) {
            return Err(anyhow!("runtime field is not writable: {field_code}"));
        }
        let Some(field) = metadata.field_by_code(field_code) else {
            continue;
        };
        if field.field_kind == domain::ModelFieldKind::FilePreviewUrl {
            return Err(RuntimeModelError::InvalidOperationField(field_code.clone()).into());
        }
        if field.is_system || !field.is_writable || is_system_field(&field.physical_column_name) {
            return Err(anyhow!("runtime field is not writable: {field_code}"));
        }
    }

    Ok(())
}

#[derive(Clone)]
pub struct RuntimeEngine {
    default_value_resolver: Arc<dyn DefaultValueResolver>,
    validator: Arc<dyn RecordValidator>,
    registry: RuntimeModelRegistry,
    computed_fields: Option<Arc<dyn computed_fields::RuntimeComputedFieldResolver>>,
    records: Arc<dyn RuntimeRecordRepository>,
    ordered_tree_records: Option<Arc<dyn OrderedTreeRuntimeRepository>>,
    data_source_records: Option<Arc<dyn DataSourceRuntimeRecordBackend>>,
    template_catalog: crate::data_model_template_registry::DataModelTemplateCatalog,
}

impl RuntimeEngine {
    pub fn new(registry: RuntimeModelRegistry, records: Arc<dyn RuntimeRecordRepository>) -> Self {
        Self {
            default_value_resolver: Arc::new(PassthroughValueResolver),
            validator: Arc::new(NoopRecordValidator),
            registry,
            computed_fields: None,
            records,
            ordered_tree_records: None,
            data_source_records: None,
            template_catalog: crate::data_model_template_registry::DataModelTemplateCatalog::core(),
        }
    }

    pub fn new_with_data_source_backend(
        registry: RuntimeModelRegistry,
        records: Arc<dyn RuntimeRecordRepository>,
        data_source_records: Arc<dyn DataSourceRuntimeRecordBackend>,
    ) -> Self {
        Self {
            default_value_resolver: Arc::new(PassthroughValueResolver),
            validator: Arc::new(NoopRecordValidator),
            registry,
            computed_fields: None,
            records,
            ordered_tree_records: None,
            data_source_records: Some(data_source_records),
            template_catalog: crate::data_model_template_registry::DataModelTemplateCatalog::core(),
        }
    }

    pub fn new_with_data_source_backend_and_templates(
        registry: RuntimeModelRegistry,
        records: Arc<dyn RuntimeRecordRepository>,
        data_source_records: Arc<dyn DataSourceRuntimeRecordBackend>,
        template_catalog: crate::data_model_template_registry::DataModelTemplateCatalog,
    ) -> Self {
        Self {
            default_value_resolver: Arc::new(PassthroughValueResolver),
            validator: Arc::new(NoopRecordValidator),
            registry,
            computed_fields: None,
            records,
            ordered_tree_records: None,
            data_source_records: Some(data_source_records),
            template_catalog,
        }
    }

    pub fn new_with_data_source_backend_templates_and_ordered_tree(
        registry: RuntimeModelRegistry,
        records: Arc<dyn RuntimeRecordRepository>,
        ordered_tree_records: Arc<dyn OrderedTreeRuntimeRepository>,
        data_source_records: Arc<dyn DataSourceRuntimeRecordBackend>,
        template_catalog: crate::data_model_template_registry::DataModelTemplateCatalog,
    ) -> Self {
        Self {
            default_value_resolver: Arc::new(PassthroughValueResolver),
            validator: Arc::new(NoopRecordValidator),
            registry,
            computed_fields: None,
            records,
            ordered_tree_records: Some(ordered_tree_records),
            data_source_records: Some(data_source_records),
            template_catalog,
        }
    }

    pub fn for_tests() -> Self {
        Self::for_tests_with_models(vec![test_model_metadata()])
    }

    pub fn for_tests_with_models(models: Vec<ModelMetadata>) -> Self {
        let registry = RuntimeModelRegistry::default();
        registry.rebuild(models);

        Self::new(
            registry,
            Arc::new(InMemoryRuntimeRecordRepository::default()),
        )
    }

    pub fn for_tests_with_models_and_data_source_backend(
        models: Vec<ModelMetadata>,
        data_source_records: Arc<dyn DataSourceRuntimeRecordBackend>,
    ) -> Self {
        let registry = RuntimeModelRegistry::default();
        registry.rebuild(models);

        Self::new_with_data_source_backend(
            registry,
            Arc::new(InMemoryRuntimeRecordRepository::default()),
            data_source_records,
        )
    }

    pub fn for_tests_with_models_data_source_backend_and_templates(
        models: Vec<ModelMetadata>,
        data_source_records: Arc<dyn DataSourceRuntimeRecordBackend>,
        template_catalog: crate::data_model_template_registry::DataModelTemplateCatalog,
    ) -> Self {
        let registry = RuntimeModelRegistry::default();
        registry.rebuild(models);
        Self::new_with_data_source_backend_and_templates(
            registry,
            Arc::new(InMemoryRuntimeRecordRepository::default()),
            data_source_records,
            template_catalog,
        )
    }

    pub fn registry(&self) -> &RuntimeModelRegistry {
        &self.registry
    }

    pub fn template_catalog(
        &self,
    ) -> &crate::data_model_template_registry::DataModelTemplateCatalog {
        &self.template_catalog
    }

    pub fn template_for_model(
        &self,
        model_code: &str,
        workspace_id: Uuid,
    ) -> Result<crate::data_model_template_registry::CompiledDataModelTemplate> {
        let runtime_model = self.load_runtime_model(model_code, workspace_id)?;
        self.ensure_available(&runtime_model)?;
        self.resolve_template(&runtime_model.metadata)
    }

    pub async fn execute_model_operation(
        &self,
        input: RuntimeModelOperationInput,
    ) -> Result<Value> {
        let metadata =
            self.load_available_metadata(&input.model_code, input.actor.current_workspace_id)?;
        let template = self.resolve_template(&metadata)?;
        let operation = template
            .operation(&input.operation_code)
            .ok_or_else(|| RuntimeModelError::unavailable(&input.model_code))?;
        let data_action =
            crate::general_data_model_template::runtime_data_action(&operation.permission_action)
                .ok_or_else(|| RuntimeModelError::unavailable(&input.model_code))?;
        ensure_runtime_record_action_allowed(&metadata, data_action)?;
        let access_scope = resolve_access_scope(
            &input.actor,
            data_action,
            metadata.model_id,
            input.scope_grant.as_ref(),
        )?;
        if let Some(handler) =
            crate::ordered_tree_template::CoreOrderedTreeOperationHandler::from_ref(
                &operation.handler_ref,
            )
        {
            return self
                .execute_ordered_tree_operation(input, metadata, access_scope, handler)
                .await;
        }
        if metadata.source_kind != domain::DataModelSourceKind::ExternalSource {
            return Err(RuntimeModelError::unavailable(&input.model_code).into());
        }
        let expected_capabilities = metadata
            .external_capability_snapshot
            .clone()
            .ok_or_else(|| RuntimeModelError::unavailable(&input.model_code))?;
        let backend = self.external_backend()?;
        backend
            .execute_model_operation(
                input.actor.current_workspace_id,
                external_data_source_instance_id(&metadata)?,
                expected_capabilities,
                DataSourceExecuteModelOperationInput {
                    connection: DataSourceConfigInput::default(),
                    handler_ref: operation.handler_ref.clone(),
                    resource_key: external_resource_key(&metadata)?,
                    template_identity: template.identity().clone(),
                    operation_code: operation.code.clone(),
                    actor: DataSourceModelOperationActorContext {
                        actor_id: input.actor.user_id.to_string(),
                    },
                    scope: DataSourceModelOperationScopeContext {
                        scope_id: access_scope
                            .scope_id
                            .unwrap_or(domain::SYSTEM_SCOPE_ID)
                            .to_string(),
                    },
                    payload: input.payload,
                    path: input.path,
                    query: input.query,
                },
            )
            .await
    }

    async fn execute_ordered_tree_operation(
        &self,
        input: RuntimeModelOperationInput,
        metadata: ModelMetadata,
        access_scope: crate::runtime_acl::RuntimeAccessScope,
        handler: crate::ordered_tree_template::CoreOrderedTreeOperationHandler,
    ) -> Result<Value> {
        use crate::ordered_tree_template::CoreOrderedTreeOperationHandler as Handler;
        use crate::runtime_record_repository::{
            OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreeCreateInput,
            OrderedTreeCreatePosition, OrderedTreeDescendantsInput, OrderedTreeLeafDeleteInput,
            OrderedTreeMoveInput, OrderedTreeMovePosition, OrderedTreeNodeInput,
            OrderedTreeSearchInput, OrderedTreeSubtreeDeleteInput,
        };

        let repository = self
            .ordered_tree_records
            .as_ref()
            .ok_or(RuntimeModelError::OrderedTreeUnavailable)?;
        let scope_id = access_scope
            .scope_id
            .unwrap_or(input.actor.current_workspace_id);
        // Generic runtime routes retain one tree per scope. Domain-specific callers can
        // use the repository contract directly with a distinct structural partition.
        let tree_partition_id = scope_id;
        match handler {
            Handler::ListRecords | Handler::GetRecord => {
                Err(RuntimeModelError::InvalidOperationInput("ordered_tree_crud_adapter").into())
            }
            Handler::CreateRecord => {
                let mut payload = operation_object(input.payload)?;
                if payload.contains_key("sibling_rank") {
                    return Err(RuntimeModelError::InvalidOperationField(
                        "sibling_rank".to_owned(),
                    )
                    .into());
                }
                let parent_id = take_optional_uuid(&mut payload, "parent_id")?;
                let before_id = take_optional_uuid(&mut payload, "before_id")?;
                let after_id = take_optional_uuid(&mut payload, "after_id")?;
                let payload = Value::Object(payload);
                let template = self.resolve_template(&metadata)?;
                ensure_runtime_payload_fields_writable(&metadata, &template, &payload)?;
                ensure_create_api_required_fields(&metadata, &template, &payload)?;
                let payload = self
                    .default_value_resolver
                    .apply(input.actor.user_id, &metadata.model_code, payload)
                    .await?;
                self.validator
                    .validate(input.actor.user_id, &metadata.model_code, &payload)
                    .await?;
                let created = repository
                    .create_ordered_tree_node(
                        &metadata,
                        OrderedTreeCreateInput {
                            actor_user_id: input.actor.user_id,
                            scope_id,
                            tree_partition_id,
                            payload,
                            position: OrderedTreeCreatePosition {
                                parent_id,
                                before_id,
                                after_id,
                            },
                        },
                    )
                    .await?;
                self.records
                    .get_record(
                        &metadata,
                        Some(scope_id),
                        None,
                        &created.node_id.to_string(),
                    )
                    .await?
                    .ok_or_else(|| anyhow!("runtime record not found after tree create"))
            }
            Handler::UpdateRecord => {
                reject_structure_fields(&input.payload)?;
                self.update_record(RuntimeUpdateInput {
                    actor: input.actor,
                    model_code: input.model_code,
                    record_id: operation_uuid(&input.path, "id")?.to_string(),
                    payload: input.payload,
                    scope_grant: input.scope_grant,
                })
                .await
            }
            Handler::DeleteRecord => {
                let deleted = repository
                    .delete_ordered_tree_leaf(
                        &metadata,
                        OrderedTreeLeafDeleteInput {
                            scope_id,
                            tree_partition_id,
                            node_id: operation_uuid(&input.path, "id")?,
                        },
                    )
                    .await?;
                if !deleted {
                    return Err(
                        crate::runtime_record_repository::OrderedTreeCommandError::NodeNotFound
                            .into(),
                    );
                }
                Ok(serde_json::json!({ "deleted": true }))
            }
            Handler::ListRoots => {
                let nodes = repository
                    .list_ordered_tree_roots(
                        &metadata,
                        OrderedTreeBoundedListInput {
                            scope_id,
                            tree_partition_id,
                            cursor: operation_optional_string(&input.query, "cursor")?,
                            result_limit: operation_u32(&input.query, "limit", 100)?,
                        },
                    )
                    .await?;
                Ok(serde_json::to_value(nodes.map(|node| node.record))?)
            }
            Handler::ListChildren => {
                let nodes = repository
                    .list_ordered_tree_children(
                        &metadata,
                        OrderedTreeChildrenInput {
                            scope_id,
                            tree_partition_id,
                            parent_id: operation_uuid(&input.path, "id")?,
                            cursor: operation_optional_string(&input.query, "cursor")?,
                            result_limit: operation_u32(&input.query, "limit", 100)?,
                        },
                    )
                    .await?;
                Ok(serde_json::to_value(nodes.map(|node| node.record))?)
            }
            Handler::ListAncestors => {
                let nodes = repository
                    .list_ordered_tree_ancestors(
                        &metadata,
                        OrderedTreeNodeInput {
                            scope_id,
                            tree_partition_id,
                            node_id: operation_uuid(&input.path, "id")?,
                        },
                    )
                    .await?;
                Ok(Value::Array(
                    nodes.into_iter().map(|node| node.record).collect(),
                ))
            }
            Handler::ListDescendants => {
                let include_path = operation_bool(&input.query, "include_path", false)?;
                let nodes = repository
                    .list_ordered_tree_descendants(
                        &metadata,
                        OrderedTreeDescendantsInput {
                            scope_id,
                            tree_partition_id,
                            node_id: operation_uuid(&input.path, "id")?,
                            max_depth: input
                                .query
                                .get("max_depth")
                                .filter(|value| !value.is_null())
                                .map(|_| operation_u32(&input.query, "max_depth", 0))
                                .transpose()?,
                            cursor: operation_optional_string(&input.query, "cursor")?,
                            result_limit: operation_u32(&input.query, "limit", 100)?,
                            include_path,
                        },
                    )
                    .await?;
                Ok(serde_json::to_value(nodes.map(|node| serde_json::json!({
                    "record": node.record, "depth": node.depth, "has_children": node.has_children, "path": node.path
                })))?)
            }
            Handler::Search => {
                let prefix = operation_string(&input.query, "prefix")?;
                let nodes = repository
                    .search_ordered_tree_prefix(
                        &metadata,
                        OrderedTreeSearchInput {
                            scope_id,
                            tree_partition_id,
                            prefix,
                            cursor: operation_optional_string(&input.query, "cursor")?,
                            match_limit: operation_u32(&input.query, "limit", 20)?,
                        },
                    )
                    .await?;
                Ok(serde_json::to_value(nodes.map(|node| {
                    serde_json::json!({
                        "record": node.record, "is_match": node.is_match
                    })
                }))?)
            }
            Handler::Move => {
                let mut payload = operation_object(input.payload)?;
                let new_parent_id = take_optional_uuid(&mut payload, "new_parent_id")?;
                let before_id = take_optional_uuid(&mut payload, "before_id")?;
                let after_id = take_optional_uuid(&mut payload, "after_id")?;
                if !payload.is_empty() {
                    return Err(RuntimeModelError::InvalidOperationInput("payload").into());
                }
                repository
                    .move_ordered_tree_node(
                        &metadata,
                        OrderedTreeMoveInput {
                            actor_user_id: input.actor.user_id,
                            scope_id,
                            tree_partition_id,
                            node_id: operation_uuid(&input.path, "id")?,
                            position: OrderedTreeMovePosition {
                                new_parent_id,
                                before_id,
                                after_id,
                            },
                        },
                    )
                    .await?;
                Ok(serde_json::json!({ "moved": true }))
            }
            Handler::DeleteSubtree => {
                let expected_affected_count =
                    operation_u64(&input.payload, "expected_affected_count")?;
                let deleted = repository
                    .delete_ordered_tree_subtree(
                        &metadata,
                        OrderedTreeSubtreeDeleteInput {
                            scope_id,
                            tree_partition_id,
                            node_id: operation_uuid(&input.path, "id")?,
                            expected_affected_count,
                        },
                    )
                    .await?;
                Ok(serde_json::json!({ "deleted_count": deleted.deleted_count }))
            }
        }
    }

    pub async fn execute_native_sql(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: &str,
        sql: &str,
    ) -> Result<NativeSqlExecutionOutput> {
        let backend = self
            .data_source_records
            .as_ref()
            .ok_or_else(|| anyhow!("data source runtime is not configured"))?;
        backend
            .execute_sql(workspace_id, data_source_instance_id, sql)
            .await
    }

    pub async fn list_records(&self, input: RuntimeListInput) -> Result<RuntimeListResult> {
        let metadata =
            self.load_available_metadata(&input.model_code, input.actor.current_workspace_id)?;
        ensure_runtime_record_action_allowed(&metadata, RuntimeDataAction::View)?;
        let access_scope = resolve_access_scope(
            &input.actor,
            RuntimeDataAction::View,
            metadata.model_id,
            input.scope_grant.as_ref(),
        )?;

        computed_fields::validate_computed_query(&metadata, &input.filter, &input.sorts)?;
        let query = RuntimeListQuery {
            scope_id: access_scope.scope_id,
            owner_user_id: access_scope.owner_user_id,
            filter: input.filter,
            sorts: input.sorts,
            expand_relations: input.expand_relations,
            page: input.page,
            page_size: input.page_size,
        };

        let mut result = match metadata.source_kind {
            domain::DataModelSourceKind::MainSource => {
                self.records.list_records(&metadata, query).await
            }
            domain::DataModelSourceKind::ExternalSource => {
                self.list_external_records(input.actor.current_workspace_id, &metadata, query)
                    .await
            }
        }?;
        self.project_computed_fields(&metadata, &mut result.items)
            .await?;
        Ok(result)
    }

    pub async fn get_record(&self, input: RuntimeGetInput) -> Result<Option<Value>> {
        let metadata =
            self.load_available_metadata(&input.model_code, input.actor.current_workspace_id)?;
        ensure_runtime_record_action_allowed(&metadata, RuntimeDataAction::View)?;
        let access_scope = resolve_access_scope(
            &input.actor,
            RuntimeDataAction::View,
            metadata.model_id,
            input.scope_grant.as_ref(),
        )?;

        let mut result = match metadata.source_kind {
            domain::DataModelSourceKind::MainSource => {
                self.records
                    .get_record(
                        &metadata,
                        access_scope.scope_id,
                        access_scope.owner_user_id,
                        &input.record_id,
                    )
                    .await
            }
            domain::DataModelSourceKind::ExternalSource => {
                self.get_external_record(
                    input.actor.current_workspace_id,
                    &metadata,
                    access_scope,
                    input.record_id,
                )
                .await
            }
        }?;
        if let Some(record) = result.as_mut() {
            self.project_computed_fields(&metadata, std::slice::from_mut(record))
                .await?;
        }
        Ok(result)
    }

    pub async fn create_record(&self, input: RuntimeCreateInput) -> Result<Value> {
        let metadata =
            self.load_available_metadata(&input.model_code, input.actor.current_workspace_id)?;
        if domain::file_management::is_file_data_model_template(
            &metadata.template_provider,
            &metadata.template_code,
            &metadata.template_version,
        ) {
            return Err(anyhow!("file_record_requires_upload"));
        }
        ensure_runtime_record_action_allowed(&metadata, RuntimeDataAction::Create)?;
        let access_scope = resolve_access_scope(
            &input.actor,
            RuntimeDataAction::Create,
            metadata.model_id,
            input.scope_grant.as_ref(),
        )?;
        let scope_id = access_scope
            .scope_id
            .unwrap_or(input.actor.current_workspace_id);
        let payload = self
            .default_value_resolver
            .apply(input.actor.user_id, &input.model_code, input.payload)
            .await?;
        let template = self.resolve_template(&metadata)?;
        ensure_runtime_payload_fields_writable(&metadata, &template, &payload)?;
        ensure_create_api_required_fields(&metadata, &template, &payload)?;
        self.validator
            .validate(input.actor.user_id, &input.model_code, &payload)
            .await?;

        match metadata.source_kind {
            domain::DataModelSourceKind::MainSource => {
                self.records
                    .create_record(&metadata, input.actor.user_id, scope_id, payload)
                    .await
            }
            domain::DataModelSourceKind::ExternalSource => {
                self.create_external_record(
                    input.actor.current_workspace_id,
                    &metadata,
                    access_scope.scope_id,
                    Some(input.actor.user_id),
                    payload,
                )
                .await
            }
        }
    }

    pub async fn update_record(&self, input: RuntimeUpdateInput) -> Result<Value> {
        let metadata =
            self.load_available_metadata(&input.model_code, input.actor.current_workspace_id)?;
        ensure_runtime_record_action_allowed(&metadata, RuntimeDataAction::Update)?;
        let access_scope = resolve_access_scope(
            &input.actor,
            RuntimeDataAction::Update,
            metadata.model_id,
            input.scope_grant.as_ref(),
        )?;
        let template = self.resolve_template(&metadata)?;
        ensure_runtime_payload_fields_writable(&metadata, &template, &input.payload)?;
        self.validator
            .validate(input.actor.user_id, &input.model_code, &input.payload)
            .await?;

        match metadata.source_kind {
            domain::DataModelSourceKind::MainSource => {
                self.records
                    .update_record(
                        &metadata,
                        input.actor.user_id,
                        access_scope.scope_id,
                        access_scope.owner_user_id,
                        &input.record_id,
                        input.payload,
                    )
                    .await
            }
            domain::DataModelSourceKind::ExternalSource => {
                self.update_external_record(
                    input.actor.current_workspace_id,
                    &metadata,
                    access_scope,
                    input.record_id,
                    input.payload,
                )
                .await
            }
        }
    }

    pub async fn delete_record(&self, input: RuntimeDeleteInput) -> Result<Value> {
        let metadata =
            self.load_available_metadata(&input.model_code, input.actor.current_workspace_id)?;
        ensure_runtime_record_action_allowed(&metadata, RuntimeDataAction::Delete)?;
        let access_scope = resolve_access_scope(
            &input.actor,
            RuntimeDataAction::Delete,
            metadata.model_id,
            input.scope_grant.as_ref(),
        )?;
        let deleted = match metadata.source_kind {
            domain::DataModelSourceKind::MainSource => {
                self.records
                    .delete_record(
                        &metadata,
                        access_scope.scope_id,
                        access_scope.owner_user_id,
                        &input.record_id,
                    )
                    .await?
            }
            domain::DataModelSourceKind::ExternalSource => {
                self.delete_external_record(
                    input.actor.current_workspace_id,
                    &metadata,
                    access_scope,
                    input.record_id,
                )
                .await?
            }
        };

        if !deleted {
            return Err(anyhow!("runtime record not found"));
        }

        Ok(serde_json::json!({ "deleted": true }))
    }

    fn load_available_metadata(
        &self,
        model_code: &str,
        workspace_id: Uuid,
    ) -> Result<ModelMetadata> {
        let runtime_model = self.load_runtime_model(model_code, workspace_id)?;
        self.ensure_available(&runtime_model)?;
        self.resolve_template(&runtime_model.metadata)?;
        Ok(runtime_model.metadata)
    }

    fn load_runtime_model(
        &self,
        model_code: &str,
        workspace_id: Uuid,
    ) -> Result<RegisteredRuntimeModel> {
        self.registry
            .get_runtime_model(
                domain::DataModelScopeKind::Workspace,
                workspace_id,
                model_code,
            )
            .or_else(|| {
                self.registry.get_runtime_model(
                    domain::DataModelScopeKind::System,
                    domain::SYSTEM_SCOPE_ID,
                    model_code,
                )
            })
            .ok_or_else(|| RuntimeModelError::unavailable(model_code).into())
    }

    fn ensure_available(&self, runtime_model: &RegisteredRuntimeModel) -> Result<()> {
        ensure_runtime_model_available(
            &runtime_model.metadata.model_code,
            runtime_model.availability,
        )
    }

    fn resolve_template(
        &self,
        metadata: &ModelMetadata,
    ) -> Result<crate::data_model_template_registry::CompiledDataModelTemplate> {
        let identity = extension_contracts::DataModelTemplateIdentity {
            provider: metadata.template_provider.clone(),
            code: metadata.template_code.clone(),
            version: metadata.template_version.clone(),
        };
        self.template_catalog
            .resolve(&identity)
            .map_err(|_| RuntimeModelError::unavailable(&metadata.model_code).into())
    }

    async fn list_external_records(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        query: RuntimeListQuery,
    ) -> Result<RuntimeListResult> {
        let output = self
            .external_backend()?
            .list_records(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceListRecordsInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    context: data_source_context(query.scope_id, query.owner_user_id),
                    filters: external_filters(metadata, &query.filter)?,
                    sort: external_sorts(metadata, query.sorts)?,
                    page: Some(data_source_page(query.page, query.page_size)),
                    options_json: serde_json::json!({
                        "expand_relations": query.expand_relations
                    }),
                },
            )
            .await?;

        Ok(RuntimeListResult {
            total: output.total_count.unwrap_or(output.rows.len() as u64) as i64,
            items: output
                .rows
                .into_iter()
                .map(|record| external_record_to_runtime(metadata, record))
                .collect(),
        })
    }

    async fn get_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        access_scope: crate::runtime_acl::RuntimeAccessScope,
        record_id: String,
    ) -> Result<Option<Value>> {
        self.external_backend()?
            .get_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceGetRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record_id,
                    context: data_source_context(access_scope.scope_id, access_scope.owner_user_id),
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| {
                output
                    .record
                    .map(|record| external_record_to_runtime(metadata, record))
            })
    }

    async fn create_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        payload: Value,
    ) -> Result<Value> {
        let record = runtime_payload_to_external(metadata, payload)?;
        self.external_backend()?
            .create_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceCreateRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record,
                    context: data_source_context(scope_id, owner_user_id),
                    transaction_id: None,
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| external_record_to_runtime(metadata, output.record))
    }

    async fn update_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        access_scope: crate::runtime_acl::RuntimeAccessScope,
        record_id: String,
        payload: Value,
    ) -> Result<Value> {
        let patch = runtime_payload_to_external(metadata, payload)?;
        self.external_backend()?
            .update_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceUpdateRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record_id,
                    patch,
                    context: data_source_context(access_scope.scope_id, access_scope.owner_user_id),
                    transaction_id: None,
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| external_record_to_runtime(metadata, output.record))
    }

    async fn delete_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        access_scope: crate::runtime_acl::RuntimeAccessScope,
        record_id: String,
    ) -> Result<bool> {
        self.external_backend()?
            .delete_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceDeleteRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record_id,
                    context: data_source_context(access_scope.scope_id, access_scope.owner_user_id),
                    transaction_id: None,
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| output.deleted)
    }

    fn external_backend(&self) -> Result<&Arc<dyn DataSourceRuntimeRecordBackend>> {
        self.data_source_records
            .as_ref()
            .ok_or_else(|| anyhow!("external data source runtime backend is not configured"))
    }
}

fn external_data_source_instance_id(metadata: &ModelMetadata) -> Result<Uuid> {
    metadata
        .data_source_instance_id
        .ok_or_else(|| anyhow!("external data source instance is not configured"))
}

fn external_resource_key(metadata: &ModelMetadata) -> Result<String> {
    metadata
        .external_resource_key
        .clone()
        .ok_or_else(|| anyhow!("external resource key is not configured"))
}

fn external_filters(
    metadata: &ModelMetadata,
    filter: &domain::ResourceFilterExpr,
) -> Result<Vec<DataSourceRecordFilter>> {
    let mut filters = Vec::new();
    collect_external_filters(metadata, filter, &mut filters)?;
    Ok(filters)
}

fn collect_external_filters(
    metadata: &ModelMetadata,
    filter: &domain::ResourceFilterExpr,
    filters: &mut Vec<DataSourceRecordFilter>,
) -> Result<()> {
    match filter {
        domain::ResourceFilterExpr::All(items) => {
            for item in items {
                collect_external_filters(metadata, item, filters)?;
            }
            Ok(())
        }
        domain::ResourceFilterExpr::Any(_) => Err(anyhow!(
            "external data source filters do not support $or expressions"
        )),
        domain::ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => {
            filters.push(DataSourceRecordFilter {
                field_key: external_field_key(metadata, field)?,
                operator: runtime_filter_operator_code(*operator)?.to_string(),
                value: value.clone(),
            });
            Ok(())
        }
    }
}

fn runtime_filter_operator_code(operator: domain::ResourceFilterOperator) -> Result<&'static str> {
    match operator {
        domain::ResourceFilterOperator::Eq => Ok("eq"),
        domain::ResourceFilterOperator::Ne => Ok("ne"),
        domain::ResourceFilterOperator::Gt => Ok("gt"),
        domain::ResourceFilterOperator::Gte => Ok("gte"),
        domain::ResourceFilterOperator::Lt => Ok("lt"),
        domain::ResourceFilterOperator::Lte => Ok("lte"),
        domain::ResourceFilterOperator::Includes => Ok("includes"),
        domain::ResourceFilterOperator::NotIncludes => Ok("notIncludes"),
        domain::ResourceFilterOperator::In => Ok("in"),
    }
}

fn external_sorts(
    metadata: &ModelMetadata,
    sorts: Vec<RuntimeSortInput>,
) -> Result<Vec<DataSourceRecordSort>> {
    sorts
        .into_iter()
        .map(|sort| {
            let direction = sort.direction.to_ascii_lowercase();
            let descending = match direction.as_str() {
                "asc" => false,
                "desc" => true,
                _ => return Err(anyhow!("unsupported sort direction")),
            };

            Ok(DataSourceRecordSort {
                field_key: external_field_key(metadata, &sort.field_code)?,
                descending,
            })
        })
        .collect()
}

fn runtime_payload_to_external(metadata: &ModelMetadata, payload: Value) -> Result<Value> {
    let Value::Object(object) = payload else {
        return Err(anyhow!("runtime payload must be object"));
    };
    let mut mapped = serde_json::Map::new();

    for (field_code, value) in object {
        let field_key = external_field_key(metadata, &field_code)?;
        mapped.insert(field_key, value);
    }

    Ok(Value::Object(mapped))
}

fn external_record_to_runtime(metadata: &ModelMetadata, record: Value) -> Value {
    let Value::Object(object) = record else {
        return record;
    };
    let mut mapped = serde_json::Map::new();

    for (field_key, value) in object {
        if let Some(field_code) = runtime_field_code(metadata, &field_key) {
            mapped.insert(field_code, value);
        }
    }

    Value::Object(mapped)
}

fn external_field_key(metadata: &ModelMetadata, field_code: &str) -> Result<String> {
    if let Some(field) = metadata.field_by_code(field_code) {
        return Ok(field
            .external_field_key
            .clone()
            .unwrap_or_else(|| field.code.clone()));
    }

    if is_platform_runtime_field(field_code) {
        Ok(field_code.to_string())
    } else {
        Err(anyhow!("unknown runtime field: {field_code}"))
    }
}

fn runtime_field_code(metadata: &ModelMetadata, field_key: &str) -> Option<String> {
    if let Some(field) = metadata
        .fields
        .iter()
        .find(|field| field.external_field_key.as_deref().unwrap_or(&field.code) == field_key)
    {
        return Some(field.code.clone());
    }

    is_platform_runtime_field(field_key).then(|| field_key.to_string())
}

fn is_platform_runtime_field(field_code: &str) -> bool {
    matches!(
        field_code,
        "id" | "created_by" | "created_at" | "updated_by" | "updated_at" | "deleted_at"
    )
}

fn data_source_context(
    scope_id: Option<Uuid>,
    owner_user_id: Option<Uuid>,
) -> DataSourceRecordScopeContext {
    DataSourceRecordScopeContext {
        owner_id: owner_user_id.map(|id| id.to_string()),
        scope_id: scope_id.map(|id| id.to_string()),
    }
}

fn data_source_page(page: i64, page_size: i64) -> DataSourceRecordPage {
    let page = page.max(1) as u64;
    let page_size = page_size.max(1) as u64;
    DataSourceRecordPage {
        limit: Some(page_size.min(u32::MAX as u64) as u32),
        cursor: None,
        offset: Some((page - 1) * page_size),
    }
}

fn operation_optional_string(value: &Value, key: &str) -> Result<Option<String>> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(RuntimeModelError::InvalidOperationInput("cursor").into()),
    }
}

mod default_backends;
use default_backends::{
    test_model_metadata, InMemoryRuntimeRecordRepository, NoopRecordValidator,
    PassthroughValueResolver,
};
