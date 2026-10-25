use super::*;
use control_plane::plugin_management::prepare_managed_mcp_template;
use control_plane::ports::{
    PluginRepository, UpdateMcpInstanceDiscoveryPolicyInput, UpdateMcpInstanceInput,
    UpdateMcpToolBindingInput, UpdateMcpToolInput, UpsertMcpGroupInput,
    UpsertPluginInstallationInput,
};
use serde_json::json;
use uuid::Uuid;

fn manifest(version: &str) -> plugin_framework::PluginManifestV1 {
    let base = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../extension-package-runtime/src/_tests/managed_manifest.yaml"
    ))
    .unwrap();
    let mut value = serde_json::to_value(base).unwrap();
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
    plugin_framework::parse_plugin_manifest(&value.to_string()).unwrap()
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
            metadata_json: serde_json::to_value(manifest).unwrap(),
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
    let foreign_installation = install(&store, user.id, &another_publisher).await;
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
