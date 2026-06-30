use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use plugin_framework::data_source_contract::{
    DataSourceConfigInput, DataSourceCreateRecordInput, DataSourceCreateRecordOutput,
    DataSourceDeleteRecordInput, DataSourceDeleteRecordOutput, DataSourceGetRecordInput,
    DataSourceGetRecordOutput, DataSourceListRecordsInput, DataSourceListRecordsOutput,
    DataSourceRecordFilter, DataSourceRecordPage, DataSourceRecordScopeContext,
    DataSourceRecordSort, DataSourceUpdateRecordInput, DataSourceUpdateRecordOutput,
};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    capability_slots::{DefaultValueResolver, RecordValidator},
    model_metadata::ModelMetadata,
    runtime_acl::{resolve_access_scope, RuntimeDataAction, RuntimeScopeGrant},
    runtime_model_registry::{
        RegisteredRuntimeModel, RuntimeDataModelAvailability, RuntimeModelRegistry,
    },
    runtime_record_repository::RuntimeRecordRepository,
};

pub use crate::runtime_record_repository::{RuntimeListQuery, RuntimeListResult, RuntimeSortInput};

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

#[async_trait]
pub trait DataSourceRuntimeRecordBackend: Send + Sync {
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
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuntimeModelError {
    #[error("runtime model unavailable: {0}")]
    Unavailable(String),
    #[error("runtime model not published: {0}")]
    NotPublished(String),
    #[error("runtime model disabled: {0}")]
    Disabled(String),
    #[error("runtime model broken: {0}")]
    Broken(String),
    #[error("runtime model record action not allowed: {model_code}:{action}")]
    RecordActionNotAllowed {
        model_code: String,
        action: &'static str,
    },
    #[error("runtime model create missing api required fields: {model_code}:{fields:?}")]
    MissingCreateRequiredFields {
        model_code: String,
        fields: Vec<String>,
    },
}

impl RuntimeModelError {
    pub fn unavailable(model_code: impl Into<String>) -> Self {
        Self::Unavailable(model_code.into())
    }

    pub fn not_published(model_code: impl Into<String>) -> Self {
        Self::NotPublished(model_code.into())
    }

    pub fn disabled(model_code: impl Into<String>) -> Self {
        Self::Disabled(model_code.into())
    }

    pub fn broken(model_code: impl Into<String>) -> Self {
        Self::Broken(model_code.into())
    }

    pub fn record_action_not_allowed(model_code: impl Into<String>, action: &'static str) -> Self {
        Self::RecordActionNotAllowed {
            model_code: model_code.into(),
            action,
        }
    }

    pub fn missing_create_required_fields<I, S>(model_code: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::MissingCreateRequiredFields {
            model_code: model_code.into(),
            fields: fields.into_iter().map(Into::into).collect(),
        }
    }
}

pub fn ensure_runtime_model_available(
    model_code: &str,
    availability: RuntimeDataModelAvailability,
) -> Result<()> {
    match availability {
        RuntimeDataModelAvailability::Available => Ok(()),
        RuntimeDataModelAvailability::NotPublished => {
            Err(RuntimeModelError::not_published(model_code).into())
        }
        RuntimeDataModelAvailability::Disabled => {
            Err(RuntimeModelError::disabled(model_code).into())
        }
        RuntimeDataModelAvailability::Broken => Err(RuntimeModelError::broken(model_code).into()),
    }
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
        RuntimeDataAction::Edit => metadata.record_capabilities.can_update,
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
                RuntimeDataAction::Edit => "update",
                RuntimeDataAction::Delete => "delete",
            },
        )
        .into())
    }
}

fn ensure_create_api_required_fields(metadata: &ModelMetadata, payload: &Value) -> Result<()> {
    let missing_fields = metadata
        .fields
        .iter()
        .filter(|field| field.api_required && field.is_writable)
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

#[derive(Clone)]
pub struct RuntimeEngine {
    default_value_resolver: Arc<dyn DefaultValueResolver>,
    validator: Arc<dyn RecordValidator>,
    registry: RuntimeModelRegistry,
    records: Arc<dyn RuntimeRecordRepository>,
    data_source_records: Option<Arc<dyn DataSourceRuntimeRecordBackend>>,
}

impl RuntimeEngine {
    pub fn new(registry: RuntimeModelRegistry, records: Arc<dyn RuntimeRecordRepository>) -> Self {
        Self {
            default_value_resolver: Arc::new(PassthroughValueResolver),
            validator: Arc::new(NoopRecordValidator),
            registry,
            records,
            data_source_records: None,
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
            records,
            data_source_records: Some(data_source_records),
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

    pub fn registry(&self) -> &RuntimeModelRegistry {
        &self.registry
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

        let query = RuntimeListQuery {
            scope_id: access_scope.scope_id,
            owner_user_id: access_scope.owner_user_id,
            filter: input.filter,
            sorts: input.sorts,
            expand_relations: input.expand_relations,
            page: input.page,
            page_size: input.page_size,
        };

        match metadata.source_kind {
            domain::DataModelSourceKind::MainSource => {
                self.records.list_records(&metadata, query).await
            }
            domain::DataModelSourceKind::ExternalSource => {
                self.list_external_records(input.actor.current_workspace_id, &metadata, query)
                    .await
            }
        }
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

        match metadata.source_kind {
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
        }
    }

    pub async fn create_record(&self, input: RuntimeCreateInput) -> Result<Value> {
        let metadata =
            self.load_available_metadata(&input.model_code, input.actor.current_workspace_id)?;
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
        ensure_create_api_required_fields(&metadata, &payload)?;
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
        ensure_runtime_record_action_allowed(&metadata, RuntimeDataAction::Edit)?;
        let access_scope = resolve_access_scope(
            &input.actor,
            RuntimeDataAction::Edit,
            metadata.model_id,
            input.scope_grant.as_ref(),
        )?;
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

#[derive(Default)]
struct InMemoryRuntimeRecordRepository {
    records: Mutex<HashMap<String, HashMap<Uuid, Vec<Value>>>>,
}

#[async_trait]
impl RuntimeRecordRepository for InMemoryRuntimeRecordRepository {
    async fn list_records(
        &self,
        metadata: &ModelMetadata,
        query: RuntimeListQuery,
    ) -> Result<RuntimeListResult> {
        let page = query.page.max(1);
        let page_size = query.page_size.max(1);
        let records = self.records.lock().expect("runtime record lock poisoned");
        let mut items = records
            .get(&metadata.model_code)
            .map(|scopes| match query.scope_id {
                Some(scope_id) => scopes.get(&scope_id).cloned().unwrap_or_default(),
                None => scopes
                    .values()
                    .flat_map(|items| items.iter().cloned())
                    .collect(),
            })
            .unwrap_or_default();
        items.retain(|item| owner_matches(item, query.owner_user_id));
        items.retain(|item| filter_matches(item, &query.filter));
        items.sort_by(|left, right| compare_records(left, right, &query.sorts));
        let total = items.len() as i64;
        let offset = ((page - 1) * page_size) as usize;
        let items = items
            .into_iter()
            .skip(offset)
            .take(page_size as usize)
            .collect();

        Ok(RuntimeListResult { items, total })
    }

    async fn get_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
    ) -> Result<Option<Value>> {
        let records = self.records.lock().expect("runtime record lock poisoned");
        let Some(scopes) = records.get(&metadata.model_code) else {
            return Ok(None);
        };
        let find_record = |items: &Vec<Value>| {
            items
                .iter()
                .find(|item| {
                    item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id)
                })
                .cloned()
        };

        Ok(match scope_id {
            Some(scope_id) => scopes.get(&scope_id).and_then(find_record),
            None => scopes.values().find_map(find_record),
        })
    }

    async fn create_record(
        &self,
        metadata: &ModelMetadata,
        actor_user_id: Uuid,
        scope_id: Uuid,
        payload: Value,
    ) -> Result<Value> {
        let mut record = object_payload(payload);
        record
            .entry("id".to_string())
            .or_insert_with(|| serde_json::json!(Uuid::now_v7()));
        record.insert(
            "created_by".to_string(),
            nullable_actor_user_id(actor_user_id)
                .map(|user_id| serde_json::Value::String(user_id.to_string()))
                .unwrap_or(serde_json::Value::Null),
        );
        let value = Value::Object(record);

        let mut records = self.records.lock().expect("runtime record lock poisoned");
        records
            .entry(metadata.model_code.clone())
            .or_default()
            .entry(scope_id)
            .or_default()
            .push(value.clone());

        Ok(value)
    }

    async fn update_record(
        &self,
        metadata: &ModelMetadata,
        _actor_user_id: Uuid,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
        payload: Value,
    ) -> Result<Value> {
        let patch = object_payload(payload);
        let mut records = self.records.lock().expect("runtime record lock poisoned");
        let scopes = records.entry(metadata.model_code.clone()).or_default();
        let scoped_records = match scope_id {
            Some(scope_id) => scopes.entry(scope_id).or_default(),
            None => scopes
                .values_mut()
                .find(|items| {
                    items.iter().any(|item| {
                        item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id)
                    })
                })
                .ok_or_else(|| anyhow!("runtime record not found"))?,
        };
        let record = scoped_records
            .iter_mut()
            .find(|item| {
                item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id)
            })
            .ok_or_else(|| anyhow!("runtime record not found"))?;
        let object = record
            .as_object_mut()
            .ok_or_else(|| anyhow!("runtime record must be object"))?;

        for (key, value) in patch {
            object.insert(key, value);
        }

        Ok(record.clone())
    }

    async fn delete_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
    ) -> Result<bool> {
        let mut records = self.records.lock().expect("runtime record lock poisoned");
        let mut deleted = false;
        let mut retain_record = |item: &Value| {
            let matches =
                item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id);
            if matches {
                deleted = true;
                false
            } else {
                true
            }
        };
        let scopes = records.entry(metadata.model_code.clone()).or_default();
        match scope_id {
            Some(scope_id) => {
                scopes
                    .entry(scope_id)
                    .or_default()
                    .retain(&mut retain_record);
            }
            None => {
                for records in scopes.values_mut() {
                    records.retain(&mut retain_record);
                }
            }
        }

        Ok(deleted)
    }
}

struct NoopRecordValidator;

#[async_trait]
impl RecordValidator for NoopRecordValidator {
    async fn validate(
        &self,
        _actor_user_id: Uuid,
        _model_code: &str,
        _payload: &Value,
    ) -> Result<()> {
        Ok(())
    }
}

struct PassthroughValueResolver;

#[async_trait]
impl DefaultValueResolver for PassthroughValueResolver {
    async fn apply(
        &self,
        _actor_user_id: Uuid,
        _model_code: &str,
        payload: Value,
    ) -> Result<Value> {
        Ok(Value::Object(object_payload(payload)))
    }
}

fn object_payload(payload: Value) -> serde_json::Map<String, Value> {
    match payload {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("value".to_string(), other);
            map
        }
    }
}

fn nullable_actor_user_id(actor_user_id: Uuid) -> Option<Uuid> {
    (!actor_user_id.is_nil()).then_some(actor_user_id)
}

fn owner_matches(record: &Value, owner_user_id: Option<Uuid>) -> bool {
    match owner_user_id {
        None => true,
        Some(owner_user_id) => {
            record
                .get("created_by")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                == Some(owner_user_id)
        }
    }
}

fn filter_matches(record: &Value, filter: &domain::ResourceFilterExpr) -> bool {
    match filter {
        domain::ResourceFilterExpr::All(items) => {
            items.iter().all(|item| filter_matches(record, item))
        }
        domain::ResourceFilterExpr::Any(items) => {
            items.iter().any(|item| filter_matches(record, item))
        }
        domain::ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => {
            let current = &record[field];
            match operator {
                domain::ResourceFilterOperator::Eq => current == value,
                domain::ResourceFilterOperator::Ne => current != value,
                domain::ResourceFilterOperator::Gt => {
                    compare_json_values(current, value) == Ordering::Greater
                }
                domain::ResourceFilterOperator::Gte => matches!(
                    compare_json_values(current, value),
                    Ordering::Greater | Ordering::Equal
                ),
                domain::ResourceFilterOperator::Lt => {
                    compare_json_values(current, value) == Ordering::Less
                }
                domain::ResourceFilterOperator::Lte => matches!(
                    compare_json_values(current, value),
                    Ordering::Less | Ordering::Equal
                ),
                domain::ResourceFilterOperator::Includes => current
                    .as_str()
                    .zip(value.as_str())
                    .is_some_and(|(current, value)| {
                        current.to_lowercase().contains(&value.to_lowercase())
                    }),
                domain::ResourceFilterOperator::NotIncludes => current
                    .as_str()
                    .zip(value.as_str())
                    .is_none_or(|(current, value)| {
                        !current.to_lowercase().contains(&value.to_lowercase())
                    }),
                domain::ResourceFilterOperator::In => value
                    .as_array()
                    .is_some_and(|values| values.iter().any(|value| current == value)),
            }
        }
    }
}

fn compare_records(left: &Value, right: &Value, sorts: &[RuntimeSortInput]) -> Ordering {
    for sort in sorts {
        let ordering = compare_json_values(&left[&sort.field_code], &right[&sort.field_code]);
        if ordering != Ordering::Equal {
            return if sort.direction.eq_ignore_ascii_case("desc") {
                ordering.reverse()
            } else {
                ordering
            };
        }
    }

    Ordering::Equal
}

fn compare_json_values(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.cmp(right),
        (Value::Number(left), Value::Number(right)) => left
            .as_f64()
            .partial_cmp(&right.as_f64())
            .unwrap_or(Ordering::Equal),
        (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
        _ => left.to_string().cmp(&right.to_string()),
    }
}

fn test_model_metadata() -> ModelMetadata {
    ModelMetadata {
        model_id: Uuid::nil(),
        model_code: "orders".into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: Uuid::nil(),
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: "rtm_workspace_demo_orders".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![
            domain::ModelFieldRecord {
                id: Uuid::nil(),
                data_model_id: Uuid::nil(),
                code: "title".into(),
                title: "Title".into(),
                description: None,
                physical_column_name: "title".into(),
                external_field_key: None,
                field_kind: domain::ModelFieldKind::String,
                is_system: false,
                is_writable: true,
                is_required: true,
                api_required: true,
                is_unique: false,
                default_value: None,
                display_interface: Some("input".into()),
                display_options: serde_json::json!({}),
                relation_target_model_id: None,
                relation_options: serde_json::json!({}),
                sort_order: 0,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
            domain::ModelFieldRecord {
                id: Uuid::nil(),
                data_model_id: Uuid::nil(),
                code: "status".into(),
                title: "Status".into(),
                description: None,
                physical_column_name: "status".into(),
                external_field_key: None,
                field_kind: domain::ModelFieldKind::Enum,
                is_system: false,
                is_writable: true,
                is_required: true,
                api_required: true,
                is_unique: false,
                default_value: None,
                display_interface: Some("select".into()),
                display_options: serde_json::json!({ "options": ["draft", "paid"] }),
                relation_target_model_id: None,
                relation_options: serde_json::json!({}),
                sort_order: 1,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
        ],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: crate::resource_descriptor::ResourceDescriptor::runtime_model(
            "orders",
            domain::DataModelScopeKind::Workspace,
        ),
    }
}
