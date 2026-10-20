use control_plane::ports::{
    CreateMcpInstanceInput, CreateMcpToolBindingInput, CreateMcpToolInput,
    CreateMcpUpstreamConnectionInput, ExtensionInstallationRepository, McpManagementRepository,
    ReplaceMcpBundleGraphInput, SeedMcpBundleGraphInput, UpsertExtensionInstallationInput,
    UpsertMcpClientCredentialInput,
};

#[tokio::test]
async fn ac_001_ac_002_ac_003_builtin_bundle_is_seeded_once_and_remains_mutable() {
    let (store, workspace, actor) = seed_store().await;
    let package = managed_frontstage_package("1.0.2");
    seed_builtin_bundle_installation(&store, actor.id, "1.0.2").await;
    let input = managed_frontstage_seed_input(actor.id, workspace.id, package.clone());
    store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .unwrap();
    let instances = store.list_mcp_instances(workspace.id).await.unwrap();
    let tools = store.list_mcp_tools(workspace.id).await.unwrap();
    let bindings = store
        .list_mcp_tool_bindings(&[instances[0].id])
        .await
        .unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(tools.len(), 2);
    assert_eq!(bindings.len(), 2);
    let source = instances[0].managed_by.as_ref().unwrap();
    assert_eq!(source.organization, "1flowbase");
    assert_eq!(source.bundle_id, "frontstage_assistant");
    assert_eq!(source.bundle_version, "1.0.2");
    assert!(tools.iter().all(|tool| tool.managed_by.is_some()));

    store
        .delete_mcp_instance(workspace.id, "frontstage_browser")
        .await
        .unwrap();
    store
        .delete_mcp_tool(workspace.id, "frontstage_list_page_blocks")
        .await
        .unwrap();
    store
        .delete_mcp_tool(workspace.id, "frontstage_inspect_block_render")
        .await
        .unwrap();

    store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .unwrap();
    assert!(store
        .list_mcp_instances(workspace.id)
        .await
        .unwrap()
        .is_empty());
    assert!(store.list_mcp_tools(workspace.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn mcp_bundle_atomic_replace_restores_deleted_managed_graph() {
    let (store, workspace, actor) = seed_store().await;
    let seed =
        managed_frontstage_seed_input(actor.id, workspace.id, managed_frontstage_package("1.0.2"));
    seed_builtin_bundle_installation(&store, actor.id, "1.0.2").await;
    store
        .seed_mcp_bundle_graph_once_atomically(&seed)
        .await
        .unwrap();
    store
        .delete_mcp_instance(workspace.id, "frontstage_browser")
        .await
        .unwrap();
    store
        .delete_mcp_tool(workspace.id, "frontstage_list_page_blocks")
        .await
        .unwrap();
    store
        .delete_mcp_tool(workspace.id, "frontstage_inspect_block_render")
        .await
        .unwrap();

    store
        .replace_mcp_bundle_graph_atomically(&ReplaceMcpBundleGraphInput {
            actor_user_id: seed.actor_user_id,
            workspace_id: seed.workspace_id,
            source: seed.source,
            connections: Vec::new(),
            tools: seed.tools,
            instances: seed.instances,
        })
        .await
        .unwrap();
    let instances = store.list_mcp_instances(workspace.id).await.unwrap();
    let tools = store.list_mcp_tools(workspace.id).await.unwrap();
    assert_eq!(instances.len(), 1);
    assert_eq!(tools.len(), 2);
    assert!(instances[0].managed_by.is_some());
}

#[tokio::test]
async fn ac_005_new_builtin_bundle_version_adds_managed_tools_once() {
    let (store, workspace, actor) = seed_store().await;
    seed_builtin_bundle_installation(&store, actor.id, "1.0.2").await;
    store
        .seed_mcp_bundle_graph_once_atomically(&managed_frontstage_seed_input(
            actor.id,
            workspace.id,
            managed_frontstage_package("1.0.2"),
        ))
        .await
        .unwrap();

    seed_builtin_bundle_installation(&store, actor.id, "1.1.0").await;
    let mut upgraded = managed_frontstage_package("1.1.0");
    let mut source_tool = upgraded.tools[0].clone();
    source_tool.tool_id = "frontstage_read_block_source_fragment".into();
    source_tool.name = "read_block_source_fragment".into();
    upgraded.tools.push(source_tool);
    upgraded.instances[0]
        .bindings
        .push(domain::McpBundleToolBinding {
            group_path: "/frontstage".into(),
            tool_id: "frontstage_read_block_source_fragment".into(),
            display_alias: None,
            visible: true,
            sort_order: 2,
        });
    let upgraded_input = managed_frontstage_seed_input(actor.id, workspace.id, upgraded);
    store
        .seed_mcp_bundle_graph_once_atomically(&upgraded_input)
        .await
        .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&upgraded_input)
        .await
        .unwrap();

    let tools = store.list_mcp_tools(workspace.id).await.unwrap();
    let instances = store.list_mcp_instances(workspace.id).await.unwrap();
    let bindings = store
        .list_mcp_tool_bindings(&[instances[0].id])
        .await
        .unwrap();
    assert_eq!(tools.len(), 3);
    assert_eq!(bindings.len(), 3);
    assert_eq!(
        instances[0].managed_by.as_ref().unwrap().bundle_version,
        "1.1.0"
    );
}

async fn seed_builtin_bundle_installation(
    store: &PgControlPlaneStore,
    actor_user_id: uuid::Uuid,
    version: &str,
) {
    ExtensionInstallationRepository::upsert_extension_installation(
        store,
        &UpsertExtensionInstallationInput {
            installation_id: uuid::Uuid::now_v7(),
            identity: domain::ExtensionInstallationIdentity {
                category: domain::ExtensionCategory::Mcp,
                organization: "1flowbase".into(),
                artifact_id: "frontstage_assistant".into(),
                version: version.into(),
            },
            node_id: "test-node".into(),
            source_kind: "builtin".into(),
            trust_level: "verified_official".into(),
            local_path: "/tmp/frontstage-assistant.tar.gz".into(),
            expected_checksum: Some("sha256:test".into()),
            local_checksum: "sha256:test".into(),
            signature_status: domain::ExtensionSignatureStatus::Verified,
            signature_algorithm: Some("builtin-code-shipped".into()),
            signing_key_id: Some("1flowbase-builtin".into()),
            warnings: Vec::new(),
            receipt: serde_json::json!({"kind": "builtin"}),
            application_action: domain::ExtensionApplicationAction::ImportMcp,
            status: domain::ExtensionInstallationStatus::Installed,
            is_current: true,
            created_by: actor_user_id,
        },
    )
    .await
    .unwrap();
}
use control_plane::mcp_management::{
    CreateMcpInstanceCommand, CreateMcpToolBindingCommand, CreateMcpToolCommand, McpListItemsQuery,
    McpManagementService, McpRemoteToolDefinition, McpUpstreamCredential,
    RecordMcpUpstreamDiscoveryCommand, RefreshMcpToolDescriptionCommand,
    SaveMcpUpstreamConnectionCommand, SaveMcpUpstreamCredentialCommand,
    UpdateMcpToolBindingCommand, UpsertMcpGroupCommand,
};
use control_plane::ports::{
    CreateMemberInput, CreateWorkspaceRoleInput, MemberRepository, RoleRepository,
};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::OffsetDateTime;

#[path = "mcp_management_integration/return_defaults.rs"]
mod return_defaults;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "MCP Management")
        .await
        .unwrap();
    control_plane_test_support::upsert_permission_catalog(&store)
        .await
        .unwrap();
    control_plane_test_support::upsert_builtin_roles(&store, workspace.id)
        .await
        .unwrap();
    store
        .upsert_login_entry(&domain::LoginEntryRecord {
            id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
            connection_id: domain::PASSWORD_LOCAL_CONNECTION_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
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
        source: domain::McpInterfaceCatalogSource::StaticApi,
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

fn managed_frontstage_package(version: &str) -> domain::McpBundlePackage {
    let tool = |capability_code: &str| domain::McpBundleTool {
        max_inline_chars: None,
        response_fields: None,
        tool_id: format!("frontstage_{capability_code}"),
        name: capability_code.into(),
        short_description: capability_code.into(),
        full_description: capability_code.into(),
        execution_target: domain::McpToolExecutionTarget::AssistantClient {
            capability_code: capability_code.into(),
        },
        parameter_schema_snapshot: serde_json::json!({"type":"object"}),
        result_schema_snapshot: serde_json::json!({"type":"object"}),
        input_mapping: serde_json::json!({}),
        output_mapping: serde_json::json!({}),
        permission_code_snapshot: None,
        risk_level_snapshot: domain::McpRiskLevel::Low,
        status: domain::McpToolStatus::Enabled,
    };
    let tools = vec![tool("list_page_blocks"), tool("inspect_block_render")];
    domain::McpBundlePackage {
        manifest: domain::McpBundleManifest {
            schema_version: domain::MCP_BUNDLE_SCHEMA_VERSION.into(),
            organization: "1flowbase".into(),
            bundle_id: "frontstage_assistant".into(),
            bundle_version: version.into(),
            locale: "zh_Hans".into(),
            minimum_host_version: "0.3.6".into(),
            exported_from_system_version: "0.3.6".into(),
            exported_at: "2026-08-17T02:00:00Z".into(),
            files: Vec::new(),
        },
        instances: vec![domain::McpBundleInstance {
            instance_id: "frontstage_browser".into(),
            name: "Frontstage Browser".into(),
            description_short: Some("Managed".into()),
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/frontstage".into(),
            groups: vec![domain::McpBundleGroup {
                path: "/frontstage".into(),
                display_name: "Frontstage".into(),
                description_short: None,
                enabled: true,
                sort_order: 0,
            }],
            bindings: tools
                .iter()
                .enumerate()
                .map(|(index, tool)| domain::McpBundleToolBinding {
                    group_path: "/frontstage".into(),
                    tool_id: tool.tool_id.clone(),
                    display_alias: None,
                    visible: true,
                    sort_order: index as i32,
                })
                .collect(),
            discovery_policy: domain::McpBundleInstanceDiscoveryPolicy {
                list_default_limit: 20,
                list_max_depth: 3,
                list_regex_enabled: false,
                list_regex_max_length: 64,
                list_return_fields: serde_json::json!(["id", "name"]),
            },
        }],
        tools,
        connections: Vec::new(),
    }
}

fn managed_frontstage_seed_input(
    actor_user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    package: domain::McpBundlePackage,
) -> SeedMcpBundleGraphInput {
    let source = domain::McpManagedBundleSource {
        organization: package.manifest.organization,
        bundle_id: package.manifest.bundle_id,
        bundle_version: package.manifest.bundle_version,
    };
    let tools = package
        .tools
        .into_iter()
        .map(|tool| CreateMcpToolInput {
            max_inline_chars: None,
            response_fields: None,
            id: uuid::Uuid::now_v7(),
            actor_user_id,
            workspace_id,
            tool_id: tool.tool_id,
            name: tool.name,
            short_description: tool.short_description,
            full_description: tool.full_description,
            execution_target: tool.execution_target,
            parameter_schema: tool.parameter_schema_snapshot,
            result_schema: tool.result_schema_snapshot,
            input_mapping: tool.input_mapping,
            output_mapping: tool.output_mapping,
            permission_code: tool.permission_code_snapshot,
            risk_level: tool.risk_level_snapshot,
            des_id: uuid::Uuid::now_v7().simple().to_string()[..8].to_string(),
            des_id_required: false,
            status: tool.status,
        })
        .collect();
    SeedMcpBundleGraphInput {
        actor_user_id,
        workspace_id,
        source,
        tools,
        instances: package.instances,
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
async fn issue_1246_ac_005_ac_008_ac_009_upstream_secret_and_import_are_safe_and_idempotent() {
    let (store, _workspace, actor) = seed_store().await;
    let service = McpManagementService::new(store);
    let connection = service
        .save_upstream_connection(SaveMcpUpstreamConnectionCommand {
            actor_user_id: actor.id,
            connection_id: None,
            name: "Weather MCP".into(),
            endpoint: "https://mcp.example.com/rpc".into(),
            transport: domain::McpUpstreamTransport::StreamableHttp,
            auth_type: domain::McpUpstreamAuthType::Bearer,
            custom_header_name: None,
            status: domain::McpUpstreamConnectionStatus::Enabled,
        })
        .await
        .unwrap();
    assert!(!connection.credentials_configured);
    service
        .save_upstream_credential(SaveMcpUpstreamCredentialCommand {
            actor_user_id: actor.id,
            connection_id: connection.id,
            credential: McpUpstreamCredential::Bearer {
                token: "secret-token".into(),
            },
            master_key: "test-master-key".into(),
        })
        .await
        .unwrap();
    let listed = service.list_upstream_connections(actor.id).await.unwrap();
    assert!(listed[0].credentials_configured);

    service.record_upstream_discovery(RecordMcpUpstreamDiscoveryCommand {
        actor_user_id: actor.id,
        connection_id: connection.id,
        discovered_at: OffsetDateTime::now_utc(),
        tools: vec![McpRemoteToolDefinition {
            remote_tool_name: "weather.lookup".into(),
            description: Some("Weather".into()),
            input_schema: serde_json::json!({
                "type":"object","properties":{"city":{"type":"string"}},"required":["city"]
            }),
            output_schema: serde_json::json!({"type":"object","properties":{"temperature":{"type":"number"}}}),
            schema_hash: "schema-v1".into(),
        }],
    }).await.unwrap();
    let names = vec!["weather.lookup".to_string()];
    let first = service
        .import_upstream_tools(actor.id, connection.id, &names)
        .await
        .unwrap();
    let second = service
        .import_upstream_tools(actor.id, connection.id, &names)
        .await
        .unwrap();
    assert_eq!(first[0].id, second[0].id);
    assert_eq!(first[0].status, domain::McpToolStatus::Draft);
    assert_eq!(first[0].risk_level, domain::McpRiskLevel::High);
    assert_eq!(
        first[0].input_mapping,
        serde_json::json!({
            "mappings":[{"local_path":"city","remote_path":"city","required":true}]
        })
    );
    assert!(matches!(first[0].execution_target,
        domain::McpToolExecutionTarget::McpProxy { upstream_connection_id, ref remote_tool_name, .. }
        if upstream_connection_id == connection.id && remote_tool_name == "weather.lookup"
    ));
}

#[tokio::test]
async fn mcp_proxy_graph_records_preserve_connection_and_binding_associations() {
    let (store, workspace, actor) = seed_store().await;
    let mut graph_records = Vec::new();

    for (instance_id, tool_id) in [
        ("selected_instance", "selected_tool"),
        ("unrelated_instance", "unrelated_tool"),
    ] {
        let connection_id = uuid::Uuid::now_v7();
        let connection = store
            .create_mcp_upstream_connection(&CreateMcpUpstreamConnectionInput {
                id: connection_id,
                actor_user_id: actor.id,
                workspace_id: workspace.id,
                name: format!("{tool_id} connection"),
                endpoint: format!("https://{tool_id}.example.com/rpc"),
                transport: domain::McpUpstreamTransport::StreamableHttp,
                auth_type: domain::McpUpstreamAuthType::None,
                custom_header_name: None,
                status: domain::McpUpstreamConnectionStatus::Enabled,
            })
            .await
            .unwrap();
        let tool = store
            .create_mcp_tool(&CreateMcpToolInput {
                max_inline_chars: None,
                response_fields: None,
                id: uuid::Uuid::now_v7(),
                actor_user_id: actor.id,
                workspace_id: workspace.id,
                tool_id: tool_id.into(),
                name: tool_id.into(),
                short_description: tool_id.into(),
                full_description: tool_id.into(),
                execution_target: domain::McpToolExecutionTarget::McpProxy {
                    upstream_connection_id: connection_id,
                    remote_tool_name: format!("{tool_id}.lookup"),
                    source_schema_hash: format!("{tool_id}-schema"),
                },
                parameter_schema: serde_json::json!({"type": "object"}),
                result_schema: serde_json::json!({"type": "object"}),
                input_mapping: serde_json::json!({}),
                output_mapping: serde_json::json!({}),
                permission_code: None,
                risk_level: domain::McpRiskLevel::High,
                des_id: uuid::Uuid::now_v7().simple().to_string()[..8].to_string(),
                des_id_required: false,
                status: domain::McpToolStatus::Draft,
            })
            .await
            .unwrap();
        let instance = store
            .create_mcp_instance(&CreateMcpInstanceInput {
                id: uuid::Uuid::now_v7(),
                actor_user_id: actor.id,
                workspace_id: workspace.id,
                instance_id: instance_id.into(),
                name: instance_id.into(),
                description_short: None,
                status: domain::McpInstanceStatus::Enabled,
                default_entry_path: "/".into(),
                webmcp_exposure: domain::WebMcpExposure::Disabled,
            })
            .await
            .unwrap();
        let binding = store
            .create_mcp_tool_binding(&CreateMcpToolBindingInput {
                id: uuid::Uuid::now_v7(),
                actor_user_id: actor.id,
                instance_record_id: instance.id,
                tool_record_id: tool.id,
                group_path: "/".into(),
                display_alias: None,
                visible: true,
                sort_order: 1,
            })
            .await
            .unwrap();
        graph_records.push((connection, tool, instance, binding));
    }

    let connections = store
        .list_mcp_upstream_connections(workspace.id)
        .await
        .unwrap();
    let tools = store.list_mcp_tools(workspace.id).await.unwrap();
    assert_eq!(connections.len(), 2);
    assert_eq!(tools.len(), 2);
    for (connection, tool, instance, binding) in graph_records {
        assert!(connections.iter().any(|record| record.id == connection.id));
        assert!(matches!(
            tool.execution_target,
            domain::McpToolExecutionTarget::McpProxy {
                upstream_connection_id,
                ..
            } if upstream_connection_id == connection.id
        ));
        assert_eq!(binding.instance_record_id, instance.id);
        assert_eq!(binding.tool_record_id, tool.id);
        assert_eq!(
            store.list_mcp_tool_bindings(&[instance.id]).await.unwrap(),
            vec![binding]
        );
    }
}

#[tokio::test]
async fn mcp_bundle_graph_replace_is_atomic_and_preserves_credentials_and_other_instances() {
    let (store, workspace, actor) = seed_store().await;
    let create_instance = |id, instance_id: &str, name: &str| CreateMcpInstanceInput {
        id,
        actor_user_id: actor.id,
        workspace_id: workspace.id,
        instance_id: instance_id.into(),
        name: name.into(),
        description_short: None,
        status: domain::McpInstanceStatus::Enabled,
        default_entry_path: "/".into(),
        webmcp_exposure: domain::WebMcpExposure::Disabled,
    };
    let original = store
        .create_mcp_instance(&create_instance(
            uuid::Uuid::now_v7(),
            "replace_me",
            "Old name",
        ))
        .await
        .unwrap();
    let other = store
        .create_mcp_instance(&create_instance(uuid::Uuid::now_v7(), "keep_me", "Keep me"))
        .await
        .unwrap();
    let replacement_tool = CreateMcpToolInput {
        max_inline_chars: None,
        response_fields: None,
        id: uuid::Uuid::now_v7(),
        actor_user_id: actor.id,
        workspace_id: workspace.id,
        tool_id: "shared_runtime_profile".into(),
        name: "New tool".into(),
        short_description: "Runtime profile".into(),
        full_description: "Runtime profile".into(),
        execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
            interface_id: "runtime_profile".into(),
        },
        parameter_schema: serde_json::json!({"type": "object"}),
        result_schema: serde_json::json!({"type": "object"}),
        input_mapping: serde_json::json!({}),
        output_mapping: serde_json::json!({}),
        permission_code: None,
        risk_level: domain::McpRiskLevel::Low,
        des_id: uuid::Uuid::now_v7().simple().to_string()[..8].to_string(),
        des_id_required: false,
        status: domain::McpToolStatus::Enabled,
    };
    let existing_tool = store.create_mcp_tool(&replacement_tool).await.unwrap();
    for instance in [&original, &other] {
        store
            .create_mcp_tool_binding(&CreateMcpToolBindingInput {
                id: uuid::Uuid::now_v7(),
                actor_user_id: actor.id,
                instance_record_id: instance.id,
                tool_record_id: existing_tool.id,
                group_path: "/".into(),
                display_alias: None,
                visible: true,
                sort_order: 1,
            })
            .await
            .unwrap();
    }
    store
        .upsert_mcp_client_credential(&UpsertMcpClientCredentialInput {
            id: uuid::Uuid::now_v7(),
            user_id: actor.id,
            workspace_id: workspace.id,
            instance_record_id: original.id,
            api_key: "preserved-secret".into(),
            master_key: "test-master-key".into(),
        })
        .await
        .unwrap();

    let replacement_instance = domain::McpBundleInstance {
        instance_id: original.instance_id.clone(),
        name: "New name".into(),
        description_short: None,
        status: domain::McpInstanceStatus::Enabled,
        default_entry_path: "/".into(),
        groups: Vec::new(),
        bindings: vec![domain::McpBundleToolBinding {
            group_path: "/".into(),
            tool_id: replacement_tool.tool_id.clone(),
            display_alias: None,
            visible: true,
            sort_order: 1,
        }],
        discovery_policy: domain::McpBundleInstanceDiscoveryPolicy {
            list_default_limit: 100,
            list_max_depth: 8,
            list_regex_enabled: true,
            list_regex_max_length: 256,
            list_return_fields: serde_json::json!(["name", "description"]),
        },
    };
    let replacement = ReplaceMcpBundleGraphInput {
        actor_user_id: actor.id,
        workspace_id: workspace.id,
        source: domain::McpManagedBundleSource {
            organization: "taichuy".into(),
            bundle_id: "replace_me".into(),
            bundle_version: "1.0.0".into(),
        },
        connections: Vec::new(),
        tools: vec![replacement_tool],
        instances: vec![replacement_instance],
    };
    store
        .replace_mcp_bundle_graph_atomically(&replacement)
        .await
        .unwrap();

    let instances = store.list_mcp_instances(workspace.id).await.unwrap();
    let replaced = instances
        .iter()
        .find(|instance| instance.instance_id == original.instance_id)
        .unwrap();
    assert_eq!(replaced.id, original.id);
    assert_eq!(replaced.name, "New name");
    assert!(instances
        .iter()
        .any(|instance| instance.id == other.id && instance.name == "Keep me"));
    assert_eq!(
        store
            .get_mcp_client_credential(actor.id, workspace.id, original.id, "test-master-key",)
            .await
            .unwrap()
            .as_deref(),
        Some("preserved-secret")
    );

    let mut invalid_replacement = replacement;
    invalid_replacement.instances[0].name = "Must roll back".into();
    invalid_replacement.instances[0].bindings[0].tool_id = "missing_tool".into();
    assert!(store
        .replace_mcp_bundle_graph_atomically(&invalid_replacement)
        .await
        .is_err());
    assert_eq!(
        store
            .get_mcp_instance(workspace.id, &original.instance_id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "New name"
    );
}
#[path = "mcp_management_integration/workspace_management.rs"]
mod workspace_management;
