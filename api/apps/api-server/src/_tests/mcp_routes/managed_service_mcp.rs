use super::*;
use domain::{RoleConsoleGroupPolicy, RoleConsolePolicy};
fn registration() -> ManagedServiceRegistration {
    ManagedServiceRegistration {
        installation_id: Uuid::from_u128(99), plugin_code:"ssh".into(),plugin_version:"1.0.0".into(),pages:vec![],
        declaration:serde_json::from_value(json!({
            "scope":"system", "feature":{"feature_id":"ssh.settings","label":"SSH","description":"Manage connections","route_id":"ssh","path":"/settings/ssh"},
            "operations":[{"interface_id":"ssh.execute","contribution_id":"ssh.execute","method":"POST","path":"/api/console/managed-services/ssh/hosts/{host_id}/execute",
                "summary":"Execute command","description":"Execute command on a configured host",
                "input_schema":{"type":"object","properties":{
                    "path":{"type":"object","properties":{"host_id":{"type":"string"}},"required":["host_id"]},
                    "query":{"type":"object","properties":{"page":{"type":"integer","default":1}}},
                    "body":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}}},
                "output_schema":{"type":"object","properties":{"stdout":{"type":"string"}}},
                "mcp":{"name":"hosts.execute","description":"Execute a command on an allowed host"}
            }]
        })).unwrap(),
    }
}
fn base_catalog() -> McpCatalogSnapshot {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    let id = Uuid::from_u128(1);
    McpCatalogSnapshot {
        instances: vec![McpInstanceRecord {
            id,
            workspace_id: Uuid::from_u128(2),
            instance_id: "backend".into(),
            name: "Backend".into(),
            description_short: None,
            status: McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
            webmcp_exposure: domain::WebMcpExposure::Disabled,
            managed_by: None,
            created_by: id,
            updated_by: id,
            created_at: now,
            updated_at: now,
        }],
        groups: vec![],
        tools: vec![],
        bindings: vec![],
        discovery_policies: vec![domain::McpInstanceDiscoveryPolicyRecord {
            id,
            workspace_id: Uuid::from_u128(2),
            instance_record_id: id,
            list_default_limit: 20,
            list_max_depth: 3,
            list_regex_enabled: false,
            list_regex_max_length: 32,
            list_return_fields: json!(["id", "name"]),
            created_by: id,
            updated_by: id,
            created_at: now,
            updated_at: now,
        }],
    }
}
#[test]
fn managed_mcp_projection_uses_existing_catalog_policy_and_flat_dto_mapping() {
    let mut catalog = base_catalog();
    let instance = catalog.instances[0].clone();
    let service = registration();
    append_service(
        &mut catalog,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    assert_eq!(catalog.tools.len(), 1);
    assert_eq!(catalog.tools[0].tool_id, "plugin.ssh.hosts.execute");
    assert_eq!(
        catalog.tools[0].execution_target.interface_id(),
        Some("ssh.execute")
    );
    let mappings = catalog.tools[0].input_mapping["mappings"]
        .as_array()
        .unwrap();
    assert_eq!(
        mappings
            .iter()
            .map(|m| m["mcp_param"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["host_id", "page", "command"]
    );
    assert_eq!(mappings[1]["default_value"], 1);
    assert_eq!(
        catalog.tools[0].result_schema,
        service.declaration.operations[0].output_schema
    );
    let items = control_plane::mcp_management::list_catalog_items(
        &catalog,
        "backend",
        Some("/plugins/ssh"),
        None,
        None,
        Some(1),
        None,
    )
    .unwrap();
    assert!(items
        .iter()
        .any(|item| item.id == "plugin.ssh.hosts.execute"));
    assert!(
        control_plane::mcp_management::list_catalog_items(
            &catalog,
            "backend",
            Some("/"),
            Some(".*"),
            None,
            None,
            None
        )
        .is_err(),
        "existing disabled regex policy must remain effective"
    );
    let first_id = catalog.tools[0].id;
    let mut next = base_catalog();
    let mut upgraded = service.clone();
    upgraded.plugin_version = "2.0.0".into();
    upgraded.declaration.operations[0]
        .mcp
        .as_mut()
        .unwrap()
        .description = "Updated documentation".into();
    append_service(
        &mut next,
        &instance,
        &upgraded,
        &[&upgraded.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    assert_eq!(next.tools[0].id, first_id);
    assert_ne!(next.tools[0].des_id, catalog.tools[0].des_id);
}
#[test]
fn managed_mcp_preserves_user_tools_and_rejects_reserved_collisions() {
    let service = registration();
    let mut base = base_catalog();
    let instance = base.instances[0].clone();
    let mut projected = base.clone();
    append_service(
        &mut projected,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    let mut custom = projected.tools[0].clone();
    custom.id = Uuid::from_u128(333);
    custom.tool_id = "user_tool".into();
    base.tools.push(custom.clone());
    append_service(
        &mut base,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    assert_eq!(base.tools[0], custom);
    let mut conflict = base_catalog();
    conflict.tools.push(projected.tools[0].clone());
    assert!(append_service(
        &mut conflict,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil()
    )
    .is_err());
}
#[test]
fn managed_mcp_requires_selected_enabled_host_instance_and_role_operation() {
    let mut catalog = base_catalog();
    assert!(selected_instance(&catalog, &["missing".into()]).is_none());
    catalog.instances[0].instance_id = "web_capability".into();
    catalog.instances[0].webmcp_exposure = domain::WebMcpExposure::AuthenticatedSession;
    assert!(selected_instance(&catalog, &["web_capability".into()]).is_none());
    catalog.instances[0].instance_id = "frontstage_browser".into();
    catalog.instances[0].webmcp_exposure = domain::WebMcpExposure::Disabled;
    assert!(selected_instance(&catalog, &["frontstage_browser".into()]).is_none());
    catalog.instances[0].instance_id = "backend".into();
    catalog.instances[0].status = McpInstanceStatus::Disabled;
    assert!(selected_instance(&catalog, &["backend".into()]).is_none());
    let service = registration();
    let operation = &service.declaration.operations[0];
    let mut actor = domain::ActorContext::root(Uuid::nil(), Uuid::nil(), "root");
    actor.is_root = false;
    assert!(!operation_accessible(&actor, &[], &service, operation).unwrap());
    let group = domain::ConsolePolicyGroup::settings_feature("ssh.settings").unwrap();
    let policies = vec![RoleConsolePolicy::new(
        Uuid::nil(),
        vec![RoleConsoleGroupPolicy::full(group.clone())],
    )];
    assert!(operation_accessible(&actor, &policies, &service, operation).unwrap());
    let policies = vec![RoleConsolePolicy::new(
        Uuid::nil(),
        vec![RoleConsoleGroupPolicy::disabled(group)],
    )];
    assert!(!operation_accessible(&actor, &policies, &service, operation).unwrap());
}
#[test]
fn managed_mcp_hides_disabled_or_replaced_installation() {
    let service = registration();
    let mut current = installation(&service);
    assert!(current_matches(&service, &current));
    current.desired_state = domain::PluginDesiredState::Disabled;
    assert!(!current_matches(&service, &current));
    current.desired_state = domain::PluginDesiredState::ActiveRequested;
    current.plugin_version = "2.0.0".into();
    assert!(!current_matches(&service, &current));
}

#[tokio::test]
async fn managed_mcp_flat_arguments_reach_existing_http_dispatch_and_unwrap_api_data() {
    use crate::routes::mcp_management::{debug_execute, McpDebugExecuteBody, McpDebugResponseMode};
    use axum::{
        extract::{Path, Query},
        routing::post,
        Json, Router,
    };
    let service = registration();
    let operation = &service.declaration.operations[0];
    let router = Router::new().route(
        &operation.path,
        post(
            |Path(host): Path<String>,
             Query(query): Query<std::collections::HashMap<String, String>>,
             Json(body): Json<Value>| async move {
                assert_eq!(host, "host-fixture");
                assert_eq!(query.get("page").map(String::as_str), Some("1"));
                assert_eq!(body, json!({"command":"echo fixture"}));
                Json(json!({"data":{"stdout":"fixture"},"meta":{}}))
            },
        ),
    );
    let interface = domain::McpInterfaceCatalogEntry {
        interface_id: operation.interface_id.clone(),
        source: domain::McpInterfaceCatalogSource::StaticApi,
        method: operation.method.clone(),
        path: operation.path.clone(),
        name: "Execute".into(),
        short_description: "Execute".into(),
        parameter_descriptors: vec![],
        parameter_schema: operation.input_schema.clone(),
        result_schema: operation.output_schema.clone(),
        permission_code: None,
        security: json!({}),
        risk_level: McpRiskLevel::High,
        bindable: true,
        disabled_reason: None,
    };
    let mut edited_mapping = input_mapping(&operation.input_schema);
    let host_mapping = edited_mapping["mappings"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|mapping| mapping["interface_param"] == "host_id")
        .unwrap();
    host_mapping["mcp_param"] = json!("selected_host");
    host_mapping["source"]["path"] = json!("selected_host");
    let result = debug_execute::execute_with_console_router(
        router,
        axum::http::HeaderMap::new(),
        interface,
        McpDebugExecuteBody {
            interface_id: operation.interface_id.clone(),
            debug_response_mode: McpDebugResponseMode::ToolResult,
            mcp_arguments: json!({"selected_host":"host-fixture","command":"echo fixture"}),
            input_mapping: edited_mapping,
            output_mapping: operation.output_schema.clone(),
        },
        debug_execute::McpServerBoundInputs {
            workspace_id: Uuid::nil(),
        },
    )
    .await;
    let value = match result {
        Ok(value) => value,
        Err(debug_execute::McpDebugExecuteError::Api(error)) => {
            panic!("managed MCP dispatch failed: {error:#}")
        }
        Err(debug_execute::McpDebugExecuteError::TargetResponse(response)) => {
            panic!("managed MCP target returned {}", response.status())
        }
    };
    assert_eq!(value, json!({"stdout":"fixture"}));
}

#[test]
fn managed_media_projection_preserves_mapping_and_advertises_metadata_schema() {
    let mut service = registration();
    let mapping = json!({"mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"});
    let metadata_schema = json!({"type":"object","properties":{"path":{"type":"string"},"byte_count":{"type":"integer"}}});
    let operation = &mut service.declaration.operations[0];
    operation.output_schema = json!({"type":"object","properties":{"data":{"type":"string"},"mime_type":{"type":"string"},"metadata":metadata_schema}});
    operation.mcp.as_mut().unwrap().output_mapping = Some(mapping.clone());
    let mut catalog = base_catalog();
    let instance = catalog.instances[0].clone();
    append_service(
        &mut catalog,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    assert_eq!(catalog.tools[0].output_mapping, mapping);
    assert_eq!(catalog.tools[0].result_schema, metadata_schema);
    assert!(catalog.tools[0].result_schema["properties"]
        .get("data")
        .is_none());
}

#[test]
fn managed_mcp_parameter_schema_flattens_dto_fields_named_like_transport_containers() {
    let mut service = registration();
    let operation = &mut service.declaration.operations[0];
    operation.input_schema["properties"]["body"]["properties"]["path"] =
        json!({"type":"string","description":"Absolute remote image path"});
    operation.input_schema["properties"]["body"]["properties"]["query"] =
        json!({"type":"string","default":"status"});
    operation.input_schema["properties"]["body"]["properties"]["body"] = json!({"type":"string"});
    operation.input_schema["properties"]["body"]["required"] = json!(["command", "path", "body"]);
    let business_schema = operation.input_schema.clone();
    let mut catalog = base_catalog();
    let instance = catalog.instances[0].clone();
    append_service(
        &mut catalog,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    let tool = &catalog.tools[0];
    for name in ["host_id", "command", "path", "query", "body"] {
        assert_eq!(
            tool.parameter_schema["properties"][name]["type"], "string",
            "{name}"
        );
    }
    assert_eq!(
        tool.parameter_schema["properties"]["page"],
        json!({"type":"integer","default":1})
    );
    assert_eq!(
        tool.parameter_schema["properties"]["query"]["default"],
        "status"
    );
    assert_eq!(
        tool.parameter_schema["properties"]["path"]["description"],
        "Absolute remote image path"
    );
    assert_eq!(
        tool.parameter_schema["required"],
        json!(["host_id", "command", "path", "body"])
    );
    assert_eq!(
        service.declaration.operations[0].input_schema,
        business_schema
    );
    let mappings = tool.input_mapping["mappings"].as_array().unwrap();
    for name in ["host_id", "command", "path", "query", "body", "page"] {
        let mapping = mappings
            .iter()
            .find(|mapping| mapping["interface_param"] == name)
            .unwrap();
        assert_eq!(mapping["mcp_param"], name);
        assert_eq!(mapping["source"]["path"], name);
    }
    assert_eq!(
        mappings
            .iter()
            .find(|mapping| mapping["interface_param"] == "query")
            .unwrap()["default_value"],
        "status"
    );
}

fn installation(service: &ManagedServiceRegistration) -> domain::PluginInstallationRecord {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    domain::PluginInstallationRecord {
        id: service.installation_id,
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "acme".into(),
        provider_code: service.plugin_code.clone(),
        plugin_id: "ssh@1.0.0".into(),
        plugin_version: service.plugin_version.clone(),
        contract_version: "1flowbase.extension-bus/v1".into(),
        protocol: "stdio_json_multiplex_v1".into(),
        display_name: "SSH".into(),
        source_kind: "uploaded".into(),
        trust_level: "unverified".into(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: domain::PluginDesiredState::ActiveRequested,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: json!({"managed_service":service.declaration}),
        is_system_reserved: false,
        created_by: Uuid::nil(),
        updated_by: None,
        created_at: now,
        updated_at: now,
    }
}

fn interface_entry(service: &ManagedServiceRegistration) -> domain::McpInterfaceCatalogEntry {
    let operation = &service.declaration.operations[0];
    domain::McpInterfaceCatalogEntry {
        interface_id: operation.interface_id.clone(),
        source: domain::McpInterfaceCatalogSource::StaticApi,
        method: operation.method.clone(),
        path: operation.path.clone(),
        name: operation.summary.clone(),
        short_description: operation.description.clone(),
        parameter_descriptors: vec![],
        parameter_schema: operation.input_schema.clone(),
        result_schema: operation.output_schema.clone(),
        permission_code: None,
        security: json!({}),
        risk_level: McpRiskLevel::High,
        bindable: true,
        disabled_reason: None,
    }
}

#[test]
fn dedicated_templates_use_persisted_graph_and_never_synthetic_append() {
    let mut service = registration();
    service.declaration.mcp_instance = Some(plugin_framework::ManagedServiceMcpInstance {
        instance_id: "ssh_tools".into(),
        name: "SSH tools".into(),
        description: "SSH tools".into(),
    });
    let mut catalog = base_catalog();
    let instance = catalog.instances[0].clone();
    append_service(
        &mut catalog,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    assert!(
        catalog.tools.is_empty(),
        "declared templates are never appended to any selected instance"
    );
    let mut installed = installation(&service);
    installed.desired_state = domain::PluginDesiredState::Disabled;
    let mut dedicated = instance.clone();
    dedicated.instance_id = "ssh_tools".into();
    assert!(is_template_instance(&dedicated, &[installed.clone()]));
    dedicated.instance_id = "user_renamed_instance".into();
    dedicated.managed_by = Some(domain::McpManagedBundleSource {
        organization: installed.organization.clone(),
        bundle_id: service.plugin_code.clone(),
        bundle_version: "0.9.0".into(),
    });
    assert!(
        is_template_instance(&dedicated, &[installed.clone()]),
        "user edits and a disabled source do not admit other plugins' synthetic tools"
    );
    assert!(!is_template_instance(&instance, &[installed]));
}

#[test]
fn persisted_managed_wrappers_keep_edits_but_follow_current_activation_and_role() {
    let mut catalog = base_catalog();
    let instance = catalog.instances[0].clone();
    let mut service = registration();
    append_service(
        &mut catalog,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        Uuid::nil(),
    )
    .unwrap();
    catalog.tools[0].name = "User-edited tool".into();
    catalog.tools[0].input_mapping["mappings"][0]["mcp_param"] = json!("user_host");
    let edited = catalog.clone();
    service.declaration.mcp_instance = Some(plugin_framework::ManagedServiceMcpInstance {
        instance_id: "ssh_tools".into(),
        name: "SSH".into(),
        description: "SSH".into(),
    });
    let installed = installation(&service);
    let interfaces =
        crate::routes::mcp_protocol::virtual_ui::McpInterfaceCatalogSnapshot::new(vec![
            interface_entry(&service),
        ]);
    let mut actor = domain::ActorContext::root(Uuid::nil(), instance.workspace_id, "root");
    filter_persisted_operations(
        &mut catalog,
        std::slice::from_ref(&installed),
        &[&service],
        &actor,
        &[],
        &interfaces,
    )
    .unwrap();
    assert_eq!(
        catalog, edited,
        "discovery does not regenerate or overwrite persisted mappings"
    );
    actor.is_root = false;
    let mut denied = edited.clone();
    filter_persisted_operations(
        &mut denied,
        std::slice::from_ref(&installed),
        &[&service],
        &actor,
        &[],
        &interfaces,
    )
    .unwrap();
    assert!(denied.tools.is_empty());
    assert!(denied.bindings.is_empty());
    let group = domain::ConsolePolicyGroup::settings_feature("ssh.settings").unwrap();
    let policies = vec![RoleConsolePolicy::new(
        Uuid::nil(),
        vec![RoleConsoleGroupPolicy::full(group)],
    )];
    let mut granted = edited.clone();
    filter_persisted_operations(
        &mut granted,
        std::slice::from_ref(&installed),
        &[&service],
        &actor,
        &policies,
        &interfaces,
    )
    .unwrap();
    assert_eq!(granted, edited);
    let mut disabled = edited.clone();
    filter_persisted_operations(
        &mut disabled,
        std::slice::from_ref(&installed),
        &[],
        &actor,
        &policies,
        &interfaces,
    )
    .unwrap();
    assert!(disabled.tools.is_empty());
    let mut unbound = edited;
    let no_interfaces =
        crate::routes::mcp_protocol::virtual_ui::McpInterfaceCatalogSnapshot::new(vec![]);
    filter_persisted_operations(
        &mut unbound,
        &[installed],
        &[&service],
        &actor,
        &policies,
        &no_interfaces,
    )
    .unwrap();
    assert!(unbound.tools.is_empty());
}
