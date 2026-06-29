use runtime_core::runtime_model_registry::{RuntimeDataModelAvailability, RuntimeModelRegistry};

#[test]
fn runtime_model_registry_rebuilds_and_refreshes_by_model_code() {
    let registry = RuntimeModelRegistry::default();
    registry.rebuild(vec![model_metadata_with_status(
        "orders",
        domain::DataModelStatus::Published,
    )]);

    assert!(registry
        .get(
            domain::DataModelScopeKind::Workspace,
            uuid::Uuid::nil(),
            "orders"
        )
        .is_some());
}

#[test]
fn draft_model_rebuilds_as_visible_but_not_published() {
    let registry = RuntimeModelRegistry::default();
    registry.rebuild(vec![model_metadata_with_status(
        "orders",
        domain::DataModelStatus::Draft,
    )]);

    let registered = registry
        .get_runtime_model(
            domain::DataModelScopeKind::Workspace,
            uuid::Uuid::nil(),
            "orders",
        )
        .unwrap();

    assert_eq!(
        registered.availability,
        RuntimeDataModelAvailability::NotPublished
    );
    assert!(registry
        .get(
            domain::DataModelScopeKind::Workspace,
            uuid::Uuid::nil(),
            "orders"
        )
        .is_some());
}

#[test]
fn published_model_registers_as_runtime_available() {
    let registry = RuntimeModelRegistry::default();
    registry.rebuild_with_status(vec![(
        model_metadata_with_status("orders", domain::DataModelStatus::Published),
        domain::DataModelStatus::Published,
    )]);

    let registered = registry
        .get_runtime_model(
            domain::DataModelScopeKind::Workspace,
            uuid::Uuid::nil(),
            "orders",
        )
        .unwrap();

    assert_eq!(
        registered.availability,
        RuntimeDataModelAvailability::Available
    );
}

fn model_metadata_with_status(
    model_code: &str,
    status: domain::DataModelStatus,
) -> runtime_core::model_metadata::ModelMetadata {
    runtime_core::model_metadata::ModelMetadata {
        model_id: uuid::Uuid::nil(),
        model_code: model_code.into(),
        status,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: uuid::Uuid::nil(),
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: format!("rtm_workspace_demo_{model_code}"),
        scope_column_name: "scope_id".into(),
        fields: vec![],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: runtime_core::resource_descriptor::ResourceDescriptor::runtime_model(
            model_code,
            domain::DataModelScopeKind::Workspace,
        ),
    }
}
