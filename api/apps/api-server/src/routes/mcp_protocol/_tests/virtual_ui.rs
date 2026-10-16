use super::*;

#[test]
fn tool_call_controls_use_defaults_and_reject_closed_overrides() {
    let mut catalog = catalog_with_server_bound_workspace();
    let tool = &mut catalog.tools[0];
    tool.des_id_required = false;
    tool.input_mapping["interface_parameters"] = json!([
        {"name":"des_id","required":false,"source":{"kind":"mcp_call"}},
        {"name":"max_inline_chars","required":false,"source":{"kind":"mcp_call"}},
        {"name":"response_fields","required":false,"source":{"kind":"mcp_call"}}
    ]);
    tool.input_mapping["mappings"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "interface_param":"response_fields", "mcp_param":"response_fields",
            "required":false,"source":{"kind":"mcp_call","path":"response_fields"}
        }));

    assert!(validate_tool_call_controls(&json!({}), tool, Some(tool)).is_ok());
    assert!(validate_tool_call_controls(&json!({"response_fields": []}), tool, Some(tool)).is_ok());
    assert_eq!(
        validate_tool_call_controls(&json!({"max_inline_chars": 100}), tool, Some(tool)),
        Err("Call parameter not open for this tool")
    );
    assert_eq!(
        validate_tool_call_controls(&json!({"des_id": "revision"}), tool, Some(tool)),
        Err("Call parameter not open for this tool")
    );

    tool.input_mapping["mappings"].as_array_mut().unwrap().pop();
    tool.input_mapping["mappings"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "interface_param":"des_id", "mcp_param":"des_id",
            "required":true,"source":{"kind":"mcp_call","path":"des_id"}
        }));
    assert_eq!(
        validate_tool_call_controls(&json!({"des_id": "stale"}), tool, Some(tool)),
        Err("Invalid des_id")
    );
    assert!(validate_tool_call_controls(&json!({"des_id": "revision"}), tool, Some(tool)).is_ok());
}

#[tokio::test]
async fn target_interface_unauthorized_exposes_stable_authentication_code() {
    let response = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(axum::body::Body::from(
            json!({
                "status": 401,
                "code": "not_authenticated",
                "message": "sensitive detail"
            })
            .to_string(),
        ))
        .unwrap();

    let VirtualToolOutcome::Error { data, .. } = target_interface_failure(response).await else {
        panic!("target failure should remain an MCP error");
    };
    assert_eq!(
        data,
        Some(json!({
            "category": "target_authentication",
            "http_status": 401,
            "target_code": "not_authenticated",
            "outcome": "failed",
            "retry_original": false
        }))
    );
}

#[test]
#[should_panic(expected = "portable function-name characters")]
fn assistant_mcp_meta_tool_definition_rejects_non_portable_function_name() {
    portable_meta_tool_name("mcp.list");
}

fn catalog_with_server_bound_workspace() -> domain::McpCatalogSnapshot {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    let workspace_id = uuid::Uuid::from_u128(10);
    let instance_id = uuid::Uuid::from_u128(11);
    let tool_id = uuid::Uuid::from_u128(12);
    domain::McpCatalogSnapshot {
        instances: vec![domain::McpInstanceRecord {
            id: instance_id,
            workspace_id,
            instance_id: "selected".to_string(),
            name: "Selected".to_string(),
            description_short: None,
            status: McpInstanceStatus::Enabled,
            default_entry_path: "/".to_string(),
            webmcp_exposure: domain::WebMcpExposure::Disabled,
            managed_by: None,
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        }],
        groups: vec![domain::McpGroupRecord {
            id: uuid::Uuid::from_u128(13),
            instance_record_id: instance_id,
            path: "/".to_string(),
            display_name: "Root".to_string(),
            description_short: None,
            enabled: true,
            sort_order: 0,
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        }],
        tools: vec![domain::McpToolRecord {
            max_inline_chars: None,
            response_fields: None,
            id: tool_id,
            workspace_id,
            tool_id: "lookup".to_string(),
            name: "Lookup".to_string(),
            short_description: "Lookup".to_string(),
            full_description: "Lookup".to_string(),
            execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "lookup".to_string(),
            },
            parameter_schema: json!({
                "type": "object",
                "properties": {"workspace_id": {"type": "string"}, "query": {"type": "string"}},
                "required": ["workspace_id", "query"]
            }),
            result_schema: json!({}),
            input_mapping: json!({"mappings": [
                {"interface_param":"workspace_id","source":{"kind":"server_binding","binding":"workspace_id"},"required":true},
                {"interface_param":"query","source":{"kind":"mcp_argument","path":"query"},"required":true}
            ]}),
            output_mapping: json!({"mappings": []}),
            permission_code: None,
            risk_level: domain::McpRiskLevel::Low,
            des_id: "revision".to_string(),
            des_id_required: false,
            status: McpToolStatus::Enabled,
            revision: 1,
            managed_by: None,
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        }],
        bindings: vec![domain::McpToolBindingRecord {
            id: uuid::Uuid::from_u128(14),
            instance_record_id: instance_id,
            tool_record_id: tool_id,
            group_path: "/".to_string(),
            tool_id: "lookup".to_string(),
            display_alias: None,
            visible: true,
            sort_order: 0,
            created_by: workspace_id,
            updated_by: workspace_id,
            created_at: now,
            updated_at: now,
        }],
        discovery_policies: Vec::new(),
    }
}

#[test]
fn assistant_mcp_get_uses_mapped_schema_without_server_bound_workspace() {
    let catalog = catalog_with_server_bound_workspace();
    let scope = VirtualMcpScope::selected(&catalog, &["selected".to_string()]);
    let VirtualToolOutcome::Success(result) =
        get(&catalog, &scope, &json!({"tool_id": "lookup"}), true)
    else {
        panic!("selected tool should be visible");
    };
    let schema = &result["structuredContent"]["input_schema"];
    assert!(schema["properties"].get("workspace_id").is_none());
    assert_eq!(schema["required"], json!(["query"]));
}

#[test]
fn assistant_mcp_scope_rejects_unselected_instance() {
    let catalog = catalog_with_server_bound_workspace();
    let scope = VirtualMcpScope::selected(&catalog, &[]);
    assert!(matches!(
        get(&catalog, &scope, &json!({"tool_id": "lookup"}), true),
        VirtualToolOutcome::Error { code: -32602, .. }
    ));
}

#[test]
fn assistant_mcp_binding_visibility_does_not_require_group_record() {
    let mut catalog = catalog_with_server_bound_workspace();
    catalog.groups.clear();
    let scope = VirtualMcpScope::selected(&catalog, &["selected".to_string()]);

    assert!(matches!(
        get(&catalog, &scope, &json!({"tool_id": "lookup"}), true),
        VirtualToolOutcome::Success(_)
    ));
}

#[test]
fn assistant_mcp_scope_rejects_disabled_and_ambiguous_selected_instances() {
    let mut disabled = catalog_with_server_bound_workspace();
    disabled.instances[0].status = McpInstanceStatus::Disabled;
    let scope = VirtualMcpScope::selected(&disabled, &["selected".to_string()]);
    assert!(matches!(
        get(&disabled, &scope, &json!({"tool_id": "lookup"}), true),
        VirtualToolOutcome::Error { code: -32602, .. }
    ));

    let mut ambiguous = catalog_with_server_bound_workspace();
    let mut second_instance = ambiguous.instances[0].clone();
    second_instance.id = uuid::Uuid::from_u128(21);
    second_instance.instance_id = "second".to_string();
    let mut second_group = ambiguous.groups[0].clone();
    second_group.id = uuid::Uuid::from_u128(22);
    second_group.instance_record_id = second_instance.id;
    let mut second_binding = ambiguous.bindings[0].clone();
    second_binding.id = uuid::Uuid::from_u128(23);
    second_binding.instance_record_id = second_instance.id;
    ambiguous.instances.push(second_instance);
    ambiguous.groups.push(second_group);
    ambiguous.bindings.push(second_binding);
    let scope =
        VirtualMcpScope::selected(&ambiguous, &["selected".to_string(), "second".to_string()]);
    assert!(matches!(
        get(&ambiguous, &scope, &json!({"tool_id": "lookup"}), true),
        VirtualToolOutcome::Error { code: -32602, .. }
    ));
}

#[test]
fn ac_008_assistant_client_target_is_hidden_without_a_browser_execution_port() {
    let mut catalog = catalog_with_server_bound_workspace();
    catalog.tools[0].execution_target = domain::McpToolExecutionTarget::AssistantClient {
        capability_code: "inspect_block_render".to_string(),
    };
    let scope = VirtualMcpScope::selected(&catalog, &["selected".to_string()]);

    assert!(matches!(
        get(&catalog, &scope, &json!({"tool_id": "lookup"}), false),
        VirtualToolOutcome::Error { code: -32602, .. }
    ));
    assert!(matches!(
        get(&catalog, &scope, &json!({"tool_id": "lookup"}), true),
        VirtualToolOutcome::Success(_)
    ));

    catalog.tools[0].execution_target = domain::McpToolExecutionTarget::AssistantClient {
        capability_code: "run_javascript".to_string(),
    };
    assert!(matches!(
        get(&catalog, &scope, &json!({"tool_id": "lookup"}), true),
        VirtualToolOutcome::Error { code: -32602, .. }
    ));
}

fn require_description(tool: &mut domain::McpToolRecord, target: Option<&str>) {
    tool.input_mapping["interface_parameters"] = json!([
        {"name":"des_id","required":false,"source":{"kind":"mcp_call"}},
        {"name":"max_inline_chars","required":false,"source":{"kind":"mcp_call"}},
        {"name":"response_fields","required":false,"source":{"kind":"mcp_call"}}
    ]);
    let mut mapping = json!({"interface_param":"des_id","mcp_param":"des_id","required":true,"source":{"kind":"mcp_call","path":"des_id"}});
    if let Some(id) = target {
        mapping["source"]["tool_id"] = json!(id);
    }
    tool.input_mapping["mappings"]
        .as_array_mut()
        .unwrap()
        .push(mapping);
}

#[test]
fn required_description_uses_raw_argument_and_current_revision() {
    let mut catalog = catalog_with_server_bound_workspace();
    let tool = &mut catalog.tools[0];
    require_description(tool, None);
    // The persisted flag is deliberately stale: mapping is the authority.
    tool.des_id_required = false;
    assert_eq!(
        validate_tool_call_controls(&json!({}), tool, Some(tool)),
        Err("Missing des_id")
    );
    for value in [json!(null), json!(42), json!("previous"), json!("")] {
        assert!(validate_tool_call_controls(&json!({"des_id":value}), tool, Some(tool)).is_err());
    }
    assert!(validate_tool_call_controls(&json!({"des_id":"revision"}), tool, Some(tool)).is_ok());
    for (field, value) in [
        ("default_value", json!("revision")),
        ("hidden", json!(true)),
    ] {
        let mut invalid = tool.clone();
        invalid.input_mapping["mappings"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()[field] = value;
        assert!(validate_tool_call_controls(&json!({}), &invalid, Some(&invalid)).is_err());
        assert!(validate_tool_call_controls(
            &json!({"des_id":"revision"}),
            &invalid,
            Some(&invalid)
        )
        .is_err());
    }
    tool.input_mapping["mappings"].as_array_mut().unwrap().pop();
    tool.des_id_required = true;
    assert!(validate_tool_call_controls(&json!({}), tool, Some(tool)).is_ok());
}

fn catalog_with_description_prerequisite() -> domain::McpCatalogSnapshot {
    let mut catalog = catalog_with_server_bound_workspace();
    let mut prerequisite = catalog.tools[0].clone();
    prerequisite.id = uuid::Uuid::from_u128(50);
    prerequisite.tool_id = "instructions".into();
    prerequisite.des_id = "instructions-current".into();
    // Do not recurse into the prerequisite's own unavailable dependency.
    require_description(&mut prerequisite, Some("unavailable-dependency"));
    let mut binding = catalog.bindings[0].clone();
    binding.id = uuid::Uuid::from_u128(51);
    binding.tool_record_id = prerequisite.id;
    binding.tool_id = prerequisite.tool_id.clone();
    require_description(&mut catalog.tools[0], Some("instructions"));
    catalog.tools.push(prerequisite);
    catalog.bindings.push(binding);
    catalog
}

#[test]
fn prerequisite_description_requires_its_current_token_without_disclosing_it() {
    let catalog = catalog_with_description_prerequisite();
    let scope = VirtualMcpScope::single("selected".into());
    let tool = &catalog.tools[0];
    let description = description_tool(&catalog, &scope, tool, false);
    assert_eq!(description.unwrap().tool_id, "instructions");
    for token in ["revision", "instructions-previous"] {
        assert_eq!(
            validate_tool_call_controls(&json!({"des_id":token}), tool, description),
            Err("Invalid des_id")
        );
    }
    assert_eq!(
        validate_tool_call_controls(&json!({}), tool, description),
        Err("Missing des_id")
    );
    assert!(validate_tool_call_controls(
        &json!({"des_id":"instructions-current"}),
        tool,
        description
    )
    .is_ok());
    let VirtualToolOutcome::Success(result) =
        get(&catalog, &scope, &json!({"tool_id":"lookup"}), false)
    else {
        panic!("expected get");
    };
    assert_eq!(result["structuredContent"]["des_id_required"], true);
    assert_eq!(
        result["structuredContent"]["description_tool_id"],
        "instructions"
    );
    assert_eq!(result["structuredContent"]["des_id"], "revision");
    assert!(!result.to_string().contains("instructions-current"));
    let VirtualToolOutcome::Success(result) =
        get(&catalog, &scope, &json!({"tool_id":"instructions"}), false)
    else {
        panic!("expected prerequisite get");
    };
    assert_eq!(
        result["structuredContent"]["des_id"],
        "instructions-current"
    );
}

#[test]
fn prerequisite_description_fails_closed_when_not_visible_enabled_or_in_scope() {
    for variant in 0..4 {
        let mut catalog = catalog_with_description_prerequisite();
        match variant {
            0 => catalog.bindings[1].visible = false,
            1 => catalog.tools[1].status = McpToolStatus::Disabled,
            2 => catalog.bindings[1].instance_record_id = uuid::Uuid::from_u128(999),
            _ => {
                catalog.tools.pop();
            }
        }
        let scope = VirtualMcpScope::single("selected".into());
        let tool = &catalog.tools[0];
        let description = description_tool(&catalog, &scope, tool, false);
        assert!(description.is_none());
        assert_eq!(
            validate_tool_call_controls(
                &json!({"des_id":"instructions-current"}),
                tool,
                description
            ),
            Err("Description tool not visible")
        );
    }
}

#[tokio::test]
async fn node_diagnostics_api_to_mcp_preserves_structured_configuration_error() {
    use axum::response::IntoResponse;
    use orchestration_runtime::compiler::{FlowValidationError, NodeDiagnostic};
    let response = crate::error_response::ApiError(anyhow::Error::new(FlowValidationError {
        diagnostics: vec![NodeDiagnostic {
            node_id: Some("upstream-code".into()),
            code: "invalid_selector_source".into(),
            field_path: "/bindings/name".into(),
            message: "Selector missing.name references an unknown source node".into(),
            expected: None,
        }],
    }))
    .into_response();
    let VirtualToolOutcome::Error {
        data: Some(data), ..
    } = target_interface_failure(response).await
    else {
        panic!("configuration error must stay an MCP error");
    };
    assert_eq!(data["http_status"], 400);
    assert_eq!(data["target_code"], "flow_validation_failed");
    assert_eq!(
        data["target_details"]["diagnostics"][0]["node_id"],
        "upstream-code"
    );
    assert_eq!(
        data["target_details"]["diagnostics"][0]["field_path"],
        "/bindings/name"
    );
    assert_eq!(data["retry_original"], false);
}

#[tokio::test]
async fn node_diagnostics_mcp_does_not_forward_auth_or_server_details() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        let response = Response::builder()
            .status(status)
            .body(axum::body::Body::from(
                json!({
                    "code":"test_error", "details":{"diagnostics":[{"message":"private"}]},
                })
                .to_string(),
            ))
            .unwrap();
        let VirtualToolOutcome::Error {
            data: Some(data), ..
        } = target_interface_failure(response).await
        else {
            panic!("must remain an error");
        };
        assert!(data.get("target_details").is_none());
        assert!(!data.to_string().contains("private"));
    }
}

struct MediaFixtureDispatch {
    value: Value,
    calls: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl McpInterfaceDispatchPort for MediaFixtureDispatch {
    async fn execute(
        &self,
        _headers: HeaderMap,
        _interface: domain::McpInterfaceCatalogEntry,
        body: McpDebugExecuteBody,
        _server_bound_inputs: McpServerBoundInputs,
    ) -> Result<Value, debug_execute::McpDebugExecuteError> {
        assert_eq!(
            crate::resource_import::selected_mcp_scope(),
            Some(vec!["selected".into()])
        );
        // The real debug executor treats declarations without properties as raw result projection.
        assert!(body.output_mapping.get("properties").is_none());
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(self.value.clone())
    }
}

fn media_fixture_interfaces() -> McpInterfaceCatalogSnapshot {
    McpInterfaceCatalogSnapshot::new(vec![serde_json::from_value(json!({
        "interface_id":"lookup","source":"static_api","method":"GET","path":"/fixture/image",
        "name":"Read image","short_description":"Read image","parameter_descriptors":[],
        "parameter_schema":{},"result_schema":{},"permission_code":null,"security":{},
        "risk_level":"low","bindable":true,"disabled_reason":null
    }))
    .unwrap()])
}

fn media_fixture_dependencies() -> RuntimeInternalToolInvokerDependencies {
    // GET delivery uses memory; continuation tolerates an unavailable optional durable receipt.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(10))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    RuntimeInternalToolInvokerDependencies::new(
        storage_durable_postgres::PgControlPlaneStore::new(pool),
        Arc::new(storage_ephemeral::MokaCacheStore::new(
            "mcp-media-fixture",
            1024,
        )),
        String::new(),
    )
}

#[tokio::test]
async fn interface_wrapper_media_preserves_image_under_low_text_budget_and_continues_metadata() {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(vec![91; 64_000]);
    let mut catalog = catalog_with_server_bound_workspace();
    catalog.tools[0].output_mapping = json!({"mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"},{"type":"text","text_path":"/native_text"},{"type":"audio","data_path":"/data","mime_type_path":"/audio_mime"},{"type":"resource_link","uri_path":"/uri","name_path":"/name"},{"type":"resource","uri_path":"/uri","blob_path":"/data"}],"structured_content_path":"/metadata"});
    catalog.tools[0].max_inline_chars = Some(1600);
    let metadata = json!({"path":"/tmp/image.png","byte_count":64000,"description":"metadata words ".repeat(2000)});
    let dispatch = MediaFixtureDispatch {
        value: json!({"data":data,"mime_type":"image/png","metadata":metadata,"native_text":"native text survives","audio_mime":"audio/wav","uri":"urn:fixture:media","name":"file"}),
        calls: Default::default(),
    };
    let dependencies = media_fixture_dependencies();
    let interfaces = media_fixture_interfaces();
    let actor =
        domain::ActorContext::root(uuid::Uuid::from_u128(10), uuid::Uuid::from_u128(10), "root");
    let scope = VirtualMcpScope::single("selected".into());
    let arguments = json!({"tool_id":"lookup","arguments":{"query":"image"}});
    let result = call(McpCallRequest {
        dependencies: &dependencies,
        interface_catalog: &interfaces,
        interface_dispatch: &dispatch,
        headers: &HeaderMap::new(),
        actor: &actor,
        catalog: &catalog,
        scope: &scope,
        arguments: &arguments,
        assistant_client: None,
    })
    .await
    .unwrap();
    let VirtualToolOutcome::Success(result) = result else {
        panic!("image call should succeed")
    };
    let image = result["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "image")
        .unwrap();
    assert_eq!(image["data"], data);
    assert_eq!(image["mimeType"], "image/png");
    let content = result["content"].as_array().unwrap();
    assert!(content
        .iter()
        .any(|item| item["type"] == "text" && item["text"] == "native text survives"));
    assert!(content
        .iter()
        .any(|item| item["type"] == "audio" && item["data"] == data));
    assert!(content
        .iter()
        .any(|item| item["type"] == "resource_link" && item["uri"] == "urn:fixture:media"));
    assert!(content
        .iter()
        .any(|item| item["type"] == "resource" && item["resource"]["blob"] == data));
    let structured = &result["structuredContent"];
    assert!(structured.to_string().chars().count() <= 1600);
    assert!(!structured.to_string().contains(&data));
    assert_eq!(structured["detail"]["status"], "continuation_available");
    let result_ref =
        uuid::Uuid::parse_str(structured["detail"]["result_ref"].as_str().unwrap()).unwrap();
    let page = result_delivery::read_result(
        &dependencies.result_delivery,
        &actor,
        result_ref,
        &json!({"response_fields":["/path","/byte_count"],"max_inline_chars":1600}),
    )
    .await;
    assert_eq!(
        page["structuredContent"]["entries"],
        json!([{"path":"/byte_count","value":64000},{"path":"/path","value":"/tmp/image.png"}])
    );
    assert_eq!(dispatch.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn interface_wrapper_media_invalid_results_fail_and_plain_text_remains_unchanged() {
    let dependencies = media_fixture_dependencies();
    let interfaces = media_fixture_interfaces();
    let actor =
        domain::ActorContext::root(uuid::Uuid::from_u128(10), uuid::Uuid::from_u128(10), "root");
    let scope = VirtualMcpScope::single("selected".into());
    let mut catalog = catalog_with_server_bound_workspace();
    let arguments = json!({"tool_id":"lookup","arguments":{"query":"image"}});
    for source in [
        json!({"data":"%%%","mime_type":"image/png","metadata":{}}),
        json!({"data":"YQ==","mime_type":"text/plain","metadata":{}}),
        json!({"mime_type":"image/png","metadata":{}}),
    ] {
        catalog.tools[0].output_mapping = json!({"mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"});
        let dispatch = MediaFixtureDispatch {
            value: source,
            calls: Default::default(),
        };
        let result = call(McpCallRequest {
            dependencies: &dependencies,
            interface_catalog: &interfaces,
            interface_dispatch: &dispatch,
            headers: &HeaderMap::new(),
            actor: &actor,
            catalog: &catalog,
            scope: &scope,
            arguments: &arguments,
            assistant_client: None,
        })
        .await
        .unwrap();
        assert!(matches!(
            result,
            VirtualToolOutcome::Error {
                message: "invalid_tool_configuration",
                ..
            }
        ));
    }
    let dispatch = MediaFixtureDispatch {
        value: json!({"stdout":"ordinary text"}),
        calls: Default::default(),
    };
    catalog.tools[0].output_mapping = json!({"mcp_content":[]});
    let result = call(McpCallRequest {
        dependencies: &dependencies,
        interface_catalog: &interfaces,
        interface_dispatch: &dispatch,
        headers: &HeaderMap::new(),
        actor: &actor,
        catalog: &catalog,
        scope: &scope,
        arguments: &arguments,
        assistant_client: None,
    })
    .await
    .unwrap();
    assert!(matches!(
        result,
        VirtualToolOutcome::Error {
            message: "invalid_tool_configuration",
            ..
        }
    ));
    assert_eq!(dispatch.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    catalog.tools[0].output_mapping = json!({"mappings":[]});
    let result = call(McpCallRequest {
        dependencies: &dependencies,
        interface_catalog: &interfaces,
        interface_dispatch: &dispatch,
        headers: &HeaderMap::new(),
        actor: &actor,
        catalog: &catalog,
        scope: &scope,
        arguments: &arguments,
        assistant_client: None,
    })
    .await
    .unwrap();
    let VirtualToolOutcome::Success(result) = result else {
        panic!("plain result should succeed")
    };
    assert_eq!(
        result["structuredContent"],
        json!({"stdout":"ordinary text"})
    );
    assert!(result["content"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["type"] == "text"));
}

#[test]
fn managed_mcp_get_uses_flat_body_path_query_body_fields_and_retains_existing_contract() {
    let declaration: plugin_framework::ManagedServiceManifest = serde_json::from_value(json!({
        "scope":"system","feature":{"feature_id":"ssh.settings","label":"SSH","description":"SSH settings","route_id":"ssh","path":"/settings/ssh"},
        "operations":[{"interface_id":"ssh.image","contribution_id":"ssh.image","method":"POST","path":"/api/console/managed-services/ssh/hosts/{host_id}/image",
            "summary":"Read image","description":"Read image",
            "input_schema":{"type":"object","properties":{
                "path":{"type":"object","properties":{"host_id":{"type":"string"}},"required":["host_id"]},
                "query":{"type":"object","properties":{"page":{"type":"integer","default":1}}},
                "body":{"type":"object","properties":{"path":{"type":"string"},"query":{"type":"string","default":"status"},"body":{"type":"string"},"command":{"type":"string"}},"required":["path","body","command"]}
            }},"output_schema":{},"mcp":{"name":"image.read","description":"Read remote image"}
        }]
    })).unwrap();
    let service = crate::managed_services::ManagedServiceRegistration {
        installation_id: uuid::Uuid::nil(),
        plugin_code: "ssh".into(),
        plugin_version: "1".into(),
        pages: vec![],
        declaration,
    };
    let input_schema = service.declaration.operations[0].input_schema.clone();
    let mut catalog = catalog_with_server_bound_workspace();
    let instance = catalog.instances[0].clone();
    crate::managed_service_mcp::append_service(
        &mut catalog,
        &instance,
        &service,
        &[&service.declaration.operations[0]],
        uuid::Uuid::nil(),
    )
    .unwrap();
    let scope = VirtualMcpScope::single("selected".into());
    let VirtualToolOutcome::Success(result) = get(
        &catalog,
        &scope,
        &json!({"tool_id":"plugin.ssh.image.read"}),
        false,
    ) else {
        panic!("managed image tool should be visible");
    };
    let advertised = &result["structuredContent"]["input_schema"];
    for name in ["host_id", "path", "query", "body", "command"] {
        assert_eq!(advertised["properties"][name]["type"], "string", "{name}");
    }
    assert_eq!(
        advertised["properties"]["page"],
        json!({"type":"integer","default":1})
    );
    assert_eq!(advertised["properties"]["query"]["default"], "status");
    let required = advertised["required"].as_array().unwrap();
    for name in ["host_id", "path", "body", "command"] {
        assert!(required.contains(&json!(name)));
    }
    for name in ["page", "query"] {
        assert!(!required.contains(&json!(name)));
    }
    assert_eq!(service.declaration.operations[0].input_schema, input_schema);
}

fn readable_resource_catalog() -> domain::McpCatalogSnapshot {
    let mut catalog = catalog_with_server_bound_workspace();
    catalog.tools[0].output_mapping = json!({"structured_content_path":"/metadata","resource_read":true,"mcp_content":[
        {"type":"resource_link","uri_path":"/uri","name_path":"/name","mime_type_path":"/mime"},
        {"type":"resource","uri_path":"/uri","blob_path":"/blob","mime_type_path":"/mime"}
    ]});
    catalog
}

#[tokio::test]
async fn resource_read_dispatch_reexecutes_once_and_retains_requested_uri_and_complete_blob() {
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(vec![91; 300_000]);
    let catalog = readable_resource_catalog();
    let dispatch_port = MediaFixtureDispatch {
        value: json!({"uri":"file:///tmp/file.bin","name":"file.bin","mime":"application/octet-stream","blob":data,"metadata":{"size":300000}}),
        calls: Default::default(),
    };
    let dependencies = media_fixture_dependencies();
    let interfaces = media_fixture_interfaces();
    let actor =
        domain::ActorContext::root(uuid::Uuid::from_u128(10), uuid::Uuid::from_u128(10), "root");
    let scope = VirtualMcpScope::single("selected".into());
    let first = dispatch(
        &dependencies,
        &interfaces,
        &dispatch_port,
        &HeaderMap::new(),
        &actor,
        &catalog,
        &scope,
        MCP_CALL,
        json!({"tool_id":"lookup","arguments":{"query":"file"}}),
        None,
    )
    .await
    .unwrap();
    let VirtualToolOutcome::Success(first) = first else {
        panic!("resource tool call failed")
    };
    let resource = first["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "resource")
        .unwrap();
    let uri = resource["resource"]["uri"].as_str().unwrap();
    assert!(uri.starts_with("flowbase-resource://read/"));
    assert_eq!(resource["resource"]["blob"], data);
    let link = first["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "resource_link")
        .unwrap();
    assert_eq!(link["uri"], uri);
    assert_eq!(
        dispatch_port
            .calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    let before = dispatch_port
        .calls
        .load(std::sync::atomic::Ordering::SeqCst);
    let second = dispatch(
        &dependencies,
        &interfaces,
        &dispatch_port,
        &HeaderMap::new(),
        &actor,
        &catalog,
        &scope,
        resources::READ_TOOL,
        json!({"uri":uri}),
        None,
    )
    .await
    .unwrap();
    let VirtualToolOutcome::Success(second) = second else {
        panic!("resource read failed")
    };
    assert_eq!(
        second,
        json!({"contents":[{"uri":uri,"mimeType":"application/octet-stream","blob":data}]})
    );
    assert_eq!(
        dispatch_port
            .calls
            .load(std::sync::atomic::Ordering::SeqCst),
        before + 1
    );
    let listed = dispatch(
        &dependencies,
        &interfaces,
        &dispatch_port,
        &HeaderMap::new(),
        &actor,
        &catalog,
        &scope,
        resources::LIST_TOOL,
        json!({}),
        None,
    )
    .await
    .unwrap();
    let VirtualToolOutcome::Success(listed) = listed else {
        panic!("resource list failed")
    };
    assert_eq!(listed, json!({"resources":[]}));
    assert_eq!(
        dispatch_port
            .calls
            .load(std::sync::atomic::Ordering::SeqCst),
        before + 1
    );
}

#[tokio::test]
async fn stale_or_unauthorized_resource_reads_reject_before_interface_dispatch() {
    let catalog = readable_resource_catalog();
    let actor =
        domain::ActorContext::root(uuid::Uuid::from_u128(10), uuid::Uuid::from_u128(10), "root");
    let reference = resources::ResourceReference(extension_contracts::ResourceReference {
        v: 1,
        user_id: actor.user_id,
        workspace_id: actor.current_workspace_id,
        instance_id: "selected".into(),
        tool_id: "lookup".into(),
        des_id: catalog.tools[0].des_id.clone(),
        arguments: json!({"query":"file"}),
        resource_uri: "file:///tmp/file.bin".into(),
    });
    let uri = reference.encode().unwrap();
    let dependencies = media_fixture_dependencies();
    let interfaces = media_fixture_interfaces();
    let dispatch_port = MediaFixtureDispatch {
        value: json!({}),
        calls: Default::default(),
    };
    let mut cases = Vec::new();
    let mut other_user = actor.clone();
    other_user.user_id = uuid::Uuid::from_u128(999);
    cases.push((other_user, catalog.clone(), "selected", uri.clone()));
    let mut other_workspace = actor.clone();
    other_workspace.current_workspace_id = uuid::Uuid::from_u128(999);
    cases.push((other_workspace, catalog.clone(), "selected", uri.clone()));
    cases.push((actor.clone(), catalog.clone(), "unselected", uri.clone()));
    cases.push((
        actor.clone(),
        catalog.clone(),
        "selected",
        "https://example.com/unknown".into(),
    ));
    let mut revoked = catalog.clone();
    revoked.bindings[0].visible = false;
    cases.push((actor.clone(), revoked, "selected", uri.clone()));
    let mut flag_false = catalog.clone();
    flag_false.tools[0].output_mapping["resource_read"] = json!(false);
    cases.push((actor.clone(), flag_false, "selected", uri.clone()));
    let mut changed = catalog.clone();
    changed.tools[0].des_id = "changed".into();
    cases.push((actor.clone(), changed, "selected", uri.clone()));
    let mut unknown_tool = reference.clone();
    unknown_tool.tool_id = "arbitrary-write-tool".into();
    cases.push((
        actor.clone(),
        catalog.clone(),
        "selected",
        unknown_tool.encode().unwrap(),
    ));
    let mut unknown_instance = reference.clone();
    unknown_instance.instance_id = "unknown".into();
    cases.push((
        actor.clone(),
        catalog.clone(),
        "selected",
        unknown_instance.encode().unwrap(),
    ));
    for (actor, catalog, selected, uri) in cases {
        let result = dispatch(
            &dependencies,
            &interfaces,
            &dispatch_port,
            &HeaderMap::new(),
            &actor,
            &catalog,
            &VirtualMcpScope::single(selected.into()),
            resources::READ_TOOL,
            json!({"uri":uri}),
            None,
        )
        .await
        .unwrap();
        assert!(matches!(result, VirtualToolOutcome::Error { .. }));
        assert_eq!(
            dispatch_port
                .calls
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
}

fn file_import_catalog(field_name: &str) -> domain::McpCatalogSnapshot {
    let mut catalog = catalog_with_server_bound_workspace();
    let tool = &mut catalog.tools[0];
    tool.tool_id = "file_import".to_owned();
    catalog.bindings[0].tool_id = "file_import".to_owned();
    tool.parameter_schema = json!({
        "type":"object", "properties":{"body":{"type":"object","properties":{}}}
    });
    tool.parameter_schema["properties"]["body"]["properties"][field_name] = json!({
        "type":"object",
        "properties":{
            "download_url":{"type":"string"}, "file_id":{"type":"string"},
            "mime_type":{"type":["string","null"]},
            "file_name":{"anyOf":[{"type":"string"},{"type":"null"}]}
        },
        "required":["download_url","file_id"]
    });
    tool.input_mapping = json!({
        "openai_file_params":[field_name],
        "interface_parameters":[{
            "name":field_name,"parameter_type":"json_body","field_type":"object","required":true
        }],
        "mappings":[{
            "interface_param":field_name,"mcp_param":field_name,
            "source":{"kind":"mcp_argument","path":field_name},"required":true
        }]
    });
    catalog
}

#[test]
fn mcp_call_file_params_remain_optional_and_only_meta_tools_are_advertised() {
    let tools = meta_tools(false);
    assert_eq!(tools.len(), 4);
    assert_eq!(tools[3]["name"], "mcp_call");
    assert_eq!(tools[3]["_meta"]["openai/fileParams"], json!(["file"]));
    let input = &tools[3]["inputSchema"];
    assert_eq!(input["required"], json!(["tool_id", "arguments"]));
    assert_eq!(
        input["properties"]["file"]["required"],
        json!(["download_url", "file_id"])
    );
    assert_eq!(input["properties"]["file"]["additionalProperties"], false);
    for name in ["download_url", "file_id", "mime_type", "file_name"] {
        assert_eq!(
            input["properties"]["file"]["properties"][name]["type"],
            "string"
        );
    }
    assert!(tools[..3].iter().all(|tool| tool.get("_meta").is_none()));
}

#[test]
fn mcp_call_injects_authorized_file_into_opted_in_interface_wrapper() {
    let catalog = file_import_catalog("file");
    let tool = &catalog.tools[0];
    let file = json!({
        "download_url":"https://files.example.org/test.png",
        "file_id":"file_a",
        "mime_type":"image/png",
        "file_name":"report.png"
    });
    let original = json!({
        "tool_id":"file_import",
        "des_id":"current-description",
        "arguments":{"file_table_id":"attachments"},
        "file":file,
        "response_fields":["/record/id"]
    });
    let forwarded = file_params::merge_call_file(tool, &original)
        .unwrap()
        .unwrap();
    assert_eq!(forwarded["arguments"]["file"], original["file"]);
    assert_eq!(forwarded["arguments"]["file_table_id"], "attachments");
    assert_eq!(forwarded["tool_id"], "file_import");
    assert_eq!(forwarded["des_id"], "current-description");
    assert_eq!(forwarded["response_fields"], json!(["/record/id"]));
    assert!(forwarded.get("file").is_none());
    assert!(file_params::merge_call_file(
        tool,
        &json!({"tool_id":"file_import","arguments":{"file_table_id":"attachments"}})
    )
    .unwrap()
    .is_none());
}

#[test]
fn mcp_call_file_target_can_use_another_mapped_argument_name() {
    let catalog = file_import_catalog("upload");
    let file = json!({"download_url":"https://files.example.org/test.png","file_id":"file_a"});
    let forwarded = file_params::merge_call_file(
        &catalog.tools[0],
        &json!({"tool_id":"file_import","arguments":{},"file":file}),
    )
    .unwrap()
    .unwrap();
    assert_eq!(forwarded["arguments"]["upload"], file);
    assert!(forwarded["arguments"].get("file").is_none());
}

#[test]
fn mcp_call_rejects_undeclared_conflicting_and_invalid_file_inputs() {
    let mut catalog = file_import_catalog("file");
    let file = json!({"download_url":"https://files.example.org/test.png","file_id":"file_a"});
    let incoming = json!({"tool_id":"file_import","arguments":{},"file":file});
    assert_eq!(
        file_params::merge_call_file(&catalog.tools[0], &incoming)
            .unwrap()
            .unwrap()["arguments"]["file"],
        file
    );
    catalog.tools[0]
        .input_mapping
        .as_object_mut()
        .unwrap()
        .remove("openai_file_params");
    assert_eq!(
        file_params::merge_call_file(&catalog.tools[0], &incoming),
        Err("Target tool does not accept file input")
    );
    catalog = file_import_catalog("file");
    let conflict =
        json!({"tool_id":"file_import","arguments":{"file":{"file_id":"other"}},"file":file});
    assert_eq!(
        file_params::merge_call_file(&catalog.tools[0], &conflict),
        Err("Conflicting file arguments")
    );
    let identical = json!({"tool_id":"file_import","arguments":{"file":file},"file":file});
    let merged = file_params::merge_call_file(&catalog.tools[0], &identical)
        .unwrap()
        .unwrap();
    assert_eq!(merged["arguments"]["file"], file);
    assert!(merged.get("file").is_none());
    let missing = json!({"tool_id":"file_import","arguments":{},"file":{"file_id":"file_a"}});
    assert_eq!(
        file_params::merge_call_file(&catalog.tools[0], &missing),
        Err("Invalid file parameter")
    );
    let missing_args = json!({"tool_id":"file_import","file":file});
    assert_eq!(
        file_params::merge_call_file(&catalog.tools[0], &missing_args),
        Err("Invalid arguments")
    );
    catalog.tools[0].parameter_schema["properties"]["body"]["properties"]["file"]["properties"]
        ["mime_type"] = json!({"type":"integer"});
    assert_eq!(
        file_params::merge_call_file(&catalog.tools[0], &incoming),
        Err("Target tool does not accept file input")
    );
}

struct CapturedFileArgumentDispatch {
    expected: Value,
}

#[async_trait::async_trait]
impl McpInterfaceDispatchPort for CapturedFileArgumentDispatch {
    async fn execute(
        &self,
        _headers: HeaderMap,
        _interface: domain::McpInterfaceCatalogEntry,
        body: McpDebugExecuteBody,
        _server_bound_inputs: McpServerBoundInputs,
    ) -> Result<Value, debug_execute::McpDebugExecuteError> {
        assert_eq!(body.mcp_arguments, self.expected);
        assert_eq!(body.interface_id, "lookup");
        Ok(json!({"file_received": true}))
    }
}

#[tokio::test]
async fn mcp_call_file_param_reaches_existing_interface_dispatch() {
    let catalog = file_import_catalog("file");
    let image_file = json!({
        "download_url":"https://files.example.org/generated.png",
        "file_id":"file_generated",
        "mime_type":"image/png",
        "file_name":"日报封面.png"
    });
    let expected = json!({"file":image_file,"file_table_id":"attachments"});
    let dispatch_port = CapturedFileArgumentDispatch {
        expected: expected.clone(),
    };
    let actor =
        domain::ActorContext::root(uuid::Uuid::from_u128(10), uuid::Uuid::from_u128(10), "root");
    let scope = VirtualMcpScope::single("selected".into());
    let headers = HeaderMap::new();
    let arguments = json!({
        "tool_id":"file_import",
        "arguments":{"file_table_id":"attachments"},
        "file":image_file
    });
    let result = call(McpCallRequest {
        dependencies: &media_fixture_dependencies(),
        interface_catalog: &media_fixture_interfaces(),
        interface_dispatch: &dispatch_port,
        headers: &headers,
        actor: &actor,
        catalog: &catalog,
        scope: &scope,
        arguments: &arguments,
        assistant_client: None,
    })
    .await
    .unwrap();
    assert!(matches!(result, VirtualToolOutcome::Success(_)));
}
