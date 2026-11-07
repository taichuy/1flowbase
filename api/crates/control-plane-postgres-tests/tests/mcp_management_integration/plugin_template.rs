use super::*;
use control_plane::plugin_management::{
    prepare_managed_mcp_bundle_template, prepare_managed_mcp_template,
};
use control_plane::ports::{
    PluginRepository, UpdateMcpInstanceDiscoveryPolicyInput, UpdateMcpInstanceInput,
    UpdateMcpToolBindingInput, UpdateMcpToolInput, UpsertMcpGroupInput,
    UpsertPluginInstallationInput,
};
use serde_json::json;
use uuid::Uuid;

fn manifest_document(version: &str) -> serde_json::Value {
    // Build the intake document directly: the parsed production manifest is not a wire DTO.
    let mut value = json!({
        "manifest_version": 2,
        "vendor": "acme",
        "display_name": "Managed fixture",
        "description": "Explicit managed service package",
        "source_kind": "uploaded",
        "trust_level": "unverified",
        "consumption_kind": "runtime_extension",
        "execution_mode": "process_per_call",
        "slot_codes": [],
        "selection_mode": "assignment_then_select",
        "minimum_host_version": "0.1.0",
        "contract_version": "1flowbase.extension-bus/v1",
        "schema_version": "1flowbase.plugin.manifest/v2",
        "permissions": {"network":"none", "secrets":"none", "storage":"none", "mcp":"none", "subprocess":"deny"},
        "runtime": {"entry":"bin/fixture"},
        "managed": {"module": {"bus_version":"v1", "module_kind":"runtime"}}
    });
    value["plugin_id"] = json!("plugin_template");
    value["version"] = json!(version);
    value["publisher_namespace"] = json!("acme");
    value["binding_targets"] = json!(["system"]);
    value["runtime"]["protocol"] = json!("stdio_json_multiplex_v1");
    value["settings_pages"] = json!([]);
    let names = ["read", "image", "private"];
    value["managed"]["module"]["module_id"] = json!("plugin_template");
    value["managed"]["module"]["module_version"] = json!(version);
    value["managed"]["module"]["contributions"] = json!(names.iter().map(|name| json!({
        "contribution_id":format!("plugin_template.{name}"), "contributor_module_id":"plugin_template",
        "point_id":"1flowbase.managed-service.operation", "contract_version":"1", "mode":"append", "required_permissions":["service.execute"]
    })).collect::<Vec<_>>());
    value["managed"]["execution_bindings"] = json!(names.iter().map(|name| json!({
        "contribution_id":format!("plugin_template.{name}"), "execution_mode":"process_per_call",
        "runtime":{"protocol":"stdio_json_multiplex_v1","entry":"bin/fixture"}, "handler":name
    })).collect::<Vec<_>>());
    value["managed_service"] = json!({"scope":"system", "worker_lifetime":"actor_workspace_scope",
        "mcp_instance":{"instance_id":"plugin_template_tools","name":"Template tools","description":"Initial plugin tools"},
        "feature":{"feature_id":"plugin_template.settings","label":"Template","description":"Configure template","route_id":"plugin_template","path":"/settings/plugin_template"},
        "operations":names.iter().map(|name| {
            let mut op = json!({
                "interface_id":format!("plugin_template.{name}"), "contribution_id":format!("plugin_template.{name}"),
                "method":"POST", "path":format!("/api/console/managed-services/plugin_template/{name}"), "summary":name, "description":name,
                "input_schema":{"type":"object","properties":{"body":{"type":"object","properties":{"text":{"type":"string","default":"default text"}},"required":["text"]}}},
                "output_schema":{"type":"object","properties":{"data":{"type":"string"},"mime_type":{"type":"string"},"metadata":{"type":"object","properties":{"width":{"type":"integer"}}}}}
            });
            if *name != "private" { op["mcp"] = json!({"name":name,"description":name}); }
            if *name == "image" { op["mcp"]["output_mapping"] = json!({"mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"}); }
            op
        }).collect::<Vec<_>>()
    });
    value
}

fn manifest(version: &str) -> plugin_framework::PluginManifestV1 {
    plugin_framework::parse_plugin_manifest(&manifest_document(version).to_string()).unwrap()
}

async fn install(
    store: &PgControlPlaneStore,
    actor: Uuid,
    manifest: &plugin_framework::PluginManifestV1,
) -> Uuid {
    let id = Uuid::now_v7();
    store
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: id,
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: manifest.publisher_namespace.clone(),
            provider_code: manifest.plugin_code().unwrap().into(),
            plugin_id: manifest.versioned_plugin_id().unwrap(),
            plugin_version: manifest.version.clone(),
            contract_version: "1flowbase.extension-bus/v1".into(),
            protocol: "stdio_json_multiplex_v1".into(),
            display_name: "Template".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"managed_service": &manifest.managed_service, "managed": &manifest.managed, "binding_targets": &manifest.binding_targets}),
            is_system_reserved: false,
            actor_user_id: actor,
        })
        .await
        .unwrap()
        .id
}

async fn catalog(
    store: &PgControlPlaneStore,
    actor: Uuid,
    workspace: Uuid,
) -> domain::McpCatalogSnapshot {
    McpManagementService::new(store.clone())
        .read_catalog_for_actor(&domain::ActorContext::root(actor, workspace, "root"))
        .await
        .unwrap()
}

#[tokio::test]
async fn plugin_template_seed_is_atomic_idempotent_editable_and_preserves_upgrades_and_deletions() {
    let (store, workspace, user) = seed_store().await;
    let actor = domain::ActorContext::root(user.id, workspace.id, "root");
    let manifest = manifest("1.0.0");
    let installation = install(&store, user.id, &manifest).await;
    let input = prepare_managed_mcp_template(&actor, installation, &manifest)
        .unwrap()
        .unwrap();
    let (first, duplicate) = tokio::join!(
        store.seed_mcp_bundle_graph_once_atomically(&input),
        store.seed_mcp_bundle_graph_once_atomically(&input)
    );
    first.unwrap();
    duplicate.unwrap();
    let seeded = catalog(&store, user.id, workspace.id).await;
    assert_eq!(seeded.instances.len(), 1);
    assert_eq!(
        seeded.tools.len(),
        2,
        "only operations declaring mcp are seeded"
    );
    assert_eq!(seeded.groups.len(), 1);
    assert_eq!(seeded.bindings.len(), 2);
    assert_eq!(seeded.discovery_policies.len(), 1);
    assert_eq!(
        seeded.instances[0].managed_by.as_ref().unwrap().bundle_id,
        "plugin_template"
    );
    let read = seeded
        .tools
        .iter()
        .find(|tool| tool.tool_id.ends_with(".read"))
        .unwrap();
    assert_eq!(
        read.input_mapping["interface_parameters"][0]["name"],
        "text"
    );
    let image = seeded
        .tools
        .iter()
        .find(|tool| tool.tool_id.ends_with(".image"))
        .unwrap();
    assert_eq!(image.output_mapping["mcp_content"][0]["type"], "image");
    assert_eq!(
        image.result_schema["properties"]["width"]["type"],
        "integer"
    );
    let mut mapping = read.input_mapping.clone();
    mapping["mappings"][0]["mcp_param"] = json!("edited_text");
    mapping["mappings"][0]["source"]["path"] = json!("edited_text");
    store
        .update_mcp_tool(&UpdateMcpToolInput {
            actor_user_id: user.id,
            workspace_id: workspace.id,
            tool_id: read.tool_id.clone(),
            name: "User name".into(),
            short_description: "User description".into(),
            full_description: read.full_description.clone(),
            execution_target: read.execution_target.clone(),
            parameter_schema: read.parameter_schema.clone(),
            result_schema: read.result_schema.clone(),
            input_mapping: mapping,
            output_mapping: read.output_mapping.clone(),
            max_inline_chars: Some(1024),
            response_fields: None,
            permission_code: read.permission_code.clone(),
            risk_level: read.risk_level,
            des_id: read.des_id.clone(),
            des_id_required: false,
            status: domain::McpToolStatus::Disabled,
        })
        .await
        .unwrap();
    store
        .delete_mcp_tool(workspace.id, &image.tool_id)
        .await
        .unwrap();
    let instance = &seeded.instances[0];
    store
        .update_mcp_instance(&UpdateMcpInstanceInput {
            actor_user_id: user.id,
            workspace_id: workspace.id,
            instance_id: instance.instance_id.clone(),
            name: "User instance".into(),
            description_short: Some("User instance description".into()),
            status: domain::McpInstanceStatus::Disabled,
            default_entry_path: "/".into(),
            webmcp_exposure: domain::WebMcpExposure::Disabled,
        })
        .await
        .unwrap();
    let group = &seeded.groups[0];
    store
        .update_mcp_group(&UpsertMcpGroupInput {
            id: group.id,
            actor_user_id: user.id,
            instance_record_id: instance.id,
            path: group.path.clone(),
            display_name: "User group".into(),
            description_short: Some("User group description".into()),
            enabled: false,
            sort_order: 99,
        })
        .await
        .unwrap();
    let binding = seeded
        .bindings
        .iter()
        .find(|binding| binding.tool_record_id == read.id)
        .unwrap();
    store
        .update_mcp_tool_binding(&UpdateMcpToolBindingInput {
            actor_user_id: user.id,
            workspace_id: workspace.id,
            binding_id: binding.id,
            group_path: "/".into(),
            display_alias: Some("User alias".into()),
            visible: false,
            sort_order: 88,
        })
        .await
        .unwrap();
    store
        .update_mcp_instance_discovery_policy(&UpdateMcpInstanceDiscoveryPolicyInput {
            actor_user_id: user.id,
            workspace_id: workspace.id,
            instance_record_id: instance.id,
            list_default_limit: 3,
            list_max_depth: 1,
            list_regex_enabled: true,
            list_regex_max_length: 32,
            list_return_fields: json!(["id"]),
        })
        .await
        .unwrap();
    let edited = catalog(&store, user.id, workspace.id).await;
    let mut upgraded = manifest.clone();
    upgraded.version = "2.0.0".into();
    upgraded.managed.as_mut().unwrap().module.module_version =
        extension_contracts::extension_bus::ModuleVersion::new("2.0.0").unwrap();
    upgraded
        .managed_service
        .as_mut()
        .unwrap()
        .mcp_instance
        .as_mut()
        .unwrap()
        .name = "Upgrade must not replace user name".into();
    let installation_v2 = install(&store, user.id, &upgraded).await;
    let upgraded_input = prepare_managed_mcp_template(&actor, installation_v2, &upgraded)
        .unwrap()
        .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&upgraded_input)
        .await
        .unwrap();
    let after = catalog(&store, user.id, workspace.id).await;
    assert_eq!(after.instances, edited.instances);
    assert_eq!(after.groups, edited.groups);
    assert_eq!(after.tools, edited.tools);
    assert_eq!(after.bindings, edited.bindings);
    assert_eq!(after.discovery_policies, edited.discovery_policies);
    store
        .delete_mcp_instance(workspace.id, &instance.instance_id)
        .await
        .unwrap();
    store
        .delete_mcp_tool(workspace.id, &read.tool_id)
        .await
        .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&upgraded_input)
        .await
        .unwrap();
    let deleted = catalog(&store, user.id, workspace.id).await;
    assert!(deleted.instances.is_empty());
    assert!(deleted.tools.is_empty());
    let other = store
        .upsert_workspace(workspace.tenant_id, "Other template workspace")
        .await
        .unwrap();
    let other_actor = domain::ActorContext::root(user.id, other.id, "root");
    let other_input = prepare_managed_mcp_template(&other_actor, installation_v2, &upgraded)
        .unwrap()
        .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&other_input)
        .await
        .unwrap();
    let other_catalog = catalog(&store, user.id, other.id).await;
    assert_eq!(other_catalog.tools.len(), 2);
    assert_ne!(other_catalog.instances[0].id, instance.id);
    assert!(catalog(&store, user.id, workspace.id)
        .await
        .instances
        .is_empty());
}

#[tokio::test]
async fn plugin_template_collision_and_mid_graph_failure_roll_back_without_import_receipt() {
    let (store, workspace, user) = seed_store().await;
    let actor = domain::ActorContext::root(user.id, workspace.id, "root");
    let manifest = manifest("1.0.0");
    let installation = install(&store, user.id, &manifest).await;
    let input = prepare_managed_mcp_template(&actor, installation, &manifest)
        .unwrap()
        .unwrap();
    let collision = store.create_mcp_tool(&input.tools[1]).await.unwrap();
    assert!(store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .is_err());
    assert_eq!(
        store.list_mcp_tools(workspace.id).await.unwrap(),
        vec![collision.clone()]
    );
    assert!(store
        .list_mcp_instances(workspace.id)
        .await
        .unwrap()
        .is_empty());
    assert!(!store
        .has_mcp_extension_bundle_import(workspace.id, installation)
        .await
        .unwrap());
    store
        .delete_mcp_tool(workspace.id, &collision.tool_id)
        .await
        .unwrap();
    let mut bad_binding = input.clone();
    bad_binding.instances[0].bindings[0].tool_id = "not_seeded".into();
    assert!(store
        .seed_mcp_bundle_graph_once_atomically(&bad_binding)
        .await
        .is_err());
    assert!(store.list_mcp_tools(workspace.id).await.unwrap().is_empty());
    assert!(store
        .list_mcp_instances(workspace.id)
        .await
        .unwrap()
        .is_empty());
    assert!(!store
        .has_mcp_extension_bundle_import(workspace.id, installation)
        .await
        .unwrap());
    store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .unwrap();
    assert_eq!(store.list_mcp_tools(workspace.id).await.unwrap().len(), 2);
    let original = catalog(&store, user.id, workspace.id).await;
    let mut another_publisher = manifest.clone();
    another_publisher.publisher_namespace = "another_publisher".into();
    another_publisher.plugin_id = "foreign_template".into();
    let foreign_installation = install(&store, user.id, &another_publisher).await;
    assert_ne!(
        foreign_installation, installation,
        "collision requires distinct installations"
    );
    let foreign = prepare_managed_mcp_template(&actor, foreign_installation, &another_publisher)
        .unwrap()
        .unwrap();
    assert!(
        store
            .seed_mcp_bundle_graph_once_atomically(&foreign)
            .await
            .is_err(),
        "managed namespace collisions must fail too"
    );
    assert_eq!(catalog(&store, user.id, workspace.id).await, original);
    assert!(!store
        .has_mcp_extension_bundle_import(workspace.id, foreign_installation)
        .await
        .unwrap());
}

fn native_template_fixture(
    version: &str,
) -> (plugin_framework::PluginManifestV1, domain::McpBundlePackage) {
    let mut manifest = manifest(version);
    let actor = domain::ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
    let flat = prepare_managed_mcp_template(&actor, Uuid::now_v7(), &manifest)
        .unwrap()
        .unwrap();
    let mut instance = flat.instances.into_iter().next().unwrap();
    instance.default_entry_path = "/native".into();
    instance.groups[0].path = "/native".into();
    instance.groups[0].sort_order = 17;
    for (index, binding) in instance.bindings.iter_mut().enumerate() {
        binding.group_path = "/native".into();
        binding.sort_order = 30 - index as i32;
        binding.display_alias = Some(format!("Native alias {index}"));
    }
    let tools = flat
        .tools
        .into_iter()
        .map(|tool| domain::McpBundleTool {
            tool_id: tool.tool_id,
            name: tool.name,
            short_description: tool.short_description,
            full_description: "Native full description".into(),
            execution_target: tool.execution_target,
            parameter_schema_snapshot: tool.parameter_schema,
            result_schema_snapshot: tool.result_schema,
            input_mapping: tool.input_mapping,
            output_mapping: tool.output_mapping,
            max_inline_chars: Some(2048),
            response_fields: None,
            permission_code_snapshot: tool.permission_code,
            risk_level_snapshot: tool.risk_level,
            status: tool.status,
        })
        .collect();
    let declaration = manifest
        .managed_service
        .as_mut()
        .unwrap()
        .mcp_instance
        .as_mut()
        .unwrap();
    declaration.template = Some(plugin_framework::ManagedServiceMcpTemplate {
        path: "_assets/mcp/manifest.json".into(),
        sha256: format!("sha256:{}", "a".repeat(64)),
    });
    declaration.reset_on_activation = true;
    let package = domain::McpBundlePackage {
        manifest: domain::McpBundleManifest {
            schema_version: domain::MCP_BUNDLE_SCHEMA_VERSION.into(),
            organization: manifest.publisher_namespace.clone(),
            bundle_id: manifest.plugin_code().unwrap().into(),
            bundle_version: version.into(),
            locale: "en_US".into(),
            minimum_host_version: "0.1.0".into(),
            exported_from_system_version: "0.1.0".into(),
            exported_at: "2026-11-07T00:00:00Z".into(),
            files: vec![],
        },
        tools,
        instances: vec![instance],
        connections: vec![],
    };
    (manifest, package)
}

#[tokio::test]
async fn plugin_template_native_reset_restores_defaults_identity_and_upgrade_without_touching_neighbors(
) {
    let (store, workspace, user) = seed_store().await;
    let actor = domain::ActorContext::root(user.id, workspace.id, "root");
    let (manifest, package) = native_template_fixture("1.0.0");
    let installation = install(&store, user.id, &manifest).await;
    let input =
        prepare_managed_mcp_bundle_template(&actor, installation, &manifest, package.clone())
            .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .unwrap();
    let seeded = catalog(&store, user.id, workspace.id).await;
    let instance = &seeded.instances[0];
    let tool = &seeded.tools[0];
    store
        .upsert_mcp_client_credential(&UpsertMcpClientCredentialInput {
            id: Uuid::now_v7(),
            user_id: user.id,
            workspace_id: workspace.id,
            instance_record_id: instance.id,
            api_key: "dedicated-credential".into(),
            master_key: "test-master-key".into(),
        })
        .await
        .unwrap();
    assert_eq!(instance.default_entry_path, "/native");
    assert_eq!(seeded.groups[0].sort_order, 17);
    assert_eq!(tool.full_description, "Native full description");
    assert_eq!(tool.max_inline_chars, Some(2048));
    assert!(seeded
        .bindings
        .iter()
        .any(|binding| binding.display_alias.as_deref() == Some("Native alias 0")));
    // Neighbors include an ordinary global tool, an unrelated instance from the same bundle,
    // and a separate workspace. None is part of this dedicated reset.
    let mut neighbor_tool = input.tools[0].clone();
    neighbor_tool.id = Uuid::now_v7();
    neighbor_tool.tool_id = "legacy_global_tool".into();
    neighbor_tool.des_id = "neighbor".into();
    let neighbor_tool = store.create_mcp_tool(&neighbor_tool).await.unwrap();
    // Ordinary create preserves the same owned source without an additional bundle import.
    sqlx::query("insert into mcp_instances (id,workspace_id,instance_id,name,status,default_entry_path,managed_bundle_organization,managed_bundle_id,managed_bundle_version,created_by,updated_by) values ($1,$2,'other_instance','Other','enabled','/',$3,$4,$5,$6,$6)")
        .bind(Uuid::now_v7()).bind(workspace.id).bind(&input.source.organization)
        .bind(&input.source.bundle_id).bind(&input.source.bundle_version).bind(user.id)
        .execute(store.pool()).await.unwrap();
    let other = store
        .upsert_workspace(workspace.tenant_id, "Native reset other workspace")
        .await
        .unwrap();
    let other_actor = domain::ActorContext::root(user.id, other.id, "root");
    let other_input =
        prepare_managed_mcp_bundle_template(&other_actor, installation, &manifest, package.clone())
            .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&other_input)
        .await
        .unwrap();
    let other_before = catalog(&store, user.id, other.id).await;
    let neighbors_before = catalog(&store, user.id, workspace.id).await;
    let neighbor_instance = neighbors_before
        .instances
        .iter()
        .find(|item| item.instance_id == "other_instance")
        .unwrap()
        .clone();
    sqlx::query("update mcp_tools set name='Edited', input_mapping='{}'::jsonb, output_mapping='{}'::jsonb, status='disabled' where id=$1")
        .bind(tool.id).execute(store.pool()).await.unwrap();
    sqlx::query("update mcp_instances set name='Edited instance',default_entry_path='/',status='disabled' where id=$1")
        .bind(instance.id).execute(store.pool()).await.unwrap();
    sqlx::query("update mcp_groups set display_name='Edited group',enabled=false,sort_order=99 where instance_record_id=$1")
        .bind(instance.id).execute(store.pool()).await.unwrap();
    sqlx::query("update mcp_tool_bindings set visible=false,display_alias='Edited alias',sort_order=99 where instance_record_id=$1")
        .bind(instance.id).execute(store.pool()).await.unwrap();
    sqlx::query("update mcp_instance_discovery_policies set list_default_limit=1 where instance_record_id=$1")
        .bind(instance.id).execute(store.pool()).await.unwrap();
    let restored_input =
        prepare_managed_mcp_bundle_template(&actor, installation, &manifest, package.clone())
            .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&restored_input)
        .await
        .unwrap();
    let restored = catalog(&store, user.id, workspace.id).await;
    let restored_instance = restored
        .instances
        .iter()
        .find(|item| item.id == instance.id)
        .unwrap();
    assert_eq!(restored_instance.name, instance.name);
    assert_eq!(
        restored_instance.default_entry_path,
        instance.default_entry_path
    );
    assert_eq!(restored_instance.status, instance.status);
    for original in &seeded.tools {
        let current = restored
            .tools
            .iter()
            .find(|item| item.tool_id == original.tool_id)
            .unwrap();
        assert_eq!(current.id, original.id);
        assert_eq!(current.des_id, original.des_id);
        assert_eq!(current.name, original.name);
        assert_eq!(current.input_mapping, original.input_mapping);
        assert_eq!(current.output_mapping, original.output_mapping);
        assert_eq!(current.parameter_schema, original.parameter_schema);
        assert_eq!(current.result_schema, original.result_schema);
        assert_eq!(current.status, original.status);
    }
    assert_eq!(
        restored.groups[0].display_name,
        seeded.groups[0].display_name
    );
    assert_eq!(restored.groups[0].sort_order, 17);
    assert!(restored.bindings.iter().all(|binding| binding.visible
        && binding
            .display_alias
            .as_deref()
            .unwrap()
            .starts_with("Native alias")));
    assert_eq!(restored.discovery_policies[0].list_default_limit, 20);
    assert_eq!(
        restored
            .tools
            .iter()
            .find(|item| item.id == neighbor_tool.id)
            .unwrap(),
        &neighbor_tool
    );
    assert_eq!(
        restored
            .instances
            .iter()
            .find(|item| item.id == neighbor_instance.id)
            .unwrap(),
        &neighbor_instance
    );
    assert_eq!(catalog(&store, user.id, other.id).await, other_before);
    let (upgrade, mut upgraded_package) = native_template_fixture("2.0.0");
    upgraded_package.instances[0].name = "Version two instance".into();
    upgraded_package.tools[0].name = "Version two tool".into();
    let upgraded_installation = install(&store, user.id, &upgrade).await;
    let upgraded = prepare_managed_mcp_bundle_template(
        &actor,
        upgraded_installation,
        &upgrade,
        upgraded_package,
    )
    .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&upgraded)
        .await
        .unwrap();
    let after = catalog(&store, user.id, workspace.id).await;
    let after_instance = after
        .instances
        .iter()
        .find(|item| item.id == instance.id)
        .unwrap();
    assert_eq!(after_instance.name, "Version two instance");
    assert_eq!(
        after_instance.managed_by.as_ref().unwrap().bundle_version,
        "2.0.0"
    );
    let upgraded_tool = after
        .tools
        .iter()
        .find(|item| item.tool_id == upgraded.tools[0].tool_id)
        .unwrap();
    assert_eq!(
        upgraded_tool.id,
        seeded
            .tools
            .iter()
            .find(|item| item.tool_id == upgraded.tools[0].tool_id)
            .unwrap()
            .id
    );
    assert_eq!(upgraded_tool.name, "Version two tool");
    assert_eq!(after.instances.len(), 2);
    assert_eq!(
        store
            .get_mcp_client_credential(user.id, workspace.id, instance.id, "test-master-key")
            .await
            .unwrap()
            .as_deref(),
        Some("dedicated-credential")
    );
    assert_eq!(catalog(&store, user.id, other.id).await, other_before);
}

#[tokio::test]
async fn plugin_template_native_reset_rejects_foreign_ownership_and_rolls_back_updates() {
    let (store, workspace, user) = seed_store().await;
    let actor = domain::ActorContext::root(user.id, workspace.id, "root");
    let (manifest, package) = native_template_fixture("1.0.0");
    let installation = install(&store, user.id, &manifest).await;
    let input =
        prepare_managed_mcp_bundle_template(&actor, installation, &manifest, package).unwrap();
    let collision = store.create_mcp_tool(&input.tools[0]).await.unwrap();
    assert!(store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .is_err());
    assert_eq!(
        store.list_mcp_tools(workspace.id).await.unwrap(),
        vec![collision.clone()]
    );
    assert!(!store
        .has_mcp_extension_bundle_import(workspace.id, installation)
        .await
        .unwrap());
    sqlx::query("update mcp_tools set managed_bundle_organization=$1,managed_bundle_id=$2,managed_bundle_version=$3 where id=$4")
        .bind(&input.source.organization).bind(&input.source.bundle_id).bind(&input.source.bundle_version).bind(collision.id)
        .execute(store.pool()).await.unwrap();
    assert!(
        store
            .seed_mcp_bundle_graph_once_atomically(&input)
            .await
            .is_err(),
        "same-source metadata without a plugin receipt must not be captured"
    );
    assert!(!store
        .has_mcp_extension_bundle_import(workspace.id, installation)
        .await
        .unwrap());
    store
        .delete_mcp_tool(workspace.id, &collision.tool_id)
        .await
        .unwrap();
    store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .unwrap();
    let before = catalog(&store, user.id, workspace.id).await;
    let mut broken = input.clone();
    broken.tools[0].name = "Must roll back".into();
    broken.instances[0].bindings[0].tool_id = "missing".into();
    assert!(store
        .seed_mcp_bundle_graph_once_atomically(&broken)
        .await
        .is_err());
    assert_eq!(catalog(&store, user.id, workspace.id).await, before);
    let mut namespace = input.clone();
    namespace.tools[0].tool_id = "foreign.operation".into();
    assert!(store
        .seed_mcp_bundle_graph_once_atomically(&namespace)
        .await
        .is_err());
    let mut cross_workspace = input.clone();
    cross_workspace.tools[0].workspace_id = Uuid::now_v7();
    assert!(store
        .seed_mcp_bundle_graph_once_atomically(&cross_workspace)
        .await
        .is_err());
    let (mut foreign, _) = native_template_fixture("2.0.0");
    foreign.publisher_namespace = "foreign".into();
    let foreign_installation = install(&store, user.id, &foreign).await;
    let mut foreign_input = input.clone();
    foreign_input.kind = control_plane::ports::McpBundleSeedKind::PluginTemplateReset {
        installation_id: foreign_installation,
    };
    foreign_input.source.organization = "foreign".into();
    foreign_input.source.bundle_version = foreign.version.clone();
    let error = store
        .seed_mcp_bundle_graph_once_atomically(&foreign_input)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(&control_plane::errors::ControlPlaneError::Conflict(
            "mcp_plugin_template_seed_collision"
        )),
        "{error:#}"
    );
    assert!(!store
        .has_mcp_extension_bundle_import(workspace.id, foreign_installation)
        .await
        .unwrap());
    assert_eq!(catalog(&store, user.id, workspace.id).await, before);
}

#[test]
fn plugin_template_native_scope_validation_rejects_external_targets_instances_and_bindings() {
    let actor = domain::ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
    let (mut manifest, package) = native_template_fixture("1.0.0");
    manifest
        .managed_service
        .as_mut()
        .unwrap()
        .operations
        .iter_mut()
        .for_each(|operation| operation.mcp = None);
    assert!(
        prepare_managed_mcp_bundle_template(&actor, Uuid::now_v7(), &manifest, package.clone())
            .is_ok(),
        "native wrapper declaration does not require flat mcp declarations"
    );
    let base = serde_json::to_value(&package).unwrap();
    for (pointer, replacement) in [
        ("/manifest/organization", json!("foreign")),
        ("/manifest/bundle_id", json!("foreign")),
        ("/manifest/bundle_version", json!("2.0.0")),
        ("/instances/0/instance_id", json!("foreign")),
        ("/tools/0/tool_id", json!("foreign.tool")),
        (
            "/tools/0/execution_target",
            json!({"kind":"interface_wrapper", "interface_id":"core.roles.list"}),
        ),
        (
            "/tools/0/execution_target",
            json!({"kind":"mcp_proxy", "upstream_connection_id":Uuid::now_v7(),"remote_tool_name":"foreign","source_schema_hash":"foreign"}),
        ),
        ("/instances/0/bindings/0/tool_id", json!("foreign.tool")),
        ("/instances/0/bindings/0/group_path", json!("/missing")),
        ("/instances/0/default_entry_path", json!("/missing")),
    ] {
        let mut invalid = base.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        let invalid: domain::McpBundlePackage = serde_json::from_value(invalid).unwrap();
        assert!(
            prepare_managed_mcp_bundle_template(&actor, Uuid::now_v7(), &manifest, invalid)
                .is_err(),
            "{pointer}"
        );
    }
    let mut seed_once = manifest.clone();
    seed_once
        .managed_service
        .as_mut()
        .unwrap()
        .mcp_instance
        .as_mut()
        .unwrap()
        .reset_on_activation = false;
    assert!(
        matches!(
            prepare_managed_mcp_bundle_template(
                &actor,
                Uuid::now_v7(),
                &seed_once,
                package.clone()
            )
            .unwrap()
            .kind,
            control_plane::ports::McpBundleSeedKind::PluginTemplate { .. }
        ),
        "native template reset remains opt-in"
    );
    let mut connection = package.clone();
    connection
        .connections
        .push(domain::McpBundleUpstreamConnection {
            connection_id: Uuid::now_v7(),
            name: "Foreign upstream".into(),
            endpoint: "https://example.com/mcp".into(),
            transport: domain::McpUpstreamTransport::StreamableHttp,
            auth_type: domain::McpUpstreamAuthType::None,
            custom_header_name: None,
            status: domain::McpUpstreamConnectionStatus::Enabled,
        });
    assert!(
        prepare_managed_mcp_bundle_template(&actor, Uuid::now_v7(), &manifest, connection).is_err()
    );
    let mut extra_instance = package.clone();
    extra_instance
        .instances
        .push(extra_instance.instances[0].clone());
    assert!(
        prepare_managed_mcp_bundle_template(&actor, Uuid::now_v7(), &manifest, extra_instance)
            .is_err()
    );
    let mut duplicate_group = package.clone();
    duplicate_group.instances[0]
        .groups
        .push(package.instances[0].groups[0].clone());
    assert!(prepare_managed_mcp_bundle_template(
        &actor,
        Uuid::now_v7(),
        &manifest,
        duplicate_group
    )
    .is_err());
    let mut unbound = package;
    unbound.instances[0].bindings.pop();
    assert!(
        prepare_managed_mcp_bundle_template(&actor, Uuid::now_v7(), &manifest, unbound).is_err()
    );
}

#[path = "plugin_template/activation_assets.rs"]
mod activation_assets;
