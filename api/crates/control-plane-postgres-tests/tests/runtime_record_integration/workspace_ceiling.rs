use std::sync::Arc;

use control_plane::{
    errors::ControlPlaneError,
    file_management::FilePreviewProjector,
    model_definition::ModelDefinitionService,
    role::{ReplaceRoleDataPolicyCommand, RoleService},
};
use control_plane_contracts::ports::{
    AuthRepository, CreateScopeDataModelGrantInput, ReconcileSystemModelDefinitionInput,
    ReplaceRoleConsolePolicyInput, RoleDataModelPolicyInput, RoleDataPolicyDefaultsInput,
    RoleRepository,
};
use domain::{ActorContext, RoleDataPolicyScope, ScopeDataModelPermissionProfile};
use runtime_core::{
    model_metadata::ModelMetadata,
    runtime_acl::{RuntimeDataAction, RuntimeScopeGrant},
    runtime_engine::{RuntimeEngine, RuntimeGetInput, RuntimeListInput},
    runtime_model_registry::RuntimeModelRegistry,
};

use super::*;

async fn seed_runtime_applications_metadata(store: &PgControlPlaneStore, actor_id: Uuid) -> Uuid {
    // The application API owns this table. Register a fixture projection so the runtime
    // test exercises the same physical rows without adding a production model registration.
    let model = store
        .create_model_definition(&CreateModelDefinitionInput {
            bound_storage_id: None,
            actor_user_id: actor_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.into(),
            template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.into(),
            template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.into(),
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
            code: "ac011_applications_fixture".into(),
            title: "Applications".into(),
            description: None,
        })
        .await
        .unwrap();
    store
        .reconcile_system_model_definition(&ReconcileSystemModelDefinitionInput {
            actor_user_id: actor_id,
            model_id: model.id,
            title: model.title,
            physical_table_name: "applications".into(),
            status: domain::DataModelStatus::Published,
            protection: domain::DataModelProtection::default(),
        })
        .await
        .unwrap();
    model.id
}

async fn seed_scope_grant(
    store: &PgControlPlaneStore,
    model_id: Uuid,
    scope_kind: DataModelScopeKind,
    scope_id: Uuid,
    permission_profile: ScopeDataModelPermissionProfile,
) {
    store
        .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
            grant_id: Uuid::now_v7(),
            scope_kind,
            scope_id,
            data_model_id: model_id,
            enabled: true,
            permission_profile,
            created_by: None,
        })
        .await
        .unwrap();
}

async fn runtime_list(
    service: &ModelDefinitionService<PgControlPlaneStore>,
    engine: &RuntimeEngine,
    actor: &ActorContext,
    model: &ModelMetadata,
) -> (RuntimeScopeGrant, Vec<serde_json::Value>) {
    let grant = service
        .load_runtime_scope_grant(actor, model.model_id, RuntimeDataAction::View)
        .await
        .unwrap()
        .expect("fixture actor has an enabled model grant");
    let listed = engine
        .list_records(RuntimeListInput {
            actor: actor.clone(),
            model_code: model.model_code.clone(),
            filter: domain::ResourceFilterExpr::All(vec![]),
            sorts: vec![],
            expand_relations: vec![],
            page: 1,
            page_size: 200,
            scope_grant: Some(grant.clone()),
        })
        .await
        .unwrap_or_else(|err| panic!("{}: {err:#}", model.model_code));
    assert_eq!(listed.total.min(200) as usize, listed.items.len());
    (grant, listed.items)
}

#[tokio::test]
async fn customer_workspace_caps_admin_and_switched_root_including_unscoped_builtins() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let [bootstrap, workspace_a, workspace_b, customer_tenant] = [(); 4].map(|_| Uuid::now_v7());
    let [root_user, user_a, admin_b, shared_user, disabled_member, disposable_user] =
        [(); 6].map(|_| Uuid::now_v7());
    insert_workspace(&store, bootstrap).await;
    sqlx::query("insert into tenants (id, code, name) values ($1, 'ac011-customer', 'Customer')")
        .bind(customer_tenant)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_at) values
         ($1, $3, 'Workspace A', now() - interval '2 days'),
         ($2, $3, 'Workspace B', now() - interval '1 day')",
    )
    .bind(workspace_a)
    .bind(workspace_b)
    .bind(customer_tenant)
    .execute(store.pool())
    .await
    .unwrap();
    for (id, code) in [
        (root_user, "root-fixture"),
        (user_a, "a-only"),
        (admin_b, "b-admin"),
        (shared_user, "shared"),
        (disabled_member, "disabled-b"),
        (disposable_user, "b-disposable"),
    ] {
        insert_user(&store, id, code).await;
    }
    for (workspace, user, status) in [
        (bootstrap, root_user, "active"),
        (workspace_a, user_a, "active"),
        (workspace_a, shared_user, "active"),
        (workspace_b, shared_user, "active"),
        (workspace_b, admin_b, "active"),
        (workspace_b, disabled_member, "disabled"),
        (workspace_b, disposable_user, "active"),
    ] {
        sqlx::query("insert into workspace_memberships (id, workspace_id, user_id, status) values ($1, $2, $3, $4)")
            .bind(Uuid::now_v7()).bind(workspace).bind(user).bind(status)
            .execute(store.pool()).await.unwrap();
    }
    let [role_a, role_b, app_a, app_b, attachment_a, attachment_b] =
        [(); 6].map(|_| Uuid::now_v7());
    let storage = store
        .create_file_storage(&CreateFileStorageInput {
            storage_id: Uuid::now_v7(),
            actor_user_id: root_user,
            code: "local-ac011".into(),
            title: "Local".into(),
            driver_type: "local".into(),
            enabled: true,
            is_default: true,
            config_json: json!({}),
            rule_json: json!({}),
        })
        .await
        .unwrap();
    let attachments = FileManagementBootstrapService::new(store.clone())
        .ensure_builtin_attachments(root_user, storage.id, "attachments")
        .await
        .unwrap();
    let system_metadata = SystemMetadataBootstrapService::new(store.clone());
    system_metadata
        .ensure_builtin_user_and_role_models(root_user)
        .await
        .unwrap();
    system_metadata
        .ensure_builtin_model_pricing_rules(root_user)
        .await
        .unwrap();
    let applications_model_id = seed_runtime_applications_metadata(&store, root_user).await;
    seed_scope_grant(
        &store,
        applications_model_id,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
        ScopeDataModelPermissionProfile::SystemAll,
    )
    .await;

    for (workspace, author, role, app, attachment) in [
        (workspace_a, user_a, role_a, app_a, attachment_a),
        (workspace_b, admin_b, role_b, app_b, attachment_b),
    ] {
        sqlx::query("insert into roles (id, scope_id, scope_kind, workspace_id, code, name, introduction, is_builtin, is_editable)
                    values ($1, $2, 'workspace', $2, 'admin', 'Admin', '', false, true)")
            .bind(role).bind(workspace).execute(store.pool()).await.unwrap();
        sqlx::query("insert into applications (id, workspace_id, application_type, name, description, created_by, updated_by)
                    values ($1, $2, 'agent_flow', 'Workspace App', '', $3, $3)")
            .bind(app).bind(workspace).bind(author).execute(store.pool()).await.unwrap();
        sqlx::query("insert into attachments (id, scope_id, filename, size, mimetype, path, meta, storage_id, created_by, updated_by)
                    values ($1, $2, 'file.txt', 1, 'text/plain', 'fixture/file.txt', '{}', $3, $4, $4)")
            .bind(attachment).bind(workspace).bind(storage.id).bind(author).execute(store.pool()).await.unwrap();
    }
    sqlx::query(
        "insert into user_role_bindings (id, user_id, role_id, scope_id) values ($1, $2, $3, $4)",
    )
    .bind(Uuid::now_v7())
    .bind(admin_b)
    .bind(role_b)
    .bind(workspace_b)
    .execute(store.pool())
    .await
    .unwrap();
    store
        .replace_role_console_policy(&ReplaceRoleConsolePolicyInput {
            actor_user_id: root_user,
            workspace_id: workspace_b,
            role_code: "admin".into(),
            groups: vec![domain::RoleConsoleGroupPolicy::custom(
                domain::ConsolePolicyGroup::settings_feature("system.roles").unwrap(),
                vec![domain::ConsoleOperationPolicy::simple(
                    "roles.data_policy.replace".try_into().unwrap(),
                    true,
                )],
            )],
        })
        .await
        .unwrap();
    let actor_b = AuthRepository::load_actor_context(
        &store,
        admin_b,
        customer_tenant,
        workspace_b,
        Some("admin"),
    )
    .await
    .unwrap();
    assert!(!actor_b.is_root);
    let scoped_store = store.for_actor(actor_b.clone());
    let roles = RoleService::new(scoped_store.clone());
    for (default_scope, override_scope) in [
        (RoleDataPolicyScope::SystemAll, None),
        (
            RoleDataPolicyScope::ScopeAll,
            Some(RoleDataPolicyScope::SystemAll),
        ),
        (RoleDataPolicyScope::ScopeAll, None),
    ] {
        let result = roles
            .replace_data_policy(ReplaceRoleDataPolicyCommand {
                actor_user_id: admin_b,
                role_code: "admin".into(),
                default_policy: RoleDataPolicyDefaultsInput {
                    can_view: true,
                    can_create: false,
                    can_update: false,
                    can_delete: false,
                    default_view_scope: default_scope,
                    default_update_scope: RoleDataPolicyScope::Own,
                    default_delete_scope: RoleDataPolicyScope::Own,
                },
                model_policies: vec![RoleDataModelPolicyInput {
                    data_model_id: applications_model_id,
                    can_create_override: None,
                    view_scope_override: override_scope,
                    update_scope_override: None,
                    delete_scope_override: None,
                }],
            })
            .await;
        if default_scope == RoleDataPolicyScope::SystemAll || override_scope.is_some() {
            assert!(matches!(
                result.unwrap_err().downcast_ref::<ControlPlaneError>(),
                Some(ControlPlaneError::InvalidInput(
                    "system_all_requires_system_role"
                ))
            ));
        } else {
            result.unwrap();
        }
    }

    let mut models = store.list_runtime_model_metadata().await.unwrap();
    let examples = [
        ("applications", vec![app_b], vec![app_a]),
        ("roles", vec![role_b], vec![role_a]),
        ("attachments", vec![attachment_b], vec![attachment_a]),
        (
            "users",
            vec![admin_b, shared_user, disposable_user],
            vec![user_a, disabled_member, root_user],
        ),
    ];
    // Runtime metadata is loaded from the real registry; use only columns this
    // fixture projects for applications and point created_by at the real users model.
    let users_model = models
        .iter()
        .find(|m| m.model_code == "users")
        .unwrap()
        .clone();
    let roles_model = models
        .iter()
        .find(|m| m.model_code == "roles")
        .unwrap()
        .clone();
    let applications = models
        .iter_mut()
        .find(|m| m.model_id == applications_model_id)
        .unwrap();
    applications.model_code = "applications".into();
    applications.fields = roles_model
        .fields
        .iter()
        .filter(|f| f.code == "id" || f.code == "name" || f.code == "created_by")
        .cloned()
        .collect();
    let author_relation = applications
        .fields
        .iter_mut()
        .find(|f| f.code == "created_by")
        .unwrap();
    author_relation.field_kind = ModelFieldKind::ManyToOne;
    author_relation.relation_target_model_id = Some(users_model.model_id);
    sqlx::query("update applications set created_by = $1 where id = $2")
        .bind(user_a)
        .bind(app_b)
        .execute(store.pool())
        .await
        .unwrap();
    let pricing = models
        .iter()
        .find(|m| m.model_code == "model_pricing_rules")
        .unwrap();
    assert!(!pricing.record_capabilities.can_update);
    assert!(!pricing.record_capabilities.can_delete);
    assert!(!pricing.record_capabilities.can_create);
    for code in [
        "applications",
        "roles",
        "attachments",
        "users",
        "model_pricing_rules",
    ] {
        let metadata = models.iter().find(|m| m.model_code == code).unwrap();
        seed_scope_grant(
            &store,
            metadata.model_id,
            DataModelScopeKind::Workspace,
            workspace_b,
            ScopeDataModelPermissionProfile::ScopeAll,
        )
        .await;
    }
    assert_eq!(
        attachments.model_definition_id,
        models
            .iter()
            .find(|m| m.model_code == "attachments")
            .unwrap()
            .model_id
    );
    let registry = RuntimeModelRegistry::default();
    registry.rebuild(models.clone());
    let engine = RuntimeEngine::new(registry, Arc::new(store.clone()))
        .with_computed_field_resolver(Arc::new(FilePreviewProjector::new(store.clone())));
    let root_b = ActorContext::root_in_scope(root_user, customer_tenant, workspace_b, "root");
    let root_bootstrap =
        ActorContext::root_in_scope(root_user, root_tenant_id(&store).await, bootstrap, "root");
    for actor in [&actor_b, &root_b, &root_bootstrap] {
        let service = ModelDefinitionService::new(store.for_actor(actor.clone()));
        for (code, included, excluded) in &examples {
            let metadata = models.iter().find(|m| m.model_code == *code).unwrap();
            let (grant, items) = runtime_list(&service, &engine, actor, metadata).await;
            let actual = items
                .iter()
                .map(|row| Uuid::parse_str(row["id"].as_str().unwrap()).unwrap())
                .collect::<std::collections::HashSet<_>>();
            for id in included {
                assert!(actual.contains(id), "{code} should contain {id}");
            }
            if actor.current_workspace_id == bootstrap {
                assert_eq!(
                    grant.permission_profile,
                    ScopeDataModelPermissionProfile::SystemAll
                );
                assert!(
                    excluded.iter().all(|id| actual.contains(id)),
                    "{code}: {actual:?}"
                );
            } else {
                assert_eq!(grant.scope_id, workspace_b);
                assert_eq!(actual, included.iter().copied().collect(), "{code}");
                for id in excluded {
                    assert!(
                        engine
                            .get_record(RuntimeGetInput {
                                actor: actor.clone(),
                                model_code: code.to_string(),
                                record_id: id.to_string(),
                                scope_grant: Some(grant.clone()),
                            })
                            .await
                            .unwrap()
                            .is_none(),
                        "{code}: foreign id {id}"
                    );
                }
            }
        }
        let (grant, rows) = runtime_list(&service, &engine, actor, pricing).await;
        let first = rows.first().expect("migrations seed pricing rules");
        assert!(engine
            .get_record(RuntimeGetInput {
                actor: actor.clone(),
                model_code: pricing.model_code.clone(),
                record_id: first["id"].as_str().unwrap().into(),
                scope_grant: Some(grant),
            })
            .await
            .unwrap()
            .is_some());
    }

    let applications = models
        .iter()
        .find(|m| m.model_id == applications_model_id)
        .unwrap();
    let expanded = store
        .list_records(
            applications,
            RuntimeListQuery {
                scope_id: Some(workspace_b),
                owner_user_id: None,
                filter: domain::ResourceFilterExpr::All(vec![]),
                sorts: vec![],
                expand_relations: vec!["created_by".into()],
                page: 1,
                page_size: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(expanded.items[0]["id"], json!(app_b));
    assert_eq!(expanded.items[0]["created_by"], serde_json::Value::Null);

    for id in [user_a, disabled_member] {
        for payload in [json!({}), json!({"nickname":"unauthorized"})] {
            let err = store
                .update_record(
                    &users_model,
                    admin_b,
                    Some(workspace_b),
                    None,
                    &id.to_string(),
                    payload,
                )
                .await
                .unwrap_err();
            assert!(
                err.to_string().contains("runtime record not found"),
                "{err:#}"
            );
        }
        assert!(!store
            .delete_record(&users_model, Some(workspace_b), None, &id.to_string())
            .await
            .unwrap());
    }
    let updated = store
        .update_record(
            &users_model,
            admin_b,
            Some(workspace_b),
            None,
            &shared_user.to_string(),
            json!({"nickname":"authorized"}),
        )
        .await
        .unwrap();
    assert_eq!(updated["nickname"], "authorized");
    assert!(store
        .delete_record(
            &users_model,
            Some(workspace_b),
            None,
            &disposable_user.to_string()
        )
        .await
        .unwrap());
    for model in [&users_model, pricing] {
        let persisted = store
            .get_model_definition(workspace_b, model.model_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            persisted.availability_status,
            domain::MetadataAvailabilityStatus::Available
        );
    }
}

#[tokio::test]
async fn migration_normalizes_only_workspace_role_view_scopes() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let [workspace, workspace_role, system_role] = [(); 3].map(|_| Uuid::now_v7());
    insert_workspace(&store, workspace).await;
    sqlx::query(
        "insert into roles (id, scope_id, scope_kind, workspace_id, code, name) values
         ($1, $3, 'workspace', $3, 'legacy_policy', 'Legacy workspace policy'),
         ($2, $4, 'system', null, 'legacy_policy', 'Legacy system policy')",
    )
    .bind(workspace_role)
    .bind(system_role)
    .bind(workspace)
    .bind(SYSTEM_SCOPE_ID)
    .execute(store.pool())
    .await
    .unwrap();
    let model_id: Uuid = sqlx::query_scalar(
        "select id from model_definitions where code = 'application_run_log_summaries'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    for role in [workspace_role, system_role] {
        sqlx::query("insert into role_data_policies (id, role_id, default_view_scope) values ($1, $2, 'system_all')")
            .bind(Uuid::now_v7()).bind(role).execute(store.pool()).await.unwrap();
        sqlx::query("insert into role_data_model_policies (id, role_id, data_model_id, view_scope_override) values ($1, $2, $3, 'system_all')")
            .bind(Uuid::now_v7()).bind(role).bind(model_id).execute(store.pool()).await.unwrap();
    }
    sqlx::raw_sql(include_str!("../../../storage/durable/postgres/migrations/20261123020000_normalize_workspace_role_data_view_scopes.sql"))
        .execute(store.pool()).await.unwrap();

    for (role, expected) in [(workspace_role, "scope_all"), (system_role, "system_all")] {
        let (default, per_model): (String, String) = sqlx::query_as(
            "select p.default_view_scope, m.view_scope_override
             from role_data_policies p join role_data_model_policies m using (role_id)
             where p.role_id = $1 and m.data_model_id = $2",
        )
        .bind(role)
        .bind(model_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!((default.as_str(), per_model.as_str()), (expected, expected));
    }
}
