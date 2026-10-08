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
