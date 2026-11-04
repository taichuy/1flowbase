use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use control_plane::_tests::support::MemoryFileManagementRepository;
use control_plane::{
    file_management::{
        FileAccessKey, FileAccessService, FileUploadService, FileUploadTarget, UploadFileCommand,
    },
    ports::{EphemeralKvStore, FileManagementRepository},
};
use domain::{
    DataModelScopeKind, FileStorageHealthStatus, FileStorageRecord, FileTableRecord,
    FileTableScopeKind, SYSTEM_SCOPE_ID,
};
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::file_management_upload_tests::{
    actor_in_workspace, model_definition, runtime_engine_for_model, scope_grant, temp_root,
};

#[derive(Default)]
struct TestKv {
    entries: Mutex<HashMap<String, Value>>,
    writes: Mutex<Vec<Duration>>,
}

#[async_trait]
impl EphemeralKvStore for TestKv {
    async fn set_json(&self, key: &str, value: Value, ttl: Option<Duration>) -> Result<()> {
        self.entries.lock().unwrap().insert(key.into(), value);
        self.writes.lock().unwrap().push(ttl.unwrap());
        Ok(())
    }
    async fn get_json(&self, key: &str) -> Result<Option<Value>> {
        Ok(self.entries.lock().unwrap().get(key).cloned())
    }
    async fn delete(&self, key: &str) -> Result<()> {
        self.entries.lock().unwrap().remove(key);
        Ok(())
    }
    async fn touch(&self, _: &str, _: Duration) -> Result<bool> {
        panic!("file access must not renew TTL")
    }
    async fn set_if_absent_json(&self, _: &str, _: Value, _: Option<Duration>) -> Result<bool> {
        anyhow::bail!("unused")
    }
    async fn increment_counter(&self, _: &str, _: i64, _: Option<Duration>) -> Result<i64> {
        anyhow::bail!("unused")
    }
}

#[tokio::test]
async fn file_access_preserves_stable_url_and_scopes_ephemeral_public_read() {
    let workspace_id = Uuid::now_v7();
    let mut actor = actor_in_workspace(workspace_id);
    actor
        .permissions
        .insert("files.access_links.create".to_owned());
    let repository = MemoryFileManagementRepository::new(actor.clone());
    let table_id = Uuid::now_v7();
    let storage_id = Uuid::now_v7();
    let mut model = model_definition(
        Uuid::now_v7(),
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
        "files",
    );
    model.template_code = domain::file_management::FILE_DATA_MODEL_TEMPLATE_CODE.into();
    repository.insert_scope_grant(scope_grant(model.id, workspace_id));
    repository.insert_model_definition(model.clone());
    let root = temp_root("temporary-file-access");
    repository
        .insert_file_storage(FileStorageRecord {
            id: storage_id,
            code: "local".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: json!({"root_path":root}),
            rule_json: json!({}),
            health_status: FileStorageHealthStatus::Ready,
            last_health_error: None,
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;
    repository
        .insert_file_table(FileTableRecord {
            id: table_id,
            code: model.code.clone(),
            title: "Files".into(),
            scope_kind: FileTableScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            model_definition_id: model.id,
            bound_storage_id: None,
            is_builtin: false,
            is_default: false,
            status: "active".into(),
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;
    let engine = runtime_engine_for_model(&model);
    let drivers = Arc::new(storage_object::builtin_driver_registry());
    let upload = FileUploadService::new(repository.clone(), drivers.clone(), engine.clone())
        .upload(UploadFileCommand {
            actor: actor.clone(),
            target: FileUploadTarget::Table(table_id),
            original_filename: format!("{}.md", "名".repeat(300)),
            content_type: Some("text/markdown".into()),
            bytes: b"file contents".to_vec(),
        })
        .await
        .unwrap();
    let id = Uuid::parse_str(upload.record["id"].as_str().unwrap()).unwrap();
    assert_eq!(
        upload.record["url"],
        domain::file_management::file_content_url(table_id, id)
    );
    assert!(upload.record["filename"].as_str().unwrap().len() > 255);
    repository.insert_file_record(table_id, upload.record.clone());
    let kv = Arc::new(TestKv::default());
    let service = FileAccessService::new(repository.clone(), drivers, engine, kv.clone());
    let mut reader = actor.clone();
    reader.permissions.remove("files.access_links.create");
    assert!(service.read(reader.clone(), table_id, id).await.is_ok());
    let denied = service
        .create_share_link(reader, table_id, id, 600)
        .await
        .err()
        .expect("read-only callers must not issue public shares");
    assert!(denied.to_string().contains("files.access_links.create"));
    assert!(kv.entries.lock().unwrap().is_empty());
    let link = service
        .create_share_link(actor.clone(), table_id, id, 600)
        .await
        .unwrap();
    assert!(link.url.starts_with("/api/public/files/content?token="));
    let token = link.url.split("token=").nth(1).unwrap();
    let mut content = service
        .read_temporary(FileAccessKey::from_token(token).unwrap())
        .await
        .unwrap();
    let mut bytes = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(content.reader.get_mut().unwrap(), &mut bytes)
        .await
        .unwrap();
    assert_eq!(bytes, b"file contents");
    assert_eq!(
        kv.writes.lock().unwrap().as_slice(),
        &[Duration::seconds(600)]
    );
    assert_eq!(
        service
            .read(actor.clone(), table_id, id)
            .await
            .unwrap()
            .filename,
        upload.record["filename"].as_str().unwrap()
    );

    let invalid = FileAccessKey::from_token(&"A".repeat(43)).unwrap();
    assert!(service
        .read_temporary(invalid)
        .await
        .err()
        .unwrap()
        .to_string()
        .contains("temporary_file_access_invalid"));
    assert!(FileAccessKey::from_token("malformed").is_err());
    assert!(service
        .create_share_link(actor.clone(), table_id, id, 0)
        .await
        .is_err());
    assert!(service
        .create_share_link(actor.clone(), table_id, id, i64::MAX)
        .await
        .is_err());
    let unauthorized = domain::ActorContext::scoped(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "reader",
        Vec::<String>::new(),
    );
    assert!(service
        .read(unauthorized.clone(), table_id, id)
        .await
        .is_err());
    assert!(service
        .create_share_link(unauthorized, table_id, id, 60)
        .await
        .is_err());
    assert_eq!(kv.entries.lock().unwrap().len(), 1);

    let mut changed = upload.record.clone();
    changed["path"] = json!("different-object");
    repository.insert_file_record(table_id, changed);
    assert!(service
        .read_temporary(FileAccessKey::from_token(token).unwrap())
        .await
        .is_err());
    repository.insert_file_record(table_id, upload.record);
    for grant in kv.entries.lock().unwrap().values_mut() {
        grant["expires_at"] = json!("1970-01-01T00:00:00Z");
    }
    assert!(service
        .read_temporary(FileAccessKey::from_token(token).unwrap())
        .await
        .is_err());
    assert!(kv.entries.lock().unwrap().is_empty());
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn image_preview_projects_upload_list_get_and_obeys_actual_storage_policy() {
    let workspace_id = Uuid::now_v7();
    let mut actor = actor_in_workspace(workspace_id);
    actor
        .permissions
        .insert("files.access_links.create".to_owned());
    let repository = MemoryFileManagementRepository::new(actor.clone());
    let table_id = Uuid::now_v7();
    let storage_id = Uuid::now_v7();
    let mut model = model_definition(
        Uuid::now_v7(),
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
        "files",
    );
    model.template_code = domain::file_management::FILE_DATA_MODEL_TEMPLATE_CODE.into();
    model.fields.push(domain::ModelFieldRecord {
        id: Uuid::now_v7(),
        data_model_id: model.id,
        code: "preview_url".into(),
        title: "Image Preview URL".into(),
        description: None,
        physical_column_name: String::new(),
        external_field_key: None,
        field_kind: domain::ModelFieldKind::FilePreviewUrl,
        is_system: true,
        is_writable: false,
        is_required: false,
        api_required: false,
        is_unique: false,
        default_value: None,
        display_interface: None,
        display_options: json!({}),
        relation_target_model_id: None,
        relation_options: json!({}),
        sort_order: 0,
        availability_status: domain::MetadataAvailabilityStatus::Available,
    });
    repository.insert_scope_grant(scope_grant(model.id, workspace_id));
    repository.insert_model_definition(model.clone());
    let root = temp_root("image-preview");
    repository
        .insert_file_storage(FileStorageRecord {
            id: storage_id,
            code: "local".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: json!({"root_path":root}),
            rule_json: json!({}),
            health_status: FileStorageHealthStatus::Ready,
            last_health_error: None,
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;
    repository
        .insert_file_table(FileTableRecord {
            id: table_id,
            code: model.code.clone(),
            title: "Files".into(),
            scope_kind: FileTableScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            model_definition_id: model.id,
            bound_storage_id: None,
            is_builtin: false,
            is_default: false,
            status: "active".into(),
            created_by: actor.user_id,
            updated_by: actor.user_id,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        })
        .await;

    let unconfigured = runtime_engine_for_model(&model);
    let engine = Arc::new(
        (*unconfigured)
            .clone()
            .with_computed_field_resolver(Arc::new(
                control_plane::file_management::FilePreviewProjector::new(repository.clone()),
            )),
    );
    let drivers = Arc::new(storage_object::builtin_driver_registry());
    let failed = FileUploadService::new(repository.clone(), drivers.clone(), unconfigured.clone())
        .upload(UploadFileCommand {
            actor: actor.clone(),
            target: FileUploadTarget::Table(table_id),
            original_filename: "must-not-commit.png".into(),
            content_type: Some("image/png".into()),
            bytes: vec![137, 80, 78, 71],
        })
        .await;
    assert!(failed
        .err()
        .unwrap()
        .to_string()
        .contains("computed field resolver"));
    let uploaded = FileUploadService::new(repository.clone(), drivers.clone(), engine.clone())
        .upload(UploadFileCommand {
            actor: actor.clone(),
            target: FileUploadTarget::Table(table_id),
            original_filename: "picture.png".into(),
            content_type: Some("image/png".into()),
            bytes: vec![137, 80, 78, 71],
        })
        .await
        .unwrap();
    let id = Uuid::parse_str(uploaded.record["id"].as_str().unwrap()).unwrap();
    let expected = domain::file_management::file_preview_url(table_id, id);
    assert_eq!(uploaded.record["preview_url"], expected);
    let listed = engine
        .list_records(runtime_core::runtime_engine::RuntimeListInput {
            actor: actor.clone(),
            model_code: model.code.clone(),
            scope_grant: Some(runtime_core::runtime_acl::RuntimeScopeGrant {
                data_model_id: model.id,
                scope_kind: DataModelScopeKind::Workspace,
                scope_id: workspace_id,
                enabled: true,
                permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
            }),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
        })
        .await
        .unwrap();
    assert_eq!(
        listed.items.len(),
        1,
        "projection failure must precede durable record commit"
    );
    assert_eq!(listed.items[0]["preview_url"], expected);
    for (filter, sorts) in [
        (
            domain::ResourceFilterExpr::All(vec![]),
            vec![runtime_core::runtime_engine::RuntimeSortInput {
                field_code: "preview_url".into(),
                direction: "asc".into(),
            }],
        ),
        (
            domain::ResourceFilterExpr::Field {
                field: "preview_url".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: Value::Null,
            },
            vec![],
        ),
    ] {
        let result = engine
            .list_records(runtime_core::runtime_engine::RuntimeListInput {
                actor: actor.clone(),
                model_code: model.code.clone(),
                scope_grant: Some(runtime_core::runtime_acl::RuntimeScopeGrant {
                    data_model_id: model.id,
                    scope_kind: DataModelScopeKind::Workspace,
                    scope_id: workspace_id,
                    enabled: true,
                    permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
                }),
                filter,
                sorts,
                expand_relations: vec![],
                page: 1,
                page_size: 20,
            })
            .await;
        assert!(matches!(
            result
                .err()
                .unwrap()
                .downcast_ref::<runtime_core::runtime_engine::RuntimeModelError>(),
            Some(runtime_core::runtime_engine::RuntimeModelError::InvalidOperationField(_))
        ));
    }
    let get = runtime_core::runtime_engine::RuntimeGetInput {
        actor: actor.clone(),
        model_code: model.code.clone(),
        record_id: id.to_string(),
        scope_grant: Some(runtime_core::runtime_acl::RuntimeScopeGrant {
            data_model_id: model.id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: workspace_id,
            enabled: true,
            permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
        }),
    };
    assert_eq!(
        engine.get_record(get.clone()).await.unwrap().unwrap()["preview_url"],
        expected
    );
    repository.insert_file_record(table_id, uploaded.record.clone());
    let kv = Arc::new(TestKv::default());
    let service = FileAccessService::new(
        repository.clone(),
        drivers.clone(),
        engine.clone(),
        kv.clone(),
    );
    assert!(
        service
            .read_with_preview(actor.clone(), table_id, id, true)
            .await
            .unwrap()
            .preview
    );
    assert!(
        !service
            .read(actor.clone(), table_id, id)
            .await
            .unwrap()
            .preview
    );
    let link = service
        .create_share_link(actor.clone(), table_id, id, 600)
        .await
        .unwrap();
    assert!(link
        .preview_url
        .as_ref()
        .unwrap()
        .ends_with("&preview=true"));
    let token = link.url.split("token=").nth(1).unwrap();
    assert!(
        service
            .read_temporary_with_preview(FileAccessKey::from_token(token).unwrap(), true)
            .await
            .unwrap()
            .preview
    );
    let mut storage = repository
        .get_file_storage(storage_id)
        .await
        .unwrap()
        .unwrap();
    storage.rule_json = json!({"image_preview_enabled":false});
    repository.insert_file_storage(storage.clone()).await;
    assert_eq!(
        engine.get_record(get).await.unwrap().unwrap()["preview_url"],
        Value::Null
    );
    assert!(service
        .read_with_preview(actor.clone(), table_id, id, true)
        .await
        .is_err());
    assert!(service
        .read_temporary_with_preview(FileAccessKey::from_token(token).unwrap(), true)
        .await
        .is_err());
    assert!(
        !service
            .read_temporary(FileAccessKey::from_token(token).unwrap())
            .await
            .unwrap()
            .preview
    );
    storage.rule_json = json!({"image_preview_enabled":true});
    repository.insert_file_storage(storage).await;
    for (name, mime) in [
        ("active.svg", "image/svg+xml"),
        ("active.html", "text/html"),
        ("text.md", "text/markdown"),
    ] {
        let other = FileUploadService::new(repository.clone(), drivers.clone(), engine.clone())
            .upload(UploadFileCommand {
                actor: actor.clone(),
                target: FileUploadTarget::Table(table_id),
                original_filename: name.into(),
                content_type: Some(mime.into()),
                bytes: b"<script>active</script>".to_vec(),
            })
            .await
            .unwrap();
        assert_eq!(other.record["preview_url"], Value::Null);
        let other_id = Uuid::parse_str(other.record["id"].as_str().unwrap()).unwrap();
        assert!(service
            .read_with_preview(actor.clone(), table_id, other_id, true)
            .await
            .is_err());
        assert!(
            !service
                .read(actor.clone(), table_id, other_id)
                .await
                .unwrap()
                .preview
        );
    }
    for grant in kv.entries.lock().unwrap().values_mut() {
        grant["expires_at"] = json!("1970-01-01T00:00:00Z");
    }
    assert!(service
        .read_temporary_with_preview(FileAccessKey::from_token(token).unwrap(), true)
        .await
        .is_err());
    tokio::fs::remove_dir_all(root).await.unwrap();
}
