use domain::{
    ApiExposureStatus, DataModelOwnerKind, DataModelScopeKind, DataModelSourceKind,
    DataModelStatus, MetadataAvailabilityStatus, ModelFieldKind,
};
use serde_json::json;
use storage_postgres::mappers::model_definition_mapper::{
    PgModelDefinitionMapper, StoredModelDefinitionRow,
};
use storage_postgres::mappers::model_field_mapper::{PgModelFieldMapper, StoredModelFieldRow};
use uuid::Uuid;

#[test]
fn model_field_mapper_preserves_runtime_and_external_field_flags() {
    let field_id = Uuid::now_v7();
    let data_model_id = Uuid::now_v7();
    let relation_target_model_id = Uuid::now_v7();

    let record = PgModelFieldMapper::to_model_field_record(StoredModelFieldRow {
        id: field_id,
        data_model_id,
        code: "owner_id".into(),
        title: "Owner".into(),
        description: Some("Owner relation".into()),
        physical_column_name: "owner_id".into(),
        external_field_key: Some("owner.id".into()),
        field_kind: "many_to_one".into(),
        is_system: true,
        is_writable: false,
        is_required: true,
        api_required: true,
        is_unique: false,
        default_value: Some(json!("root")),
        display_interface: Some("user_picker".into()),
        display_options: json!({ "mode": "compact" }),
        relation_target_model_id: Some(relation_target_model_id),
        relation_options: json!({ "on_delete": "restrict" }),
        sort_order: 10,
        availability_status: "broken".into(),
    });

    assert_eq!(record.id, field_id);
    assert_eq!(record.data_model_id, data_model_id);
    assert_eq!(record.description.as_deref(), Some("Owner relation"));
    assert_eq!(record.external_field_key.as_deref(), Some("owner.id"));
    assert!(matches!(record.field_kind, ModelFieldKind::ManyToOne));
    assert!(record.is_system);
    assert!(!record.is_writable);
    assert!(record.is_required);
    assert!(!record.is_unique);
    assert_eq!(record.default_value, Some(json!("root")));
    assert_eq!(record.display_interface.as_deref(), Some("user_picker"));
    assert_eq!(record.display_options, json!({ "mode": "compact" }));
    assert_eq!(
        record.relation_target_model_id,
        Some(relation_target_model_id)
    );
    assert_eq!(record.relation_options, json!({ "on_delete": "restrict" }));
    assert_eq!(record.sort_order, 10);
    assert!(matches!(
        record.availability_status,
        MetadataAvailabilityStatus::Broken
    ));
}

#[test]
fn model_definition_mapper_preserves_scope_source_exposure_and_protection() {
    let model_id = Uuid::now_v7();
    let scope_id = Uuid::now_v7();
    let data_source_instance_id = Uuid::now_v7();
    let field_id = Uuid::now_v7();

    let field = PgModelFieldMapper::to_model_field_record(StoredModelFieldRow {
        id: field_id,
        data_model_id: model_id,
        code: "email".into(),
        title: "Email".into(),
        description: None,
        physical_column_name: "email".into(),
        external_field_key: None,
        field_kind: "string".into(),
        is_system: false,
        is_writable: true,
        is_required: true,
        api_required: true,
        is_unique: true,
        default_value: None,
        display_interface: None,
        display_options: json!({}),
        relation_target_model_id: None,
        relation_options: json!({}),
        sort_order: 1,
        availability_status: "available".into(),
    });

    let record = PgModelDefinitionMapper::to_model_definition_record(StoredModelDefinitionRow {
        id: model_id,
        scope_kind: "system".into(),
        scope_id,
        data_source_instance_id: Some(data_source_instance_id),
        source_kind: "external_source".into(),
        external_resource_key: Some("crm.contacts".into()),
        external_table_id: Some("contacts".into()),
        external_capability_snapshot: Some(json!({ "driver": "http" })),
        code: "contacts".into(),
        title: "Contacts".into(),
        physical_table_name: "dm_contacts".into(),
        acl_namespace: "data_model.contacts".into(),
        audit_namespace: "data_model.contacts".into(),
        availability_status: "unavailable".into(),
        status: "broken".into(),
        api_exposure_status: "unsafe_external_source".into(),
        owner_kind: "runtime_extension".into(),
        owner_id: Some("crm-provider".into()),
        is_protected: true,
        fields: vec![field],
    });

    assert_eq!(record.id, model_id);
    assert!(matches!(record.scope_kind, DataModelScopeKind::System));
    assert_eq!(record.scope_id, scope_id);
    assert_eq!(
        record.data_source_instance_id,
        Some(data_source_instance_id)
    );
    assert!(matches!(
        record.source_kind,
        DataModelSourceKind::ExternalSource
    ));
    assert_eq!(
        record.external_resource_key.as_deref(),
        Some("crm.contacts")
    );
    assert_eq!(record.external_table_id.as_deref(), Some("contacts"));
    assert_eq!(
        record.external_capability_snapshot,
        Some(json!({ "driver": "http" }))
    );
    assert_eq!(record.code, "contacts");
    assert_eq!(record.physical_table_name, "dm_contacts");
    assert!(matches!(
        record.availability_status,
        MetadataAvailabilityStatus::Unavailable
    ));
    assert!(matches!(record.status, DataModelStatus::Broken));
    assert!(matches!(
        record.api_exposure_status,
        ApiExposureStatus::UnsafeExternalSource
    ));
    assert!(matches!(
        record.protection.owner_kind,
        DataModelOwnerKind::RuntimeExtension
    ));
    assert_eq!(record.protection.owner_id.as_deref(), Some("crm-provider"));
    assert!(record.protection.is_protected);
    assert_eq!(record.fields.len(), 1);
    assert_eq!(record.fields[0].code, "email");
}
