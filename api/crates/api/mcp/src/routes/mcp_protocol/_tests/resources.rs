use super::*;

fn reference() -> ResourceReference {
    ResourceReference(extension_contracts::ResourceReference {
        v: 1,
        user_id: uuid::Uuid::from_u128(10),
        workspace_id: uuid::Uuid::from_u128(20),
        instance_id: "selected".into(),
        tool_id: "plugin.ssh.read_resource".into(),
        des_id: "current-revision".into(),
        arguments: json!({"host_id":"host","path":"/tmp/file.bin"}),
        resource_uri: "file:///tmp/file.bin".into(),
    })
}

#[test]
fn resource_reference_roundtrips_without_binary_and_denies_unknown_payload_fields() {
    let original = reference();
    let uri = original.encode().unwrap();
    assert!(uri.starts_with(PREFIX));
    let decoded = ResourceReference::decode(&uri).unwrap();
    assert_eq!(decoded.arguments, original.arguments);
    assert_eq!(decoded.resource_uri, original.resource_uri);
    let mut payload = serde_json::to_value(&original).unwrap();
    assert!(payload.get("blob").is_none());
    assert!(payload.get("headers").is_none());
    payload["secret"] = json!("not permitted");
    let unknown = format!(
        "{PREFIX}{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap())
    );
    assert!(ResourceReference::decode(&unknown).is_err());
    payload.as_object_mut().unwrap().remove("secret");
    payload["v"] = json!(2);
    let future = format!(
        "{PREFIX}{}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap())
    );
    assert!(ResourceReference::decode(&future).is_err());
    for invalid in [
        "https://example.com/file",
        "file:///tmp/file",
        "flowbase-resource://other/a",
        "flowbase-resource://read/%%",
        "flowbase-resource://read/e30",
    ] {
        assert!(ResourceReference::decode(invalid).is_err());
    }
}

#[test]
fn references_follow_actual_tool_arguments_transport_budget() {
    let mut oversized = reference();
    oversized.arguments = json!({"path":"x".repeat(2 * 1024 * 1024)});
    assert!(oversized.encode().is_err());
    assert!(
        ResourceReference::decode(&format!("{PREFIX}{}", "a".repeat(2 * 1024 * 1024))).is_err()
    );
    let mut too_large_after_encoding = reference();
    too_large_after_encoding.arguments = json!({"path":"x".repeat(1_600_000)});
    assert!(McpToolArguments::from_protocol(too_large_after_encoding.arguments.clone()).is_ok());
    assert!(too_large_after_encoding.encode().is_err());
}

#[test]
fn resources_rpc_projects_through_tool_call_and_preserves_standard_result_shapes() {
    assert_eq!(capabilities(), json!({"tools":{},"resources":{}}));
    let params = json!({"uri":"urn:test:a","_meta":{"ignored":true}});
    let (name, arguments) = super::super::protocol_call("resources/read", &params).unwrap();
    assert_eq!(name, READ_TOOL);
    assert_eq!(arguments, json!({"uri":"urn:test:a"}));
    let list_params = json!({});
    let (name, arguments) = super::super::protocol_call("resources/list", &list_params).unwrap();
    assert_eq!(name, LIST_TOOL);
    assert_eq!(arguments, json!({}));
    let params = json!({"cursor":"next-page"});
    let (name, arguments) =
        super::super::protocol_call("resources/templates/list", &params).unwrap();
    assert_eq!(name, TEMPLATES_TOOL);
    assert_eq!(arguments, params);
    assert!(super::super::protocol_call("resources/list", &json!({"cursor":42})).is_err());
    assert!(super::super::protocol_call("resources/read", &json!({"uri":1})).is_err());
    let result = json!({"content":[
        {"type":"resource","resource":{"uri":"urn:test:a","mimeType":"application/octet-stream","blob":"YQ=="}},
        {"type":"resource","resource":{"uri":"urn:test:b","text":"other"}},
        {"type":"resource_link","uri":"urn:test:a","name":"file"}
    ]});
    assert_eq!(
        read_result(&result, "urn:test:a", "flowbase-resource://read/reference").unwrap(),
        json!({"contents":[{"uri":"flowbase-resource://read/reference","mimeType":"application/octet-stream","blob":"YQ=="}]})
    );
    assert!(read_result(&result, "urn:test:missing", "requested").is_err());
    assert!(read_result(
        &json!({"isError":true,"content":result["content"]}),
        "urn:test:a",
        "requested"
    )
    .is_err());
}

#[test]
fn host_reference_is_a_valid_uri_and_preserves_final_binary_budget() {
    use base64::engine::general_purpose::STANDARD;
    let uri = reference().encode().unwrap();
    assert!(media::valid_uri(&uri));
    let blob = STANDARD.encode(vec![73; 524_288]);
    for result in [
        json!({"content":[{"type":"resource","resource":{"uri":uri,"mimeType":"application/octet-stream","blob":blob}}]}),
        json!({"contents":[{"uri":uri,"mimeType":"application/octet-stream","blob":blob}]}),
    ] {
        let original = result.clone();
        let (_, response) = super::super::bounded_jsonrpc_response(
            axum::http::StatusCode::OK,
            super::super::JsonRpcResponse {
                jsonrpc: "2.0",
                id: Some(json!(1)),
                result: Some(result),
                error: None,
            },
        );
        assert!(response.0.error.is_none());
        assert_eq!(response.0.result.unwrap(), original);
    }
}

pub(super) fn catalog_with_server_bound_workspace() -> domain::McpCatalogSnapshot {
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
            status: domain::McpInstanceStatus::Enabled,
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
            status: domain::McpToolStatus::Enabled,
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
fn resource_import_rechecks_selected_scope_identity_currentness_visibility_and_opt_in() {
    let mut catalog = catalog_with_server_bound_workspace();
    catalog.tools[0].output_mapping = json!({"structured_content_path":"/metadata", "resource_read":true,"mcp_content":[{"type":"resource","uri_path":"/uri","blob_path":"/blob"}]});
    let actor = domain::ActorContext::root(
        uuid::Uuid::from_u128(1),
        catalog.instances[0].workspace_id,
        "root",
    );
    let mut locator = extension_contracts::ResourceReference {
        v: 1,
        user_id: actor.user_id,
        workspace_id: actor.current_workspace_id,
        instance_id: "selected".into(),
        tool_id: "lookup".into(),
        des_id: "revision".into(),
        arguments: json!({}),
        resource_uri: "file:///a.bin".into(),
    };
    let scope = virtual_ui::VirtualMcpScope::from_instance_ids(vec!["selected".into()]);
    assert!(authorize_reference(&locator, &actor, &catalog, &scope).is_ok());
    let unselected = virtual_ui::VirtualMcpScope::from_instance_ids(vec![]);
    assert!(
        authorize_reference(&locator, &actor, &catalog, &unselected).is_err(),
        "URI must not select its own instance"
    );
    locator.user_id = uuid::Uuid::from_u128(9);
    assert!(authorize_reference(&locator, &actor, &catalog, &scope).is_err());
    locator.user_id = actor.user_id;
    locator.des_id = "stale".into();
    assert!(authorize_reference(&locator, &actor, &catalog, &scope).is_err());
    locator.des_id = "revision".into();
    catalog.bindings[0].visible = false;
    assert!(authorize_reference(&locator, &actor, &catalog, &scope).is_err());
    catalog.bindings[0].visible = true;
    catalog.tools[0].output_mapping["resource_read"] = json!(false);
    assert!(authorize_reference(&locator, &actor, &catalog, &scope).is_err());
}

#[test]
fn import_blob_requires_one_exact_binary_resource_and_preserves_all_bytes_filename_and_mime() {
    use base64::engine::general_purpose::STANDARD;
    let bytes: Vec<u8> = (0..700_001).map(|n| (n % 251) as u8).collect();
    let uri = "file:///tmp/a%20b.bin";
    let result = json!({"content":[{"type":"resource","resource":{"uri":uri,"blob":STANDARD.encode(&bytes),"mimeType":"application/octet-stream"}}, {"type":"resource_link","uri":uri,"name":"original filename.bin"}]});
    let imported = imported_file(&result, uri).unwrap();
    assert_eq!(imported.bytes, bytes);
    assert_eq!(imported.filename, "original filename.bin");
    assert_eq!(
        imported.mime_type.as_deref(),
        Some("application/octet-stream")
    );
    assert!(imported_file(&result, "file:///other").is_err());
    let mut duplicate = result.clone();
    let block = duplicate["content"][0].clone();
    duplicate["content"].as_array_mut().unwrap().push(block);
    assert!(
        matches!(imported_file(&duplicate, uri), Err(e) if e.code == "file_import_resource_ambiguous")
    );
    let mut text = result.clone();
    text["content"][0]["resource"]["text"] = json!("wrong kind");
    assert!(imported_file(&text, uri).is_err());
    let mut bad = result.clone();
    bad["content"][0]["resource"]["blob"] = json!("%%");
    assert!(imported_file(&bad, uri).is_err());
    let mut failed = result.clone();
    failed["isError"] = json!(true);
    assert!(imported_file(&failed, uri).is_err());
    let mut fallback = result;
    fallback["content"].as_array_mut().unwrap().pop();
    assert_eq!(imported_file(&fallback, uri).unwrap().filename, "a b.bin");
}

#[test]
fn same_installation_serialized_source_is_rejected_before_lock_reentry_but_parallel_read_is_allowed(
) {
    assert!(reentrant_serialized_source(
        "installation",
        "installation",
        true
    ));
    assert!(!reentrant_serialized_source(
        "installation",
        "installation",
        false
    ));
    assert!(!reentrant_serialized_source(
        "installation",
        "other-source",
        true
    ));
}

#[test]
fn independent_link_is_wrapped_and_replay_overrides_only_public_arguments() {
    let mut catalog = catalog_with_server_bound_workspace();
    catalog.tools[0].output_mapping = json!({"structured_content_path":"/metadata","resource_read":true,
        "resource_read_arguments":{"link_only":false},
        "mcp_content":[{"type":"resource_link","uri_path":"/uri","name_path":"/name"},{"type":"resource","uri_path":"/uri","blob_path":"/blob","skip_if_null":true}]});
    let actor = domain::ActorContext::root(
        uuid::Uuid::from_u128(1),
        catalog.instances[0].workspace_id,
        "root",
    );
    let mut content = json!([{"type":"resource_link","uri":"urn:file:a","name":"a"}]);
    wrap_content(
        &mut content,
        &actor,
        &catalog.instances[0],
        &catalog.tools[0],
        &json!({"link_only":true,"path":"/a"}),
    )
    .unwrap();
    let uri = content[0]["uri"].as_str().unwrap();
    let locator = ResourceReference::decode(uri).unwrap();
    let scope = virtual_ui::VirtualMcpScope::single("selected".into());
    assert!(authorize_reference(&locator, &actor, &catalog, &scope).is_ok());
    assert_eq!(locator.arguments["link_only"], true);
    let replay = replay_arguments(&locator, &catalog.tools[0]).unwrap();
    assert_eq!(
        replay,
        json!({"tool_id":"lookup","arguments":{"path":"/a","link_only":false}})
    );
    assert_eq!(locator.user_id, actor.user_id);
    assert!(
        read_result(&json!({"content":content}), "urn:file:a", uri).is_err(),
        "replay requires bytes/text, a link alone is insufficient"
    );
    let duplicate = json!({"content":[{"type":"resource","resource":{"uri":"urn:file:a","blob":"YQ=="}},{"type":"resource","resource":{"uri":"urn:file:a","blob":"Yg=="}}]});
    assert!(read_result(&duplicate, "urn:file:a", uri).is_err());
}
