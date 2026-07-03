use domain::ActorContext;
use plugin_framework::data_source_contract::{
    DataSourceCreateRecordInput, DataSourceCreateRecordOutput, DataSourceDeleteRecordInput,
    DataSourceDeleteRecordOutput, DataSourceGetRecordInput, DataSourceGetRecordOutput,
    DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourceUpdateRecordInput,
    DataSourceUpdateRecordOutput,
};
use runtime_core::runtime_acl::RuntimeScopeGrant;
use runtime_core::runtime_engine::{
    DataSourceRuntimeRecordBackend, RuntimeCreateInput, RuntimeDeleteInput, RuntimeEngine,
    RuntimeGetInput, RuntimeListInput, RuntimeModelError, RuntimeSortInput, RuntimeUpdateInput,
};
use runtime_core::{model_metadata::ModelMetadata, resource_descriptor::ResourceDescriptor};
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn scope_grant(model_id: Uuid, scope_id: Uuid) -> RuntimeScopeGrant {
    RuntimeScopeGrant {
        data_model_id: model_id,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id,
        enabled: true,
        permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
    }
}

#[tokio::test]
async fn runtime_engine_runs_full_crud_against_repository_and_scope_context() {
    let engine = RuntimeEngine::for_tests();
    let root = ActorContext::root(Uuid::nil(), Uuid::nil(), "root");
    let grant = scope_grant(Uuid::nil(), Uuid::nil());
    let first = engine
        .create_record(RuntimeCreateInput {
            actor: root.clone(),
            model_code: "orders".into(),
            payload: json!({ "title": "A-001", "status": "draft" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();

    let created = engine
        .create_record(RuntimeCreateInput {
            actor: root.clone(),
            model_code: "orders".into(),
            payload: json!({ "title": "A-002", "status": "paid" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();

    let first_record_id = first["id"].as_str().unwrap().to_string();
    let record_id = created["id"].as_str().unwrap().to_string();

    let listed = engine
        .list_records(RuntimeListInput {
            actor: root.clone(),
            model_code: "orders".into(),
            filter: domain::ResourceFilterExpr::Field {
                field: "status".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("paid"),
            },
            sorts: vec![RuntimeSortInput {
                field_code: "title".into(),
                direction: "desc".into(),
            }],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0]["title"], json!("A-002"));

    let fetched = engine
        .get_record(RuntimeGetInput {
            actor: root.clone(),
            model_code: "orders".into(),
            record_id: first_record_id,
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched["title"], json!("A-001"));

    let updated = engine
        .update_record(RuntimeUpdateInput {
            actor: root.clone(),
            model_code: "orders".into(),
            record_id: record_id.clone(),
            payload: json!({ "title": "A-002" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(updated["title"], json!("A-002"));

    let deleted = engine
        .delete_record(RuntimeDeleteInput {
            actor: root,
            model_code: "orders".into(),
            record_id,
            scope_grant: Some(grant),
        })
        .await
        .unwrap();
    assert_eq!(deleted["deleted"], json!(true));
}

#[tokio::test]
async fn runtime_engine_rejects_record_writes_when_model_capability_is_read_only() {
    let mut metadata = status_model_metadata("application_run_log_summaries");
    metadata.record_capabilities = domain::DataModelRecordCapabilities::read_only();
    let model_id = metadata.model_id;
    let engine = RuntimeEngine::for_tests_with_models(vec![metadata]);
    let actor = ActorContext::root(Uuid::nil(), Uuid::nil(), "root");
    let grant = scope_grant(model_id, Uuid::nil());

    for error in [
        engine
            .create_record(RuntimeCreateInput {
                actor: actor.clone(),
                model_code: "application_run_log_summaries".into(),
                payload: json!({ "title": "blocked" }),
                scope_grant: Some(grant.clone()),
            })
            .await
            .unwrap_err(),
        engine
            .update_record(RuntimeUpdateInput {
                actor: actor.clone(),
                model_code: "application_run_log_summaries".into(),
                record_id: "record-1".into(),
                payload: json!({ "title": "blocked" }),
                scope_grant: Some(grant.clone()),
            })
            .await
            .unwrap_err(),
        engine
            .delete_record(RuntimeDeleteInput {
                actor,
                model_code: "application_run_log_summaries".into(),
                record_id: "record-1".into(),
                scope_grant: Some(grant),
            })
            .await
            .unwrap_err(),
    ] {
        assert!(matches!(
            error.downcast_ref::<RuntimeModelError>(),
            Some(RuntimeModelError::RecordActionNotAllowed { .. })
        ));
    }
}

#[tokio::test]
async fn external_source_runtime_crud_dispatches_to_data_source_backend_after_acl_scope_resolution()
{
    let backend = Arc::new(CapturingDataSourceBackend::default());
    let model_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let actor_user_id = Uuid::now_v7();
    let data_source_instance_id = Uuid::now_v7();
    let engine = RuntimeEngine::for_tests_with_models_and_data_source_backend(
        vec![external_model_metadata(
            model_id,
            workspace_id,
            data_source_instance_id,
        )],
        backend.clone(),
    );
    let actor = ActorContext::scoped(actor_user_id, workspace_id, "member", Vec::<String>::new());
    let grant = RuntimeScopeGrant {
        data_model_id: model_id,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        enabled: true,
        permission_profile: domain::ScopeDataModelPermissionProfile::Owner,
    };

    let listed = engine
        .list_records(RuntimeListInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            filter: domain::ResourceFilterExpr::Field {
                field: "email".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("ada@example.test"),
            },
            sorts: vec![RuntimeSortInput {
                field_code: "created_at".into(),
                direction: "desc".into(),
            }],
            expand_relations: vec!["company".into()],
            page: 3,
            page_size: 25,
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(
        listed.items,
        vec![json!({
            "id": "external-1",
            "email": "ada@example.test",
            "name": "Ada",
            "created_by": actor_user_id.to_string(),
        })]
    );
    assert_eq!(listed.total, 41);

    let fetched = engine
        .get_record(RuntimeGetInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(
        fetched,
        Some(json!({
            "id": "external-1",
            "email": "ada@example.test",
            "name": "Ada",
            "created_by": actor_user_id.to_string(),
        }))
    );

    let created = engine
        .create_record(RuntimeCreateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            payload: json!({ "email": "ada@example.test", "name": "Ada" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(created["id"], json!("created-external"));
    assert_eq!(created["email"], json!("ada@example.test"));
    assert_eq!(created["name"], json!("Ada"));
    assert!(created.get("external_only").is_none());

    let updated = engine
        .update_record(RuntimeUpdateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            payload: json!({ "name": "Ada Lovelace" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(updated["name"], json!("Ada Lovelace"));

    let deleted = engine
        .delete_record(RuntimeDeleteInput {
            actor,
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            scope_grant: Some(grant),
        })
        .await
        .unwrap();
    assert_eq!(deleted, json!({ "deleted": true }));

    let calls = backend.calls.lock().unwrap();
    assert_eq!(calls.len(), 5);
    assert!(calls
        .iter()
        .all(|call| call.instance_id == data_source_instance_id));
    assert_eq!(calls[0].method, "list");
    assert_eq!(calls[0].payload["resource_key"], json!("crm.contacts"));
    assert_eq!(
        calls[0].payload["context"]["scope_id"],
        json!(workspace_id.to_string())
    );
    assert_eq!(
        calls[0].payload["context"]["owner_id"],
        json!(actor_user_id.to_string())
    );
    assert_eq!(
        calls[0].payload["filters"][0]["field_key"],
        json!("contact_email")
    );
    assert_eq!(
        calls[0].payload["sort"][0]["field_key"],
        json!("created_at_utc")
    );
    assert_eq!(calls[0].payload["sort"][0]["descending"], json!(true));
    assert_eq!(calls[0].payload["page"]["limit"], json!(25));
    assert_eq!(calls[0].payload["page"]["offset"], json!(50));
    assert_eq!(
        calls[0].payload["options_json"]["expand_relations"],
        json!(["company"])
    );
    assert_eq!(calls[1].payload["record_id"], json!("external-1"));
    assert_eq!(
        calls[2].payload["record"],
        json!({ "contact_email": "ada@example.test", "display_name": "Ada" })
    );
    assert_eq!(
        calls[2].payload["context"]["owner_id"],
        json!(actor_user_id.to_string())
    );
    assert_eq!(
        calls[2].payload["context"]["scope_id"],
        json!(workspace_id.to_string())
    );
    assert_eq!(calls[2].payload["transaction_id"], json!(null));
    assert_eq!(
        calls[3].payload["patch"],
        json!({ "display_name": "Ada Lovelace" })
    );
    assert_eq!(calls[3].payload["transaction_id"], json!(null));
    assert_eq!(calls[4].payload["record_id"], json!("external-1"));
    assert_eq!(calls[4].payload["transaction_id"], json!(null));
}

#[tokio::test]
async fn external_source_runtime_rejects_unknown_fields_and_invalid_sort_direction() {
    let backend = Arc::new(CapturingDataSourceBackend::default());
    let model_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let engine = RuntimeEngine::for_tests_with_models_and_data_source_backend(
        vec![external_model_metadata(
            model_id,
            workspace_id,
            Uuid::now_v7(),
        )],
        backend.clone(),
    );
    let actor = ActorContext::scoped(Uuid::now_v7(), workspace_id, "member", Vec::<String>::new());
    let grant = RuntimeScopeGrant {
        data_model_id: model_id,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        enabled: true,
        permission_profile: domain::ScopeDataModelPermissionProfile::Owner,
    };

    assert!(engine
        .create_record(RuntimeCreateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            payload: json!({ "unknown": "value" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unknown runtime field"));

    assert!(engine
        .update_record(RuntimeUpdateInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            record_id: "external-1".into(),
            payload: json!({ "contact_email": "external-key-should-not-enter" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unknown runtime field"));

    assert!(engine
        .list_records(RuntimeListInput {
            actor: actor.clone(),
            model_code: "external_contacts".into(),
            filter: domain::ResourceFilterExpr::Field {
                field: "unknown".into(),
                operator: domain::ResourceFilterOperator::Eq,
                value: json!("value"),
            },
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unknown runtime field"));

    assert!(engine
        .list_records(RuntimeListInput {
            actor,
            model_code: "external_contacts".into(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![RuntimeSortInput {
                field_code: "email".into(),
                direction: "sideways".into(),
            }],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(grant),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unsupported sort direction"));

    assert!(backend.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn runtime_engine_uses_fixed_system_scope_id_for_system_models() {
    let engine = RuntimeEngine::for_tests();
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
    let model_code = "system_orders";
    let model_id = Uuid::now_v7();
    engine.registry().rebuild(vec![ModelMetadata {
        model_id,
        model_code: model_code.into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::System,
        scope_id: domain::SYSTEM_SCOPE_ID,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: "rtm_system_demo_orders".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(model_code, domain::DataModelScopeKind::System),
    }]);

    let created = engine
        .create_record(RuntimeCreateInput {
            actor: actor.clone(),
            model_code: model_code.into(),
            payload: json!({ "title": "system-order" }),
            scope_grant: Some(scope_grant(model_id, domain::SYSTEM_SCOPE_ID)),
        })
        .await
        .unwrap();
    let record_id = created["id"].as_str().unwrap().to_string();

    let fetched = engine
        .get_record(RuntimeGetInput {
            actor,
            model_code: model_code.into(),
            record_id,
            scope_grant: Some(scope_grant(model_id, domain::SYSTEM_SCOPE_ID)),
        })
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fetched["title"], json!("system-order"));
}

#[tokio::test]
async fn runtime_engine_prefers_workspace_metadata_before_system_fallback() {
    let engine = RuntimeEngine::for_tests();
    let workspace_id = Uuid::now_v7();
    let actor = ActorContext::root(Uuid::now_v7(), workspace_id, "root");
    let model_code = "shared_orders";
    let workspace_metadata = ModelMetadata {
        model_id: Uuid::now_v7(),
        model_code: model_code.into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: "rtm_workspace_demo_orders".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(
            model_code,
            domain::DataModelScopeKind::Workspace,
        ),
    };
    let system_metadata = ModelMetadata {
        model_id: Uuid::now_v7(),
        model_code: model_code.into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::System,
        scope_id: domain::SYSTEM_SCOPE_ID,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: "rtm_system_demo_orders".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(model_code, domain::DataModelScopeKind::System),
    };
    engine
        .registry()
        .rebuild(vec![workspace_metadata.clone(), system_metadata]);
    let grant = scope_grant(workspace_metadata.model_id, workspace_id);

    engine
        .create_record(RuntimeCreateInput {
            actor: actor.clone(),
            model_code: model_code.into(),
            payload: json!({ "title": "workspace-order" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();

    engine.registry().rebuild(vec![workspace_metadata]);

    let listed = engine
        .list_records(RuntimeListInput {
            actor,
            model_code: model_code.into(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(grant),
        })
        .await
        .unwrap();

    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0]["title"], json!("workspace-order"));
}

#[tokio::test]
async fn draft_model_is_visible_in_metadata_but_blocked_from_crud() {
    let engine = runtime_engine_for_status(domain::DataModelStatus::Draft);
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::nil(), "root");

    assert!(engine
        .registry()
        .get(
            domain::DataModelScopeKind::Workspace,
            Uuid::nil(),
            "status_orders"
        )
        .is_some());

    assert_crud_blocked_by_model_error(
        &engine,
        actor,
        RuntimeModelError::not_published("status_orders"),
    )
    .await;
}

#[tokio::test]
async fn disabled_model_returns_disabled_error() {
    let engine = runtime_engine_for_status(domain::DataModelStatus::Disabled);
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::nil(), "root");

    assert_crud_blocked_by_model_error(
        &engine,
        actor,
        RuntimeModelError::disabled("status_orders"),
    )
    .await;
}

#[tokio::test]
async fn broken_model_returns_broken_error() {
    let engine = runtime_engine_for_status(domain::DataModelStatus::Broken);
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::nil(), "root");

    assert_crud_blocked_by_model_error(&engine, actor, RuntimeModelError::broken("status_orders"))
        .await;
}

#[tokio::test]
async fn runtime_create_validates_api_required_fields_not_metadata_required() {
    let model_id = Uuid::now_v7();
    let engine = RuntimeEngine::for_tests_with_models(vec![ModelMetadata {
        model_id,
        model_code: "api_required_orders".into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: Uuid::nil(),
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: "rtm_workspace_api_required_orders".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![
            domain::ModelFieldRecord {
                id: Uuid::now_v7(),
                data_model_id: model_id,
                code: "title".into(),
                title: "Title".into(),
                description: None,
                physical_column_name: "title".into(),
                external_field_key: None,
                field_kind: domain::ModelFieldKind::String,
                is_system: false,
                is_writable: true,
                is_required: true,
                api_required: false,
                is_unique: false,
                default_value: None,
                display_interface: None,
                display_options: json!({}),
                relation_target_model_id: None,
                relation_options: json!({}),
                sort_order: 0,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
            domain::ModelFieldRecord {
                id: Uuid::now_v7(),
                data_model_id: model_id,
                code: "status".into(),
                title: "Status".into(),
                description: None,
                physical_column_name: "status".into(),
                external_field_key: None,
                field_kind: domain::ModelFieldKind::String,
                is_system: false,
                is_writable: true,
                is_required: false,
                api_required: true,
                is_unique: false,
                default_value: None,
                display_interface: None,
                display_options: json!({}),
                relation_target_model_id: None,
                relation_options: json!({}),
                sort_order: 1,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
        ],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(
            "api_required_orders",
            domain::DataModelScopeKind::Workspace,
        ),
    }]);
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::nil(), "root");

    assert_model_error(
        engine
            .create_record(RuntimeCreateInput {
                actor: actor.clone(),
                model_code: "api_required_orders".into(),
                payload: json!({ "title": "A-001" }),
                scope_grant: Some(scope_grant(model_id, Uuid::nil())),
            })
            .await
            .unwrap_err(),
        RuntimeModelError::missing_create_required_fields("api_required_orders", vec!["status"]),
    );

    let created = engine
        .create_record(RuntimeCreateInput {
            actor,
            model_code: "api_required_orders".into(),
            payload: json!({ "status": "draft" }),
            scope_grant: Some(scope_grant(model_id, Uuid::nil())),
        })
        .await
        .unwrap();

    assert_eq!(created["status"], json!("draft"));
}

fn runtime_engine_for_status(status: domain::DataModelStatus) -> RuntimeEngine {
    let engine = RuntimeEngine::for_tests();
    engine
        .registry()
        .rebuild_with_status(vec![(status_model_metadata("status_orders"), status)]);
    engine
}

fn status_model_metadata(model_code: &str) -> ModelMetadata {
    ModelMetadata {
        model_id: Uuid::now_v7(),
        model_code: model_code.into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: Uuid::nil(),
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: format!("rtm_workspace_demo_{model_code}"),
        scope_column_name: "scope_id".into(),
        fields: vec![],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(
            model_code,
            domain::DataModelScopeKind::Workspace,
        ),
    }
}

fn external_model_metadata(
    model_id: Uuid,
    workspace_id: Uuid,
    data_source_instance_id: Uuid,
) -> ModelMetadata {
    ModelMetadata {
        model_id,
        model_code: "external_contacts".into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        data_source_instance_id: Some(data_source_instance_id),
        source_kind: domain::DataModelSourceKind::ExternalSource,
        external_resource_key: Some("crm.contacts".into()),
        physical_table_name: "external_contacts".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![
            domain::ModelFieldRecord {
                id: Uuid::now_v7(),
                data_model_id: model_id,
                code: "email".into(),
                title: "Email".into(),
                description: None,
                physical_column_name: "email".into(),
                external_field_key: Some("contact_email".into()),
                field_kind: domain::ModelFieldKind::String,
                is_required: false,
                api_required: false,
                is_system: false,
                is_unique: false,
                is_writable: true,
                default_value: None,
                display_interface: None,
                display_options: json!({}),
                relation_target_model_id: None,
                relation_options: json!({}),
                sort_order: 0,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
            domain::ModelFieldRecord {
                id: Uuid::now_v7(),
                data_model_id: model_id,
                code: "created_at".into(),
                title: "Created At".into(),
                description: None,
                physical_column_name: "created_at".into(),
                external_field_key: Some("created_at_utc".into()),
                field_kind: domain::ModelFieldKind::String,
                is_required: false,
                api_required: false,
                is_system: false,
                is_unique: false,
                is_writable: true,
                default_value: None,
                display_interface: None,
                display_options: json!({}),
                relation_target_model_id: None,
                relation_options: json!({}),
                sort_order: 1,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
            domain::ModelFieldRecord {
                id: Uuid::now_v7(),
                data_model_id: model_id,
                code: "name".into(),
                title: "Name".into(),
                description: None,
                physical_column_name: "name".into(),
                external_field_key: Some("display_name".into()),
                field_kind: domain::ModelFieldKind::String,
                is_required: false,
                api_required: false,
                is_system: false,
                is_unique: false,
                is_writable: true,
                default_value: None,
                display_interface: None,
                display_options: json!({}),
                relation_target_model_id: None,
                relation_options: json!({}),
                sort_order: 2,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
        ],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model(
            "external_contacts",
            domain::DataModelScopeKind::Workspace,
        ),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CapturedDataSourceCall {
    method: &'static str,
    instance_id: Uuid,
    payload: serde_json::Value,
}

#[derive(Default)]
struct CapturingDataSourceBackend {
    calls: Mutex<Vec<CapturedDataSourceCall>>,
}

impl CapturingDataSourceBackend {
    fn capture<T: serde::Serialize>(&self, method: &'static str, instance_id: Uuid, input: &T) {
        self.calls.lock().unwrap().push(CapturedDataSourceCall {
            method,
            instance_id,
            payload: serde_json::to_value(input).unwrap(),
        });
    }
}

#[async_trait::async_trait]
impl DataSourceRuntimeRecordBackend for CapturingDataSourceBackend {
    async fn list_records(
        &self,
        _workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceListRecordsInput,
    ) -> anyhow::Result<DataSourceListRecordsOutput> {
        self.capture("list", data_source_instance_id, &input);
        Ok(DataSourceListRecordsOutput {
            rows: vec![json!({
                "id": "external-1",
                "contact_email": "ada@example.test",
                "display_name": "Ada",
                "created_by": input.context.owner_id,
                "external_only": "hidden",
            })],
            next_cursor: None,
            total_count: Some(41),
            metadata: json!({}),
        })
    }

    async fn get_record(
        &self,
        _workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceGetRecordInput,
    ) -> anyhow::Result<DataSourceGetRecordOutput> {
        self.capture("get", data_source_instance_id, &input);
        Ok(DataSourceGetRecordOutput {
            record: Some(json!({
                "id": "external-1",
                "contact_email": "ada@example.test",
                "display_name": "Ada",
                "created_by": input.context.owner_id,
                "external_only": "hidden",
            })),
            metadata: json!({}),
        })
    }

    async fn create_record(
        &self,
        _workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceCreateRecordInput,
    ) -> anyhow::Result<DataSourceCreateRecordOutput> {
        self.capture("create", data_source_instance_id, &input);
        Ok(DataSourceCreateRecordOutput {
            record: json!({
                "id": "created-external",
                "contact_email": input.record["contact_email"],
                "display_name": input.record["display_name"],
                "external_only": "hidden",
            }),
            metadata: json!({}),
        })
    }

    async fn update_record(
        &self,
        _workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceUpdateRecordInput,
    ) -> anyhow::Result<DataSourceUpdateRecordOutput> {
        self.capture("update", data_source_instance_id, &input);
        Ok(DataSourceUpdateRecordOutput {
            record: json!({
                "id": "external-1",
                "display_name": input.patch["display_name"],
                "external_only": "hidden",
            }),
            metadata: json!({}),
        })
    }

    async fn delete_record(
        &self,
        _workspace_id: Uuid,
        data_source_instance_id: Uuid,
        input: DataSourceDeleteRecordInput,
    ) -> anyhow::Result<DataSourceDeleteRecordOutput> {
        self.capture("delete", data_source_instance_id, &input);
        Ok(DataSourceDeleteRecordOutput {
            deleted: true,
            metadata: json!({}),
        })
    }
}

async fn assert_crud_blocked_by_model_error(
    engine: &RuntimeEngine,
    actor: ActorContext,
    expected: RuntimeModelError,
) {
    assert_model_error(
        engine
            .list_records(RuntimeListInput {
                actor: actor.clone(),
                model_code: "status_orders".into(),
                filter: domain::ResourceFilterExpr::All(vec![]),
                sorts: vec![],
                expand_relations: vec![],
                page: 1,
                page_size: 20,
                scope_grant: Some(scope_grant(Uuid::nil(), Uuid::nil())),
            })
            .await
            .unwrap_err(),
        expected.clone(),
    );
    assert_model_error(
        engine
            .get_record(RuntimeGetInput {
                actor: actor.clone(),
                model_code: "status_orders".into(),
                record_id: "missing".into(),
                scope_grant: Some(scope_grant(Uuid::nil(), Uuid::nil())),
            })
            .await
            .unwrap_err(),
        expected.clone(),
    );
    assert_model_error(
        engine
            .create_record(RuntimeCreateInput {
                actor: actor.clone(),
                model_code: "status_orders".into(),
                payload: json!({ "title": "A-001" }),
                scope_grant: Some(scope_grant(Uuid::nil(), Uuid::nil())),
            })
            .await
            .unwrap_err(),
        expected.clone(),
    );
    assert_model_error(
        engine
            .update_record(RuntimeUpdateInput {
                actor: actor.clone(),
                model_code: "status_orders".into(),
                record_id: "missing".into(),
                payload: json!({ "title": "A-002" }),
                scope_grant: Some(scope_grant(Uuid::nil(), Uuid::nil())),
            })
            .await
            .unwrap_err(),
        expected.clone(),
    );
    assert_model_error(
        engine
            .delete_record(RuntimeDeleteInput {
                actor,
                model_code: "status_orders".into(),
                record_id: "missing".into(),
                scope_grant: Some(scope_grant(Uuid::nil(), Uuid::nil())),
            })
            .await
            .unwrap_err(),
        expected,
    );
}

fn assert_model_error(error: anyhow::Error, expected: RuntimeModelError) {
    let actual = error.downcast_ref::<RuntimeModelError>().unwrap();
    assert_eq!(actual, &expected);
}
