use super::*;

pub(super) fn runtime_records_cacheable_metadata(
    adapter: &RuntimeModelOperationAdapter,
    actor: &domain::ActorContext,
    model_code: &str,
) -> Option<runtime_core::model_metadata::ModelMetadata> {
    let runtime_model = adapter
        .runtime_engine
        .registry()
        .get_runtime_model(
            domain::DataModelScopeKind::Workspace,
            actor.current_workspace_id,
            model_code,
        )
        .or_else(|| {
            adapter.runtime_engine.registry().get_runtime_model(
                domain::DataModelScopeKind::System,
                domain::SYSTEM_SCOPE_ID,
                model_code,
            )
        })?;
    runtime_core::runtime_engine::ensure_runtime_model_available(
        &runtime_model.metadata.model_code,
        runtime_model.availability,
    )
    .ok()?;
    // File metadata is written by the upload service, outside generic CRUD's
    // cache-version updates; native previews also depend on current storage policy.
    // Read these durable records directly from storage, including legacy attachments.
    if domain::file_management::is_file_data_model_template(
        &runtime_model.metadata.template_provider,
        &runtime_model.metadata.template_code,
        &runtime_model.metadata.template_version,
    ) || runtime_model
        .metadata
        .fields
        .iter()
        .any(|field| field.field_kind == domain::ModelFieldKind::FilePreviewUrl)
    {
        return None;
    }
    (runtime_model.metadata.source_kind == domain::DataModelSourceKind::MainSource)
        .then_some(runtime_model.metadata)
}

pub(super) fn runtime_records_version_key(
    metadata: &runtime_core::model_metadata::ModelMetadata,
) -> String {
    format!("runtime-records:version:v1:{}", metadata.model_id)
}

pub(super) async fn runtime_records_cache_version(
    adapter: &RuntimeModelOperationAdapter,
    metadata: &runtime_core::model_metadata::ModelMetadata,
) -> String {
    let key = runtime_records_version_key(metadata);
    adapter
        .cache_store
        .get_json(&key)
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "0".to_string())
}

pub(super) async fn bump_runtime_records_cache_version(
    adapter: &RuntimeModelOperationAdapter,
    metadata: &runtime_core::model_metadata::ModelMetadata,
) {
    let key = runtime_records_version_key(metadata);
    let _ = adapter
        .cache_store
        .set_json(
            &key,
            serde_json::json!(uuid::Uuid::now_v7().to_string()),
            None,
        )
        .await;
}

pub(super) fn runtime_scope_grant_cache_value(
    grant: Option<&runtime_core::runtime_acl::RuntimeScopeGrant>,
) -> serde_json::Value {
    match grant {
        Some(grant) => serde_json::json!({
            "data_model_id": grant.data_model_id,
            "scope_kind": grant.scope_kind.as_str(),
            "scope_id": grant.scope_id,
            "enabled": grant.enabled,
            "permission_profile": grant.permission_profile.as_str(),
        }),
        None => serde_json::Value::Null,
    }
}

pub(super) fn runtime_model_cache_fingerprint(
    metadata: &runtime_core::model_metadata::ModelMetadata,
) -> serde_json::Value {
    let fields = metadata
        .fields
        .iter()
        .map(|field| {
            serde_json::json!({
                "id": field.id,
                "code": field.code,
                "physical_column_name": field.physical_column_name,
                "field_kind": field.field_kind.as_str(),
                "is_system": field.is_system,
                "is_writable": field.is_writable,
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "model_id": metadata.model_id,
        "model_code": metadata.model_code,
        "scope_kind": metadata.scope_kind.as_str(),
        "scope_id": metadata.scope_id,
        "source_kind": metadata.source_kind.as_str(),
        "physical_table_name": metadata.physical_table_name,
        "scope_column_name": metadata.scope_column_name,
        "fields": fields,
    })
}

pub(super) fn runtime_cache_digest(value: &serde_json::Value) -> String {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(value)
        .expect("runtime cache key payload should serialize")
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(super) fn runtime_records_list_cache_key(
    metadata: &runtime_core::model_metadata::ModelMetadata,
    credential: &RuntimeCredential,
    scope_grant: Option<&runtime_core::runtime_acl::RuntimeScopeGrant>,
    query: &RuntimeListQueryParams,
    version: &str,
) -> String {
    let payload = serde_json::json!({
        "model": runtime_model_cache_fingerprint(metadata),
        "credential": credential.cache_identity(),
        "scope_grant": runtime_scope_grant_cache_value(scope_grant),
        "query": {
            "filter": query.filter,
            "sort": query.sort,
            "expand": query.expand,
            "page": query.page.unwrap_or(1),
            "page_size": query.page_size.unwrap_or(20),
        },
        "version": version,
    });
    format!(
        "runtime-records:list:v1:{}:{}",
        metadata.model_id,
        runtime_cache_digest(&payload)
    )
}

pub(super) fn runtime_records_get_cache_key(
    metadata: &runtime_core::model_metadata::ModelMetadata,
    credential: &RuntimeCredential,
    scope_grant: Option<&runtime_core::runtime_acl::RuntimeScopeGrant>,
    record_id: &str,
    version: &str,
) -> String {
    let payload = serde_json::json!({
        "model": runtime_model_cache_fingerprint(metadata),
        "credential": credential.cache_identity(),
        "scope_grant": runtime_scope_grant_cache_value(scope_grant),
        "record_id": record_id,
        "version": version,
    });
    format!(
        "runtime-records:get:v1:{}:{}",
        metadata.model_id,
        runtime_cache_digest(&payload)
    )
}

pub(super) async fn cached_runtime_list_response(
    adapter: &RuntimeModelOperationAdapter,
    key: &str,
) -> Option<RuntimeListResponse> {
    adapter
        .cache_store
        .get_json(key)
        .await
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value(value).ok())
}

pub(super) async fn cache_runtime_list_response(
    adapter: &RuntimeModelOperationAdapter,
    key: &str,
    response: &RuntimeListResponse,
) {
    let Ok(value) = serde_json::to_value(response) else {
        return;
    };
    let _ = adapter
        .cache_store
        .set_json(key, value, Some(time::Duration::seconds(30)))
        .await;
}

pub(super) async fn cached_runtime_record(
    adapter: &RuntimeModelOperationAdapter,
    key: &str,
) -> Option<Value> {
    adapter.cache_store.get_json(key).await.ok().flatten()
}

pub(super) async fn cache_runtime_record(
    adapter: &RuntimeModelOperationAdapter,
    key: &str,
    record: &Value,
) {
    let _ = adapter
        .cache_store
        .set_json(key, record.clone(), Some(time::Duration::seconds(60)))
        .await;
}
