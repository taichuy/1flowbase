use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use control_plane::_tests::support::MemoryFileManagementRepository;
use control_plane::file_management::{FileUploadService, UploadFileCommand};
use control_plane::ports::{FileManagementRepository, UpdateFileStorageBindingInput};
use domain::{
    ActorContext, DataModelScopeKind, FileStorageHealthStatus, FileTableScopeKind,
    MetadataAvailabilityStatus, ModelDefinitionRecord, ScopeDataModelGrantRecord,
    ScopeDataModelPermissionProfile, SYSTEM_SCOPE_ID,
};
use runtime_core::{
    model_metadata::ModelMetadata,
    resource_descriptor::ResourceDescriptor,
    runtime_engine::RuntimeEngine,
    runtime_model_registry::RuntimeModelRegistry,
    runtime_record_repository::{RuntimeListQuery, RuntimeListResult, RuntimeRecordRepository},
};
use serde_json::Value;
use std::{collections::HashMap, sync::Mutex};
use storage_object::OpenReadInput;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Default)]
struct TestRuntimeRecordRepository {
    records: Mutex<HashMap<String, Vec<Value>>>,
}

#[async_trait]
impl RuntimeRecordRepository for TestRuntimeRecordRepository {
    async fn list_records(
        &self,
        _metadata: &ModelMetadata,
        _query: RuntimeListQuery,
    ) -> Result<RuntimeListResult> {
        Ok(RuntimeListResult {
            items: vec![],
            total: 0,
        })
    }

    async fn get_record(
        &self,
        metadata: &ModelMetadata,
        _scope_id: Option<Uuid>,
        _owner_user_id: Option<Uuid>,
        record_id: &str,
    ) -> Result<Option<Value>> {
        Ok(self
            .records
            .lock()
            .expect("runtime record lock poisoned")
            .get(&metadata.model_code)
            .and_then(|records| {
                records
                    .iter()
                    .find(|record| record["id"].as_str() == Some(record_id))
                    .cloned()
            }))
    }

    async fn create_record(
        &self,
        metadata: &ModelMetadata,
        actor_user_id: Uuid,
        _scope_id: Uuid,
        payload: Value,
    ) -> Result<Value> {
        let mut record = payload.as_object().cloned().unwrap_or_default();
        record.insert("id".into(), serde_json::json!(Uuid::now_v7().to_string()));
        record.insert(
            "created_by".into(),
            serde_json::json!(actor_user_id.to_string()),
        );
        let value = Value::Object(record);
        self.records
            .lock()
            .expect("runtime record lock poisoned")
            .entry(metadata.model_code.clone())
            .or_default()
            .push(value.clone());
        Ok(value)
    }

    async fn update_record(
        &self,
        _metadata: &ModelMetadata,
        _actor_user_id: Uuid,
        _scope_id: Option<Uuid>,
        _owner_user_id: Option<Uuid>,
        _record_id: &str,
        _payload: Value,
    ) -> Result<Value> {
        anyhow::bail!("not implemented for upload tests")
    }

    async fn delete_record(
        &self,
        _metadata: &ModelMetadata,
        _scope_id: Option<Uuid>,
        _owner_user_id: Option<Uuid>,
        _record_id: &str,
    ) -> Result<bool> {
        Ok(false)
    }
}

fn temp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{name}-{}", Uuid::now_v7()))
}

fn model_definition(
    model_id: Uuid,
    scope_kind: DataModelScopeKind,
    scope_id: Uuid,
    code: &str,
) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        id: model_id,
        scope_kind,
        scope_id,
        code: code.to_string(),
        title: code.to_string(),
        physical_table_name: format!("rtm_{}_{}", scope_kind.as_str(), code),
        acl_namespace: format!("state_model.{code}"),
        audit_namespace: format!("audit.state_model.{code}"),
        fields: vec![],
        availability_status: MetadataAvailabilityStatus::Available,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        status: domain::DataModelStatus::Published,
        api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
        protection: domain::DataModelProtection::default(),
    }
}

fn runtime_engine_for_model(model: &ModelDefinitionRecord) -> Arc<RuntimeEngine> {
    let registry = RuntimeModelRegistry::default();
    registry.rebuild(vec![ModelMetadata {
        model_id: model.id,
        model_code: model.code.clone(),
        status: model.status,
        scope_kind: model.scope_kind,
        scope_id: model.scope_id,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: model.physical_table_name.clone(),
        scope_column_name: "scope_id".into(),
        fields: model.fields.clone(),
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(&model.code, model.scope_kind),
    }]);

    Arc::new(RuntimeEngine::new(
        registry,
        Arc::new(TestRuntimeRecordRepository::default()),
    ))
}

fn actor_in_workspace(workspace_id: Uuid) -> ActorContext {
    ActorContext::scoped(
        Uuid::now_v7(),
        workspace_id,
        "admin",
        [
            "state_data.create.all".to_string(),
            "state_data.view.all".to_string(),
        ],
    )
}

fn scope_grant(model_id: Uuid, scope_id: Uuid) -> ScopeDataModelGrantRecord {
    ScopeDataModelGrantRecord {
        id: Uuid::now_v7(),
        scope_kind: DataModelScopeKind::Workspace,
        scope_id,
        data_model_id: model_id,
        enabled: true,
        permission_profile: ScopeDataModelPermissionProfile::ScopeAll,
        created_by: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    }
}

#[tokio::test]
async fn upload_service_writes_object_and_runtime_record_with_storage_snapshot() {
    let workspace_id = Uuid::now_v7();
    let actor = actor_in_workspace(workspace_id);
    let repository = MemoryFileManagementRepository::new(actor.clone());
    let storage_id = Uuid::now_v7();
    let file_table_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let model = model_definition(
        model_id,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
        "assets",
    );
    let root = temp_root("file-upload-service");

    repository
        .insert_file_storage(domain::FileStorageRecord {
            id: storage_id,
            code: "local_default".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: serde_json::json!({
                "root_path": root.display().to_string(),
                "public_base_url": null,
            }),
            rule_json: serde_json::json!({}),
            health_status: FileStorageHealthStatus::Ready,
            last_health_error: None,
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;
    repository.insert_model_definition(model.clone());
    repository.insert_scope_grant(scope_grant(model.id, workspace_id));
    repository
        .insert_file_table(domain::FileTableRecord {
            id: file_table_id,
            code: "assets".into(),
            title: "Assets".into(),
            scope_kind: FileTableScopeKind::Workspace,
            scope_id: workspace_id,
            model_definition_id: model.id,
            bound_storage_id: storage_id,
            is_builtin: false,
            is_default: false,
            status: "active".into(),
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;

    let registry = Arc::new(storage_object::builtin_driver_registry());
    let service = FileUploadService::new(
        repository.clone(),
        registry.clone(),
        runtime_engine_for_model(&model),
    );

    let uploaded = service
        .upload(UploadFileCommand {
            actor: actor.clone(),
            file_table_id,
            original_filename: "demo.txt".into(),
            content_type: Some("text/plain".into()),
            bytes: b"hello storage".to_vec(),
        })
        .await
        .unwrap();

    assert_eq!(uploaded.storage_id, storage_id);
    let expected_storage_id = storage_id.to_string();
    assert_eq!(
        uploaded.record["storage_id"].as_str(),
        Some(expected_storage_id.as_str())
    );
    assert_eq!(uploaded.record["filename"].as_str(), Some("demo.txt"));
    assert_eq!(uploaded.record["mimetype"].as_str(), Some("text/plain"));

    let stored_path = uploaded.record["path"].as_str().unwrap().to_string();
    let stored = registry
        .get("local")
        .unwrap()
        .open_read(OpenReadInput {
            config_json: &serde_json::json!({
                "root_path": root.display().to_string(),
                "public_base_url": null,
            }),
            object_path: &stored_path,
        })
        .await
        .unwrap();
    assert_eq!(stored.bytes, b"hello storage");

    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn upload_service_uses_current_table_binding_for_each_new_upload() {
    let workspace_id = Uuid::now_v7();
    let actor = actor_in_workspace(workspace_id);
    let repository = MemoryFileManagementRepository::new(actor.clone());
    let first_storage_id = Uuid::now_v7();
    let second_storage_id = Uuid::now_v7();
    let file_table_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let model = model_definition(
        model_id,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
        "assets",
    );
    let first_root = temp_root("file-upload-first");
    let second_root = temp_root("file-upload-second");

    for (storage_id, code, root) in [
        (first_storage_id, "local_first", first_root.clone()),
        (second_storage_id, "local_second", second_root.clone()),
    ] {
        repository
            .insert_file_storage(domain::FileStorageRecord {
                id: storage_id,
                code: code.into(),
                title: code.into(),
                driver_type: "local".into(),
                enabled: true,
                is_default: storage_id == first_storage_id,
                config_json: serde_json::json!({
                    "root_path": root.display().to_string(),
                    "public_base_url": null,
                }),
                rule_json: serde_json::json!({}),
                health_status: FileStorageHealthStatus::Ready,
                last_health_error: None,
                created_by: actor.user_id,
                updated_by: actor.user_id,
                created_at: OffsetDateTime::now_utc(),
                updated_at: OffsetDateTime::now_utc(),
            })
            .await;
    }
    repository.insert_model_definition(model.clone());
    repository.insert_scope_grant(scope_grant(model.id, workspace_id));
    repository
        .insert_file_table(domain::FileTableRecord {
            id: file_table_id,
            code: "assets".into(),
            title: "Assets".into(),
            scope_kind: FileTableScopeKind::Workspace,
            scope_id: workspace_id,
            model_definition_id: model.id,
            bound_storage_id: first_storage_id,
            is_builtin: false,
            is_default: false,
            status: "active".into(),
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;

    let service = FileUploadService::new(
        repository.clone(),
        Arc::new(storage_object::builtin_driver_registry()),
        runtime_engine_for_model(&model),
    );

    let first = service
        .upload(UploadFileCommand {
            actor: actor.clone(),
            file_table_id,
            original_filename: "first.txt".into(),
            content_type: Some("text/plain".into()),
            bytes: b"first".to_vec(),
        })
        .await
        .unwrap();
    assert_eq!(first.storage_id, first_storage_id);

    FileManagementRepository::update_file_table_binding(
        &repository,
        &UpdateFileStorageBindingInput {
            actor_user_id: actor.user_id,
            file_table_id,
            bound_storage_id: second_storage_id,
        },
    )
    .await
    .unwrap();

    let second = service
        .upload(UploadFileCommand {
            actor,
            file_table_id,
            original_filename: "second.txt".into(),
            content_type: Some("text/plain".into()),
            bytes: b"second".to_vec(),
        })
        .await
        .unwrap();
    assert_eq!(second.storage_id, second_storage_id);

    let _ = std::fs::remove_dir_all(first_root);
    let _ = std::fs::remove_dir_all(second_root);
}

#[tokio::test]
async fn upload_service_requires_persisted_scope_grant_before_writing_runtime_record() {
    let workspace_id = Uuid::now_v7();
    let actor = actor_in_workspace(workspace_id);
    let repository = MemoryFileManagementRepository::new(actor.clone());
    let storage_id = Uuid::now_v7();
    let file_table_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let model = model_definition(
        model_id,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
        "assets",
    );
    let root = temp_root("file-upload-no-grant");

    repository
        .insert_file_storage(domain::FileStorageRecord {
            id: storage_id,
            code: "local_default".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: serde_json::json!({
                "root_path": root.display().to_string(),
                "public_base_url": null,
            }),
            rule_json: serde_json::json!({}),
            health_status: FileStorageHealthStatus::Ready,
            last_health_error: None,
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;
    repository.insert_model_definition(model.clone());
    repository
        .insert_file_table(domain::FileTableRecord {
            id: file_table_id,
            code: "assets".into(),
            title: "Assets".into(),
            scope_kind: FileTableScopeKind::Workspace,
            scope_id: workspace_id,
            model_definition_id: model.id,
            bound_storage_id: storage_id,
            is_builtin: false,
            is_default: false,
            status: "active".into(),
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;

    let service = FileUploadService::new(
        repository,
        Arc::new(storage_object::builtin_driver_registry()),
        runtime_engine_for_model(&model),
    );

    let result = service
        .upload(UploadFileCommand {
            actor,
            file_table_id,
            original_filename: "blocked.txt".into(),
            content_type: Some("text/plain".into()),
            bytes: b"blocked".to_vec(),
        })
        .await;
    let error = match result {
        Ok(_) => panic!("upload without persisted grant should fail"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("data_model_scope_not_granted"));
    assert!(!root.exists());
}
