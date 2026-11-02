use uuid::Uuid;

use control_plane::_tests::support::{MemoryFileManagementRepository, memory_actor_context};
use control_plane::file_management::{
    BindFileTableStorageCommand, CreateFileStorageCommand, DeleteFileStorageCommand,
    DeleteFileTableCommand, FileStorageService, FileTableService, UpdateFileStorageCommand,
};

#[tokio::test]
async fn file_table_create_can_inherit_without_resolving_default_but_explicit_binding_requires_root()
 {
    let repository = MemoryFileManagementRepository::new(memory_actor_context(false, &[]));
    let service = FileTableService::new(repository);
    let error = service
        .create_table(control_plane::file_management::CreateFileTableCommand {
            actor_user_id: Uuid::now_v7(),
            code: "fixed_files".into(),
            title: "Fixed".into(),
            bound_storage_id: Some(Uuid::now_v7()),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("permission_denied"));
    let inherited = service
        .create_table(control_plane::file_management::CreateFileTableCommand {
            actor_user_id: Uuid::now_v7(),
            code: "inherited_files".into(),
            title: "Inherited".into(),
            bound_storage_id: None,
        })
        .await
        .unwrap();
    assert!(inherited.table.bound_storage_id.is_none());
    assert!(inherited.bound_storage_title.is_none());
}

#[tokio::test]
async fn explicit_storage_read_does_not_grant_storage_writes() {
    let actor = memory_actor_context(false, &["file_storages.list"]);
    let user_id = actor.user_id;
    let repository = MemoryFileManagementRepository::new(actor);
    let service = FileStorageService::new(repository);
    assert!(service.list_storages(user_id).await.is_ok());
    let error = service
        .delete_storage(DeleteFileStorageCommand {
            actor_user_id: user_id,
            file_storage_id: Uuid::now_v7(),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("permission_denied"));
    let service = FileStorageService::new(MemoryFileManagementRepository::new(
        memory_actor_context(false, &[]),
    ));
    assert!(service.list_storages(user_id).await.is_err());
}

#[tokio::test]
async fn only_root_can_create_file_storage() {
    let repository = MemoryFileManagementRepository::new(memory_actor_context(false, &[]));
    let service = FileStorageService::new(repository);

    let error = service
        .create_storage(CreateFileStorageCommand {
            actor_user_id: Uuid::now_v7(),
            code: "local-default".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: serde_json::json!({ "root_path": "api/storage" }),
            rule_json: serde_json::json!({}),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn only_root_can_rebind_file_table_storage() {
    let repository = MemoryFileManagementRepository::new(memory_actor_context(false, &[]));
    let service = FileTableService::new(repository);

    let error = service
        .bind_storage(BindFileTableStorageCommand {
            actor_user_id: Uuid::now_v7(),
            file_table_id: Uuid::now_v7(),
            bound_storage_id: Some(Uuid::now_v7()),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn only_root_can_update_file_storage() {
    let repository = MemoryFileManagementRepository::new(memory_actor_context(false, &[]));
    let service = FileStorageService::new(repository);

    let error = service
        .update_storage(UpdateFileStorageCommand {
            actor_user_id: Uuid::now_v7(),
            file_storage_id: Uuid::now_v7(),
            title: "Archive".into(),
            enabled: true,
            is_default: false,
            config_json: serde_json::json!({ "root_path": "api/storage" }),
            rule_json: serde_json::json!({}),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn only_root_can_delete_file_storage_and_table() {
    let repository = MemoryFileManagementRepository::new(memory_actor_context(false, &[]));
    let storage_service = FileStorageService::new(repository.clone());
    let table_service = FileTableService::new(repository);

    let storage_error = storage_service
        .delete_storage(DeleteFileStorageCommand {
            actor_user_id: Uuid::now_v7(),
            file_storage_id: Uuid::now_v7(),
        })
        .await
        .unwrap_err();
    assert!(storage_error.to_string().contains("permission_denied"));

    let table_error = table_service
        .delete_table(DeleteFileTableCommand {
            actor_user_id: Uuid::now_v7(),
            file_table_id: Uuid::now_v7(),
        })
        .await
        .unwrap_err();
    assert!(table_error.to_string().contains("permission_denied"));
}
