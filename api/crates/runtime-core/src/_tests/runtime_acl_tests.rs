use domain::ActorContext;
use runtime_core::model_metadata::ModelMetadata;
use runtime_core::resource_descriptor::ResourceDescriptor;
use runtime_core::runtime_acl::RuntimeScopeGrant;
use runtime_core::runtime_engine::{
    RuntimeCreateInput, RuntimeDeleteInput, RuntimeEngine, RuntimeGetInput, RuntimeListInput,
    RuntimeUpdateInput,
};
use serde_json::json;
use uuid::Uuid;

fn scoped_actor(
    user_id: Uuid,
    workspace_id: Uuid,
    permissions: impl IntoIterator<Item = &'static str>,
) -> ActorContext {
    ActorContext::scoped(
        user_id,
        workspace_id,
        "member",
        permissions.into_iter().map(str::to_string),
    )
}

fn scope_grant(
    scope_id: Uuid,
    permission_profile: domain::ScopeDataModelPermissionProfile,
) -> RuntimeScopeGrant {
    RuntimeScopeGrant {
        data_model_id: Uuid::nil(),
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id,
        enabled: true,
        permission_profile,
    }
}

fn system_model_metadata(model_id: Uuid) -> ModelMetadata {
    ModelMetadata {
        model_id,
        model_code: "orders".into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::System,
        scope_id: domain::SYSTEM_SCOPE_ID,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        physical_table_name: "rtm_system_orders".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: ResourceDescriptor::runtime_model("orders", domain::DataModelScopeKind::System),
    }
}

#[tokio::test]
async fn state_data_view_own_filters_list_by_created_by() {
    let workspace_id = Uuid::nil();
    let grant = scope_grant(workspace_id, domain::ScopeDataModelPermissionProfile::Owner);
    let manager_user_id = Uuid::now_v7();
    let manager = scoped_actor(
        manager_user_id,
        workspace_id,
        ["state_data.create.all", "state_data.view.own"],
    );
    let admin = scoped_actor(
        Uuid::now_v7(),
        workspace_id,
        ["state_data.create.all", "state_data.view.all"],
    );
    let engine = RuntimeEngine::for_tests();

    engine
        .create_record(RuntimeCreateInput {
            actor: manager.clone(),
            model_code: "orders".into(),
            payload: json!({ "title": "manager-order", "status": "draft" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    engine
        .create_record(RuntimeCreateInput {
            actor: admin,
            model_code: "orders".into(),
            payload: json!({ "title": "admin-order", "status": "draft" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();

    let listed = engine
        .list_records(RuntimeListInput {
            actor: manager,
            model_code: "orders".into(),
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
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0]["title"], json!("manager-order"));
}

#[tokio::test]
async fn state_data_edit_own_rejects_updating_another_users_record() {
    let workspace_id = Uuid::nil();
    let grant = scope_grant(workspace_id, domain::ScopeDataModelPermissionProfile::Owner);
    let manager = scoped_actor(
        Uuid::now_v7(),
        workspace_id,
        ["state_data.create.all", "state_data.edit.own"],
    );
    let admin = scoped_actor(
        Uuid::now_v7(),
        workspace_id,
        ["state_data.create.all", "state_data.edit.all"],
    );
    let engine = RuntimeEngine::for_tests();

    let foreign_record = engine
        .create_record(RuntimeCreateInput {
            actor: admin,
            model_code: "orders".into(),
            payload: json!({ "title": "admin-order", "status": "draft" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    let foreign_record_id = foreign_record["id"].as_str().unwrap().to_string();

    let error = engine
        .update_record(RuntimeUpdateInput {
            actor: manager,
            model_code: "orders".into(),
            record_id: foreign_record_id,
            payload: json!({ "title": "blocked-update", "status": "draft" }),
            scope_grant: Some(grant),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("runtime record not found"));
}

#[tokio::test]
async fn state_data_delete_all_allows_cross_owner_delete() {
    let workspace_id = Uuid::nil();
    let grant = scope_grant(
        workspace_id,
        domain::ScopeDataModelPermissionProfile::ScopeAll,
    );
    let manager = scoped_actor(
        Uuid::now_v7(),
        workspace_id,
        [
            "state_data.create.all",
            "state_data.delete.own",
            "state_data.view.own",
        ],
    );
    let admin = scoped_actor(
        Uuid::now_v7(),
        workspace_id,
        ["state_data.delete.all", "state_data.view.all"],
    );
    let engine = RuntimeEngine::for_tests();

    let record = engine
        .create_record(RuntimeCreateInput {
            actor: manager.clone(),
            model_code: "orders".into(),
            payload: json!({ "title": "manager-order", "status": "draft" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    let record_id = record["id"].as_str().unwrap().to_string();

    let deleted = engine
        .delete_record(RuntimeDeleteInput {
            actor: admin.clone(),
            model_code: "orders".into(),
            record_id: record_id.clone(),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(deleted["deleted"], json!(true));

    let fetched = engine
        .get_record(RuntimeGetInput {
            actor: admin,
            model_code: "orders".into(),
            record_id,
            scope_grant: Some(grant),
        })
        .await
        .unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn scope_without_grant_cannot_call_runtime_crud() {
    let workspace_id = Uuid::nil();
    let actor = scoped_actor(
        Uuid::now_v7(),
        workspace_id,
        ["state_data.create.all", "state_data.view.all"],
    );
    let engine = RuntimeEngine::for_tests();

    let error = engine
        .list_records(RuntimeListInput {
            actor,
            model_code: "orders".into(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: None,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("data_model_scope_not_granted"));
}

#[tokio::test]
async fn owner_permission_only_returns_actor_owned_records() {
    let workspace_id = Uuid::nil();
    let grant = scope_grant(workspace_id, domain::ScopeDataModelPermissionProfile::Owner);
    let owner = scoped_actor(Uuid::now_v7(), workspace_id, Vec::<&'static str>::new());
    let other = scoped_actor(Uuid::now_v7(), workspace_id, Vec::<&'static str>::new());
    let engine = RuntimeEngine::for_tests();

    engine
        .create_record(RuntimeCreateInput {
            actor: owner.clone(),
            model_code: "orders".into(),
            payload: json!({ "title": "owner-order" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    engine
        .create_record(RuntimeCreateInput {
            actor: other,
            model_code: "orders".into(),
            payload: json!({ "title": "other-order" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();

    let listed = engine
        .list_records(RuntimeListInput {
            actor: owner,
            model_code: "orders".into(),
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
    assert_eq!(listed.items[0]["title"], json!("owner-order"));
}

#[tokio::test]
async fn scope_all_returns_all_records_inside_granted_scope_id() {
    let workspace_id = Uuid::nil();
    let grant = scope_grant(
        workspace_id,
        domain::ScopeDataModelPermissionProfile::ScopeAll,
    );
    let owner = scoped_actor(Uuid::now_v7(), workspace_id, Vec::<&'static str>::new());
    let other = scoped_actor(Uuid::now_v7(), workspace_id, Vec::<&'static str>::new());
    let engine = RuntimeEngine::for_tests();

    engine
        .create_record(RuntimeCreateInput {
            actor: owner.clone(),
            model_code: "orders".into(),
            payload: json!({ "title": "owner-order" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();
    engine
        .create_record(RuntimeCreateInput {
            actor: other,
            model_code: "orders".into(),
            payload: json!({ "title": "other-order" }),
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap();

    let listed = engine
        .list_records(RuntimeListInput {
            actor: owner,
            model_code: "orders".into(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(grant),
        })
        .await
        .unwrap();

    assert_eq!(listed.total, 2);
}

#[tokio::test]
async fn system_all_returns_all_granted_data_for_system_actor_only() {
    let model_id = Uuid::nil();
    let scope_a = Uuid::now_v7();
    let scope_b = Uuid::now_v7();
    let engine = RuntimeEngine::for_tests_with_models(vec![system_model_metadata(model_id)]);
    let actor_a = scoped_actor(Uuid::now_v7(), scope_a, Vec::<&'static str>::new());
    let actor_b = scoped_actor(Uuid::now_v7(), scope_b, Vec::<&'static str>::new());
    let system_actor = ActorContext::root(Uuid::now_v7(), domain::SYSTEM_SCOPE_ID, "root");
    let non_system_actor = scoped_actor(
        Uuid::now_v7(),
        domain::SYSTEM_SCOPE_ID,
        Vec::<&'static str>::new(),
    );
    let grant_a = scope_grant(scope_a, domain::ScopeDataModelPermissionProfile::ScopeAll);
    let grant_b = scope_grant(scope_b, domain::ScopeDataModelPermissionProfile::ScopeAll);
    let system_grant = scope_grant(
        domain::SYSTEM_SCOPE_ID,
        domain::ScopeDataModelPermissionProfile::SystemAll,
    );

    engine
        .create_record(RuntimeCreateInput {
            actor: actor_a,
            model_code: "orders".into(),
            payload: json!({ "title": "scope-a" }),
            scope_grant: Some(grant_a),
        })
        .await
        .unwrap();
    engine
        .create_record(RuntimeCreateInput {
            actor: actor_b,
            model_code: "orders".into(),
            payload: json!({ "title": "scope-b" }),
            scope_grant: Some(grant_b),
        })
        .await
        .unwrap();

    let listed = engine
        .list_records(RuntimeListInput {
            actor: system_actor,
            model_code: "orders".into(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(system_grant.clone()),
        })
        .await
        .unwrap();
    assert_eq!(listed.total, 2);

    let error = engine
        .list_records(RuntimeListInput {
            actor: non_system_actor,
            model_code: "orders".into(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 20,
            scope_grant: Some(system_grant),
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("system_all_requires_system_actor"));
}
