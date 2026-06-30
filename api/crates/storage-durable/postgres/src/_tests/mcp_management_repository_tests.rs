use control_plane::mcp_management::{
    CreateMcpInstanceCommand, CreateMcpToolBindingCommand, CreateMcpToolCommand,
    McpManagementService, RefreshMcpToolDescriptionCommand, UpdateMcpToolBindingCommand,
    UpsertMcpGroupCommand,
};
use control_plane::ports::{
    CreateMemberInput, CreateWorkspaceRoleInput, MemberRepository, RoleRepository,
};
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "MCP Management")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    (store, workspace, actor)
}

fn runtime_profile_interface() -> domain::McpInterfaceCatalogEntry {
    domain::McpInterfaceCatalogEntry {
        interface_id: "get_runtime_profile".into(),
        method: "GET".into(),
        path: "/api/console/system/runtime-profile".into(),
        name: "Get runtime profile".into(),
        short_description: "Read system runtime profile.".into(),
        parameter_descriptors: vec![domain::mcp_management::McpParameterDescriptor {
            name: "locale".into(),
            field_type: "string".into(),
            parameter_type: domain::mcp_management::McpParameterType::Url,
            description: None,
            required: false,
            schema: serde_json::json!({ "type": "string" }),
        }],
        parameter_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "object",
                    "properties": {
                        "locale": { "type": "string" }
                    },
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        }),
        result_schema: serde_json::json!({"type": "object"}),
        permission_code: None,
        security: serde_json::json!([{ "sessionCookie": [] }]),
        risk_level: domain::McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    }
}

fn des_id_required_input_mapping() -> serde_json::Value {
    serde_json::json!({
        "interface_parameters": [
            {
                "name": "des_id",
                "field_type": "string",
                "parameter_type": "json_body",
                "description": "des_id",
                "required": true
            }
        ],
        "mappings": [
            {
                "interface_param": "des_id",
                "mcp_param": "des_id",
                "description": "des_id",
                "required": true
            }
        ]
    })
}

#[tokio::test]
async fn mcp_management_catalog_read_does_not_seed_default_instance() {
    let (store, workspace, actor) = seed_store().await;
    let service = McpManagementService::new(store);

    let first = service.read_workspace_catalog(actor.id).await.unwrap();
    assert!(first.instances.is_empty());
    assert_eq!(first.meta_tool_config.workspace_id, workspace.id);

    let second = service.read_workspace_catalog(actor.id).await.unwrap();
    assert!(second.instances.is_empty());
    assert_eq!(second.meta_tool_config.workspace_id, workspace.id);
}

#[tokio::test]
async fn mcp_catalog_read_allows_view_permission_without_manage() {
    let (store, workspace, actor) = seed_store().await;
    RoleRepository::create_team_role(
        &store,
        &CreateWorkspaceRoleInput {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            code: "mcp_viewer".into(),
            name: "MCP Viewer".into(),
            introduction: "Can read MCP management catalog".into(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        },
    )
    .await
    .unwrap();
    RoleRepository::replace_role_permissions(
        &store,
        actor.id,
        workspace.id,
        "mcp_viewer",
        &["mcp_management.view.all".into()],
    )
    .await
    .unwrap();
    let viewer = store
        .create_member_with_default_role(&CreateMemberInput {
            actor_user_id: actor.id,
            workspace_id: workspace.id,
            account: "mcp-viewer".into(),
            email: "mcp-viewer@example.com".into(),
            phone: None,
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$test$test".into(),
            name: "MCP Viewer".into(),
            nickname: "MCP Viewer".into(),
            introduction: String::new(),
            email_login_enabled: true,
            phone_login_enabled: false,
        })
        .await
        .unwrap();
    MemberRepository::replace_member_roles(
        &store,
        actor.id,
        workspace.id,
        viewer.id,
        &["mcp_viewer".into()],
    )
    .await
    .unwrap();

    let service = McpManagementService::new(store);
    let snapshot = service.read_workspace_catalog(viewer.id).await.unwrap();

    assert!(snapshot.instances.is_empty());
    assert_eq!(snapshot.meta_tool_config.workspace_id, workspace.id);
}

#[tokio::test]
async fn mcp_management_refreshes_des_id_and_exports_configuration_only() {
    let (store, _workspace, actor) = seed_store().await;
    let service = McpManagementService::new(store);

    let instance = service
        .create_instance(CreateMcpInstanceCommand {
            actor_user_id: actor.id,
            instance_id: "ops".into(),
            name: "Operations".into(),
            description_short: Some("Operations tools".into()),
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
        })
        .await
        .unwrap();

    let tool = service
        .create_tool(CreateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: "restart_worker".into(),
            name: "Restart Worker".into(),
            short_description: "Restart a worker".into(),
            full_description: "Restarts a selected worker through the backend interface.".into(),
            interface_entry: runtime_profile_interface(),
            input_mapping: des_id_required_input_mapping(),
            output_mapping: serde_json::json!({}),
            des_id: None,
            status: domain::McpToolStatus::Enabled,
        })
        .await
        .unwrap();
    assert_eq!(tool.tool_id, "restart_worker");
    assert_eq!(tool.des_id.len(), 8);
    assert!(
        service
            .description_check(actor.id, &tool.tool_id, Some(&tool.des_id))
            .await
            .unwrap()
            .accepted
    );

    let refreshed = service
        .refresh_tool_description(RefreshMcpToolDescriptionCommand {
            actor_user_id: actor.id,
            tool_id: tool.tool_id.clone(),
        })
        .await
        .unwrap();
    assert_ne!(refreshed.des_id, tool.des_id);
    assert!(
        !service
            .description_check(actor.id, &tool.tool_id, Some(&tool.des_id))
            .await
            .unwrap()
            .accepted
    );

    service
        .upsert_group(UpsertMcpGroupCommand {
            actor_user_id: actor.id,
            instance_id: instance.instance_id.clone(),
            path: "/ops".into(),
            display_name: "Operations".into(),
            description_short: Some("Operational tools".into()),
            enabled: true,
            sort_order: 10,
        })
        .await
        .unwrap();
    service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: instance.instance_id,
            group_path: "/ops".into(),
            tool_id: tool.tool_id.clone(),
            display_alias: Some("Restart worker".into()),
            visible: true,
            sort_order: 10,
        })
        .await
        .unwrap();

    let export = service.export_workspace_catalog(actor.id).await.unwrap();
    assert_eq!(export.tools.len(), 1);
    assert_eq!(export.instances.len(), 1);
    assert_eq!(export.bindings.len(), 1);
    assert_eq!(export.groups.len(), 1);

    service.delete_tool(actor.id, &tool.tool_id).await.unwrap();
    let missing = service
        .description_check(actor.id, &tool.tool_id, Some(&refreshed.des_id))
        .await;
    assert!(missing.is_err());
}

#[tokio::test]
async fn mcp_tool_binding_write_scope_is_limited_to_actor_workspace() {
    let (store, workspace, actor) = seed_store().await;
    let other_workspace = store
        .upsert_workspace(workspace.tenant_id, "Other MCP Management")
        .await
        .unwrap();
    store
        .upsert_builtin_roles(other_workspace.id)
        .await
        .unwrap();
    let other_actor = store
        .upsert_root_user(
            other_workspace.id,
            "other-root",
            "other-root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Other Root",
            "Other Root",
        )
        .await
        .unwrap();
    let service = McpManagementService::new(store.clone());
    service
        .create_instance(CreateMcpInstanceCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            name: "Workspace Ops".into(),
            description_short: None,
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
        })
        .await
        .unwrap();
    let tool = service
        .create_tool(CreateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: "runtime_profile".into(),
            name: "Runtime Profile".into(),
            short_description: "Read runtime profile".into(),
            full_description: "Read the current runtime profile.".into(),
            interface_entry: runtime_profile_interface(),
            input_mapping: serde_json::json!({}),
            output_mapping: serde_json::json!({}),
            des_id: None,
            status: domain::McpToolStatus::Enabled,
        })
        .await
        .unwrap();
    service
        .upsert_group(UpsertMcpGroupCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            path: "/ops".into(),
            display_name: "Operations".into(),
            description_short: None,
            enabled: true,
            sort_order: 1,
        })
        .await
        .unwrap();
    let binding = service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            group_path: "/ops".into(),
            tool_id: tool.tool_id,
            display_alias: None,
            visible: true,
            sort_order: 1,
        })
        .await
        .unwrap();

    let other_service = McpManagementService::new(store);
    assert!(other_service
        .update_tool_binding(UpdateMcpToolBindingCommand {
            actor_user_id: other_actor.id,
            binding_id: binding.id,
            group_path: "/ops".into(),
            display_alias: Some("Cross workspace update".into()),
            visible: false,
            sort_order: 9,
        })
        .await
        .is_err());
    assert!(other_service
        .delete_tool_binding(other_actor.id, binding.id)
        .await
        .is_err());

    let catalog = service.read_workspace_catalog(actor.id).await.unwrap();
    let original_binding = catalog
        .bindings
        .iter()
        .find(|candidate| candidate.id == binding.id)
        .unwrap();
    assert!(original_binding.visible);
    assert_eq!(original_binding.display_alias, None);
}

#[tokio::test]
async fn mcp_instance_directory_rules_cover_visibility_and_directory_export() {
    let (store, _workspace, actor) = seed_store().await;
    let service = McpManagementService::new(store);
    service
        .create_instance(CreateMcpInstanceCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            name: "Workspace Ops".into(),
            description_short: None,
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
        })
        .await
        .unwrap();

    let tool = service
        .create_tool(CreateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: "runtime_profile".into(),
            name: "Runtime Profile".into(),
            short_description: "Read runtime profile".into(),
            full_description: "Read the current runtime profile.".into(),
            interface_entry: runtime_profile_interface(),
            input_mapping: serde_json::json!({}),
            output_mapping: serde_json::json!({}),
            des_id: None,
            status: domain::McpToolStatus::Enabled,
        })
        .await
        .unwrap();
    let disabled_tool = service
        .create_tool(CreateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: "disabled_runtime".into(),
            name: "Disabled Runtime".into(),
            short_description: "Disabled runtime profile".into(),
            full_description: "Disabled runtime profile should not be visible.".into(),
            interface_entry: runtime_profile_interface(),
            input_mapping: serde_json::json!({}),
            output_mapping: serde_json::json!({}),
            des_id: None,
            status: domain::McpToolStatus::Disabled,
        })
        .await
        .unwrap();

    service
        .upsert_group(UpsertMcpGroupCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            path: "/ops".into(),
            display_name: "Operations".into(),
            description_short: None,
            enabled: true,
            sort_order: 1,
        })
        .await
        .unwrap();
    service
        .upsert_group(UpsertMcpGroupCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            path: "/hidden".into(),
            display_name: "Hidden".into(),
            description_short: None,
            enabled: false,
            sort_order: 2,
        })
        .await
        .unwrap();
    service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            group_path: "/ops".into(),
            tool_id: tool.tool_id.clone(),
            display_alias: None,
            visible: true,
            sort_order: 1,
        })
        .await
        .unwrap();
    service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            group_path: "/ops".into(),
            tool_id: disabled_tool.tool_id.clone(),
            display_alias: Some("Disabled Runtime".into()),
            visible: true,
            sort_order: 3,
        })
        .await
        .unwrap();
    service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            group_path: "/ops/hidden".into(),
            tool_id: tool.tool_id.clone(),
            display_alias: Some("Invisible Runtime".into()),
            visible: false,
            sort_order: 4,
        })
        .await
        .unwrap();
    service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            group_path: "/admin".into(),
            tool_id: tool.tool_id.clone(),
            display_alias: Some("Admin Runtime".into()),
            visible: true,
            sort_order: 2,
        })
        .await
        .unwrap();

    let disabled_instance = service
        .create_instance(CreateMcpInstanceCommand {
            actor_user_id: actor.id,
            instance_id: "disabled_ops".into(),
            name: "Disabled Ops".into(),
            description_short: None,
            status: domain::McpInstanceStatus::Disabled,
            default_entry_path: "/".into(),
        })
        .await
        .unwrap();
    service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: disabled_instance.instance_id.clone(),
            group_path: "/ops".into(),
            tool_id: tool.tool_id.clone(),
            display_alias: None,
            visible: true,
            sort_order: 1,
        })
        .await
        .unwrap();

    let root_items = service
        .list_items(actor.id, Some("workspace_ops"), Some("/"), None, None)
        .await
        .unwrap();
    assert!(root_items
        .iter()
        .any(|item| item.item_kind == domain::McpListItemKind::Group && item.path == "/ops"));
    assert_eq!(
        root_items
            .iter()
            .filter(|item| item.item_kind == domain::McpListItemKind::Tool)
            .count(),
        2
    );
    assert!(!root_items.iter().any(|item| item.path == "/hidden"));
    assert!(!root_items
        .iter()
        .any(|item| item.id == disabled_tool.tool_id || item.name == "Invisible Runtime"));

    let ops_items = service
        .list_items(actor.id, Some("workspace_ops"), Some("/ops"), None, None)
        .await
        .unwrap();
    assert!(ops_items
        .iter()
        .all(|item| item.path == "/ops" || item.path.starts_with("/ops/")));
    assert!(!ops_items
        .iter()
        .any(|item| item.id == disabled_tool.tool_id || item.name == "Invisible Runtime"));
    assert!(service
        .list_items(
            actor.id,
            Some(&disabled_instance.instance_id),
            None,
            None,
            None,
        )
        .await
        .is_err());

    let directory_export = service.export_instance_directory(actor.id).await.unwrap();
    assert_eq!(directory_export.instances.len(), 2);
    assert_eq!(directory_export.bindings.len(), 5);
    assert_eq!(directory_export.groups.len(), 2);

    let full_export = service.export_workspace_catalog(actor.id).await.unwrap();
    assert_eq!(full_export.tools.len(), 2);

    service
        .delete_group(actor.id, "workspace_ops", "/ops")
        .await
        .unwrap();
    let after_group_delete = service.read_workspace_catalog(actor.id).await.unwrap();
    assert!(!after_group_delete
        .groups
        .iter()
        .any(|group| group.path == "/ops"));

    service
        .delete_instance(actor.id, &disabled_instance.instance_id)
        .await
        .unwrap();
    let after_instance_delete = service.read_workspace_catalog(actor.id).await.unwrap();
    assert!(!after_instance_delete
        .instances
        .iter()
        .any(|instance| instance.instance_id == disabled_instance.instance_id));
    assert!(!after_instance_delete
        .bindings
        .iter()
        .any(|binding| binding.instance_record_id == disabled_instance.id));

    service
        .update_instance(CreateMcpInstanceCommand {
            actor_user_id: actor.id,
            instance_id: "workspace_ops".into(),
            name: "Workspace Ops".into(),
            description_short: None,
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
        })
        .await
        .unwrap();
    assert!(service
        .list_items(actor.id, None, None, None, None)
        .await
        .is_err());
}
