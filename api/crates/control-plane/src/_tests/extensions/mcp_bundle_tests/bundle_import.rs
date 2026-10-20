use super::*;

#[tokio::test]
async fn bundle_preview_and_import_preserve_shared_tool_impact_and_atomic_replace_input() {
    let actor_user_id = Uuid::from_u128(110);
    let workspace_id = Uuid::from_u128(120);
    let selected_instance_record_id = Uuid::from_u128(130);
    let other_instance_record_id = Uuid::from_u128(131);
    let mut package = managed_frontstage_package("1.0.2");
    package.instances[0].name = "New name".into();
    package.tools[0].name = "New tool".into();
    package.tools[0].input_mapping = serde_json::json!({
        "interface_parameters": [
            {"name":"des_id","required":false,"source":{"kind":"mcp_call"}},
            {"name":"max_inline_chars","required":false,"source":{"kind":"mcp_call"}},
            {"name":"response_fields","required":false,"source":{"kind":"mcp_call"}}
        ],
        "mappings": [{"interface_param":"des_id","mcp_param":"des_id","required":true,"source":{"kind":"mcp_call","path":"des_id","tool_id":package.tools[1].tool_id}}]
    });

    let instance = |id, instance_id: &str, name: &str| domain::McpInstanceRecord {
        id,
        workspace_id,
        instance_id: instance_id.into(),
        name: name.into(),
        description_short: Some("Managed".into()),
        status: domain::McpInstanceStatus::Enabled,
        default_entry_path: "/frontstage".into(),
        webmcp_exposure: domain::WebMcpExposure::Disabled,
        managed_by: None,
        created_by: actor_user_id,
        updated_by: actor_user_id,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let policy = |id, instance_record_id| domain::McpInstanceDiscoveryPolicyRecord {
        id,
        workspace_id,
        instance_record_id,
        list_default_limit: 20,
        list_max_depth: 3,
        list_regex_enabled: false,
        list_regex_max_length: 64,
        list_return_fields: serde_json::json!(["id", "name"]),
        created_by: actor_user_id,
        updated_by: actor_user_id,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let record_from_bundle = |id: u128, bundle: &domain::McpBundleTool| {
        let mut record = tool(id, &bundle.tool_id, bundle.execution_target.clone());
        record.workspace_id = workspace_id;
        record.name = bundle.name.clone();
        record.short_description = bundle.short_description.clone();
        record.full_description = bundle.full_description.clone();
        record.parameter_schema = bundle.parameter_schema_snapshot.clone();
        record.result_schema = bundle.result_schema_snapshot.clone();
        record.input_mapping = bundle.input_mapping.clone();
        record.output_mapping = bundle.output_mapping.clone();
        record.permission_code = bundle.permission_code_snapshot.clone();
        record.risk_level = bundle.risk_level_snapshot;
        record.created_by = actor_user_id;
        record.updated_by = actor_user_id;
        record
    };
    let mut first_tool = record_from_bundle(1, &package.tools[0]);
    first_tool.name = "Old tool".into();
    let second_tool = record_from_bundle(2, &package.tools[1]);
    let scoped_binding = |id, instance_record_id, tool_id: &str| {
        let mut record = binding(id, tool_id);
        record.instance_record_id = instance_record_id;
        record.created_by = actor_user_id;
        record.updated_by = actor_user_id;
        record
    };
    let repository = McpBundleFixtureRepository::with_export_snapshot(McpBundleExportSnapshot {
        actor: domain::ActorContext::root(actor_user_id, workspace_id, "root"),
        instances: vec![
            instance(
                selected_instance_record_id,
                "frontstage_browser",
                "Old name",
            ),
            instance(other_instance_record_id, "keep_me", "Keep me"),
        ],
        groups: vec![domain::McpGroupRecord {
            id: Uuid::from_u128(140),
            instance_record_id: selected_instance_record_id,
            path: "/frontstage".into(),
            display_name: "Frontstage".into(),
            description_short: None,
            enabled: true,
            sort_order: 0,
            created_by: actor_user_id,
            updated_by: actor_user_id,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }],
        bindings: vec![
            scoped_binding(
                1,
                selected_instance_record_id,
                "frontstage_list_page_blocks",
            ),
            scoped_binding(
                2,
                selected_instance_record_id,
                "frontstage_inspect_block_render",
            ),
            scoped_binding(1, other_instance_record_id, "frontstage_list_page_blocks"),
        ],
        policies: vec![
            policy(Uuid::from_u128(150), selected_instance_record_id),
            policy(Uuid::from_u128(151), other_instance_record_id),
        ],
        tools: vec![first_tool, second_tool],
        connections: Vec::new(),
    });
    let service = McpManagementService::new(repository.clone());

    let mut sparse_template = package.clone();
    sparse_template.instances[0].groups.clear();
    sparse_template.instances[0].bindings.clear();
    let merged_template = service
        .preserve_unmentioned_instance_entries(actor_user_id, sparse_template)
        .await
        .unwrap();
    assert_eq!(merged_template.instances[0].groups.len(), 1);
    assert_eq!(merged_template.instances[0].bindings.len(), 2);

    let preview = service
        .preview_bundle(crate::mcp_bundle::PreviewMcpBundleCommand {
            actor_user_id,
            package: package.clone(),
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        preview.instances[0].effect,
        domain::McpBundleItemEffect::Update
    );
    assert_eq!(preview.tools[0].effect, domain::McpBundleItemEffect::Update);
    assert_eq!(
        preview.shared_tool_impacts,
        vec![domain::McpBundleSharedToolImpact {
            tool_id: "frontstage_list_page_blocks".into(),
            instance_ids: vec!["keep_me".into()],
        }]
    );

    service
        .import_bundle(crate::mcp_bundle::ImportMcpBundleCommand {
            actor_user_id,
            package: package.clone(),
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .unwrap();
    let inputs = repository.replace_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].actor_user_id, actor_user_id);
    assert_eq!(inputs[0].workspace_id, workspace_id);
    assert_eq!(inputs[0].instances[0].name, "New name");
    assert_eq!(inputs[0].tools[0].name, "New tool");
    assert!(inputs[0].tools[0].des_id_required);
    assert!(
        !service
            .description_check(actor_user_id, &package.tools[0].tool_id, None)
            .await
            .unwrap()
            .accepted
    );
    assert!(
        !service
            .description_check(
                actor_user_id,
                &package.tools[0].tool_id,
                Some(&package.tools[0].tool_id)
            )
            .await
            .unwrap()
            .accepted
    );
    assert!(
        service
            .description_check(
                actor_user_id,
                &package.tools[0].tool_id,
                Some(&package.tools[1].tool_id)
            )
            .await
            .unwrap()
            .accepted
    );

    let mut invalid_package = package;
    invalid_package.instances[0].bindings[0].tool_id = "missing_tool".into();
    repository.reject_replace();
    assert!(service
        .import_bundle(crate::mcp_bundle::ImportMcpBundleCommand {
            actor_user_id,
            package: invalid_package,
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .is_err());
    assert_eq!(repository.replace_inputs().len(), 1);
}

#[tokio::test]
async fn explicit_bundle_import_restores_a_missing_managed_graph_through_atomic_replace() {
    let actor_user_id = Uuid::from_u128(210);
    let workspace_id = Uuid::from_u128(220);
    let repository = McpBundleFixtureRepository::with_export_snapshot(McpBundleExportSnapshot {
        actor: domain::ActorContext::root(actor_user_id, workspace_id, "root"),
        instances: Vec::new(),
        groups: Vec::new(),
        bindings: Vec::new(),
        policies: Vec::new(),
        tools: Vec::new(),
        connections: Vec::new(),
    });

    McpManagementService::new(repository.clone())
        .import_bundle(crate::mcp_bundle::ImportMcpBundleCommand {
            actor_user_id,
            package: managed_frontstage_package("1.0.2"),
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .unwrap();

    let inputs = repository.replace_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].source.organization, "1flowbase");
    assert_eq!(inputs[0].source.bundle_id, "frontstage_assistant");
    assert_eq!(inputs[0].instances[0].instance_id, "frontstage_browser");
    assert_eq!(inputs[0].tools.len(), 2);
}

#[test]
fn compares_export_source_and_current_system_versions() {
    assert_eq!(
        compare_system_versions("0.2.5", "0.2.6"),
        domain::McpBundleVersionStatus::ExportedFromOlderSystem
    );
    assert_eq!(
        compare_system_versions("0.3.0", "0.2.6"),
        domain::McpBundleVersionStatus::ExportedFromNewerSystem
    );
    assert_eq!(
        compare_system_versions("latest", "0.2.6"),
        domain::McpBundleVersionStatus::UnknownSystemVersion
    );
}

#[test]
fn ac_002_ac_004_official_builtin_export_filters_workspace_capabilities_and_fails_unknown() {
    let sources = BTreeMap::from([
        (
            "static_api".to_string(),
            domain::McpInterfaceCatalogSource::StaticApi,
        ),
        (
            "builtin_crud".to_string(),
            domain::McpInterfaceCatalogSource::BuiltinDataModelCrud,
        ),
        (
            "workflow".to_string(),
            domain::McpInterfaceCatalogSource::PublishedWorkflow,
        ),
        (
            "workspace_crud".to_string(),
            domain::McpInterfaceCatalogSource::WorkspaceDataModelCrud,
        ),
    ]);

    for interface_id in ["static_api", "builtin_crud"] {
        assert_eq!(
            official_builtin_interface_disposition(interface_id, &sources).unwrap(),
            OfficialBuiltinInterfaceDisposition::Include
        );
    }
    assert_eq!(
        official_builtin_interface_disposition("workflow", &sources).unwrap(),
        OfficialBuiltinInterfaceDisposition::Exclude("published_workflow")
    );
    assert_eq!(
        official_builtin_interface_disposition("workspace_crud", &sources).unwrap(),
        OfficialBuiltinInterfaceDisposition::Exclude("workspace_data_model_crud")
    );
    assert!(official_builtin_interface_disposition("missing", &sources).is_err());
}

#[test]
fn ac_002_official_builtin_export_removes_excluded_tools_and_their_bindings() {
    let mut tools = vec![
        tool(
            1,
            "static_api",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "static_api".into(),
            },
        ),
        tool(
            2,
            "builtin_crud",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "builtin_crud".into(),
            },
        ),
        tool(
            3,
            "workflow",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "workflow".into(),
            },
        ),
        tool(
            4,
            "workspace_crud",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "workspace_crud".into(),
            },
        ),
        tool(
            5,
            "proxy",
            domain::McpToolExecutionTarget::McpProxy {
                upstream_connection_id: Uuid::from_u128(99),
                remote_tool_name: "remote".into(),
                source_schema_hash: "hash".into(),
            },
        ),
    ];
    let mut bindings = vec![
        binding(1, "static_api"),
        binding(2, "builtin_crud"),
        binding(3, "workflow"),
        binding(4, "workspace_crud"),
        binding(5, "proxy"),
    ];
    let catalog = vec![
        interface("static_api", domain::McpInterfaceCatalogSource::StaticApi),
        interface(
            "builtin_crud",
            domain::McpInterfaceCatalogSource::BuiltinDataModelCrud,
        ),
        interface(
            "workflow",
            domain::McpInterfaceCatalogSource::PublishedWorkflow,
        ),
        interface(
            "workspace_crud",
            domain::McpInterfaceCatalogSource::WorkspaceDataModelCrud,
        ),
    ];

    let report = retain_official_builtin_tools(&mut tools, &mut bindings, &catalog).unwrap();

    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["static_api", "builtin_crud", "proxy"]
    );
    assert_eq!(
        bindings
            .iter()
            .map(|binding| binding.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["static_api", "builtin_crud", "proxy"]
    );
    assert_eq!(report.excluded_tool_count, 2);
    assert_eq!(
        report.exclusion_reasons,
        vec!["published_workflow", "workspace_data_model_crud"]
    );
}

#[tokio::test]
async fn bundle_preview_rejects_retired_v1_manifest_before_replacing_any_graph() {
    let actor_user_id = Uuid::now_v7();
    let repository = McpBundleFixtureRepository::with_export_snapshot(McpBundleExportSnapshot {
        actor: domain::ActorContext::root(actor_user_id, Uuid::now_v7(), "root"),
        instances: Vec::new(),
        groups: Vec::new(),
        bindings: Vec::new(),
        policies: Vec::new(),
        tools: Vec::new(),
        connections: Vec::new(),
    });
    let mut package = managed_frontstage_package("1.0.2");
    package.manifest.schema_version = "1flowbase.mcp.bundle/v1".into();
    let result = McpManagementService::new(repository.clone())
        .preview_bundle(crate::mcp_bundle::PreviewMcpBundleCommand {
            actor_user_id,
            package,
            interface_catalog: Vec::new(),
            current_system_version: "0.5.7".into(),
        })
        .await;
    assert!(result.unwrap_err().to_string().contains("schema_version"));
    assert!(repository.replace_inputs().is_empty());
}
