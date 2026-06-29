use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    model_definition::{
        AddModelFieldCommand, BatchDeleteModelDefinitionsCommand, CreateModelDefinitionCommand,
        DeleteModelDefinitionCommand, InMemoryModelDefinitionRepository,
    },
    ports::{CreateModelDefinitionInput, ModelDefinitionRepository, RuntimeRegistrySync},
    runtime_registry_sync::ModelDefinitionMutationService,
};
use domain::{DataModelScopeKind, ModelFieldKind};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Default)]
struct CountingRuntimeRegistrySync {
    rebuild_calls: Arc<AtomicUsize>,
}

impl CountingRuntimeRegistrySync {
    fn rebuild_calls(&self) -> usize {
        self.rebuild_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl RuntimeRegistrySync for CountingRuntimeRegistrySync {
    async fn rebuild(&self) -> Result<()> {
        self.rebuild_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn create_model_rebuilds_runtime_registry_once() {
    let repository = InMemoryModelDefinitionRepository::default();
    let sync = CountingRuntimeRegistrySync::default();
    let service = ModelDefinitionMutationService::new(repository, sync.clone());

    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "orders".into(),
            title: "Orders".into(),
            status: None,
        })
        .await
        .unwrap();

    assert_eq!(created.code, "orders");
    assert_eq!(sync.rebuild_calls(), 1);
}

#[tokio::test]
async fn add_field_rebuilds_runtime_registry_once() {
    let repository = InMemoryModelDefinitionRepository::default();
    let seeded = repository
        .create_model_definition(&CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: Uuid::nil(),
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "orders".into(),
            title: "Orders".into(),
        })
        .await
        .unwrap();
    let sync = CountingRuntimeRegistrySync::default();
    let service = ModelDefinitionMutationService::new(repository, sync.clone());

    let field = service
        .add_field(AddModelFieldCommand {
            actor_user_id: Uuid::nil(),
            model_id: seeded.id,
            code: "status".into(),
            title: "Status".into(),
            description: None,
            external_field_key: None,
            field_kind: ModelFieldKind::Enum,
            is_required: true,
            is_unique: false,
            default_value: Some(json!("draft")),
            display_interface: Some("select".into()),
            display_options: json!({ "options": ["draft", "paid"] }),
            relation_target_model_id: None,
            relation_options: json!({}),
        })
        .await
        .unwrap();

    assert_eq!(field.code, "status");
    assert_eq!(sync.rebuild_calls(), 1);
}

#[tokio::test]
async fn delete_model_rebuilds_runtime_registry_once() {
    let repository = InMemoryModelDefinitionRepository::default();
    let seeded = repository
        .create_model_definition(&CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: Uuid::nil(),
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "orders".into(),
            title: "Orders".into(),
        })
        .await
        .unwrap();
    let sync = CountingRuntimeRegistrySync::default();
    let service = ModelDefinitionMutationService::new(repository, sync.clone());

    service
        .delete_model(DeleteModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            model_id: seeded.id,
            confirmed: true,
        })
        .await
        .unwrap();

    assert_eq!(sync.rebuild_calls(), 1);
}

#[tokio::test]
async fn batch_delete_models_rebuilds_runtime_registry_once() {
    let repository = InMemoryModelDefinitionRepository::default();
    let first = repository
        .create_model_definition(&CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: Uuid::nil(),
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "orders".into(),
            title: "Orders".into(),
        })
        .await
        .unwrap();
    let second = repository
        .create_model_definition(&CreateModelDefinitionInput {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: Uuid::nil(),
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: domain::DataModelProtection::default(),
            code: "invoices".into(),
            title: "Invoices".into(),
        })
        .await
        .unwrap();
    let sync = CountingRuntimeRegistrySync::default();
    let service = ModelDefinitionMutationService::new(repository, sync.clone());

    let deleted_ids = service
        .batch_delete_models(BatchDeleteModelDefinitionsCommand {
            actor_user_id: Uuid::nil(),
            model_ids: vec![first.id, second.id],
            confirmed: true,
        })
        .await
        .unwrap();

    assert_eq!(deleted_ids, vec![first.id, second.id]);
    assert_eq!(sync.rebuild_calls(), 1);
}
