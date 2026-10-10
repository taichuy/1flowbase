use super::*;
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

fn model(id: u128, code: &str) -> PortableDataModel {
    PortableDataModel {
        id: Uuid::from_u128(id),
        code: code.into(),
        title: code.into(),
        description: None,
        scope_kind: domain::DataModelScopeKind::Workspace,
        template_provider: "core".into(),
        template_code: "general".into(),
        template_version: "1".into(),
        status: domain::DataModelStatus::Draft,
        builtin: false,
        fields: vec![],
    }
}
fn group(id: u128, parent: Option<u128>) -> PortablePage {
    PortablePage {
        id: Uuid::from_u128(id),
        parent_id: parent.map(Uuid::from_u128),
        kind: domain::FrontstagePageKind::Group,
        title: Some(format!("group-{id}")),
        icon: None,
        tooltip: None,
        is_hidden: false,
        placement: domain::frontstage::FrontstageNavigationPlacement::Sidebar,
        content_presentation: domain::frontstage::FrontstagePageContentPresentation::Single,
        slug: None,
        rank: "a".into(),
        tabs: vec![],
        visibility_rules: vec![],
    }
}
fn snapshot() -> PortableTemplatePackage {
    PortableTemplatePackage {
        i18n_entries: Vec::new(),
        release: None,
        schema_version: PORTABLE_TEMPLATE_SCHEMA_VERSION.into(),
        pages: vec![],
        applications: vec![],
        data_models: vec![],
        plugins: vec![],
        mcp_bundle: None,
    }
}

fn package_over_former_capacity_limits() -> PortableTemplatePackage {
    let mut package = snapshot();
    package.data_models = (1..=1_001)
        .map(|id| model(id, &format!("model_{id}")))
        .collect();
    package.pages = (2_001..=3_001).map(|id| group(id, None)).collect();
    package.applications = (4_001..=4_101)
        .map(|id| PortableApplication {
            id: Uuid::from_u128(id),
            application_type: domain::ApplicationType::Workflow,
            workflow_trigger_type: None,
            name: format!("Workflow {id}"),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
            flow_document: json!({"graph":{"nodes":[],"edges":[]}}),
            mapping: None,
            published: None,
            schedule: None,
            dependency_issues: vec![],
        })
        .collect();
    package
}

#[test]
fn valid_resources_are_not_rejected_by_arbitrary_template_capacity() {
    let package = package_over_former_capacity_limits();
    let failures = validate_portable_template(&package);
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn large_packages_still_reject_duplicate_and_nil_resource_identities() {
    let mut package = package_over_former_capacity_limits();
    let duplicate_id = package.data_models[0].id;
    package.data_models[1].id = duplicate_id;
    package.pages[0].id = Uuid::nil();
    let failures = validate_portable_template(&package);
    assert!(failures.contains(&format!(
        "portable_template_duplicate_or_nil_id:{duplicate_id}"
    )));
    assert!(failures.contains(&format!(
        "portable_template_duplicate_or_nil_id:{}",
        Uuid::nil()
    )));
}

#[test]
fn mcp_wrappers_pull_portable_definitions_and_remap_their_target_ids() {
    let app_id = Uuid::from_u128(31);
    let model_id = Uuid::from_u128(32);
    let mut all = snapshot();
    all.data_models.push(model(32, "report_data"));
    let mut builtin = model(33, "builtin_data");
    builtin.builtin = true;
    all.data_models.push(builtin);
    all.applications.push(PortableApplication {
        id: app_id,
        application_type: domain::ApplicationType::Workflow,
        workflow_trigger_type: None,
        name: "Report".into(),
        description: String::new(),
        icon: None,
        icon_type: None,
        icon_background: None,
        flow_document: json!({}),
        mapping: None,
        published: None,
        schedule: None,
        dependency_issues: vec![],
    });
    let mut bundle: domain::McpBundlePackage = serde_json::from_value(json!({
        "manifest": {
            "schema_version": "1flowbase.mcp.bundle/v2", "organization": "test",
            "bundle_id": "test", "bundle_version": "1.0.0", "locale": "en_US",
            "minimum_host_version": "0.4.1", "exported_from_system_version": "0.4.1",
            "exported_at": "2026-09-27T00:00:00Z", "files": []
        },
        "tools": [
            {"tool_id":"report", "name":"report", "short_description":"", "full_description":"",
             "execution_target":{"kind":"interface_wrapper","interface_id":format!("published_workflow_operation:{app_id}")},
             "permission_code_snapshot":null,"risk_level_snapshot":"low","status":"enabled"},
            {"tool_id":"model", "name":"model", "short_description":"", "full_description":"",
             "execution_target":{"kind":"interface_wrapper","interface_id":format!("data_model__{model_id}__list_records")},
             "permission_code_snapshot":null,"risk_level_snapshot":"low","status":"enabled"},
            {"tool_id":"builtin", "name":"builtin", "short_description":"", "full_description":"",
             "execution_target":{"kind":"interface_wrapper","interface_id":format!("data_model__{}__list_records", Uuid::from_u128(33))},
             "permission_code_snapshot":null,"risk_level_snapshot":"low","status":"enabled"}
        ],
        "instances": [], "connections": []
    })).unwrap();
    let mut selection = PortableTemplateSelection::default();
    include_mcp_definition_dependencies(&all, &bundle, &mut selection);
    assert_eq!(selection.application_ids, vec![app_id]);
    assert_eq!(selection.data_model_ids, vec![model_id]);
    let target_app = Uuid::from_u128(41);
    let target_model = Uuid::from_u128(42);
    remap_mcp_bundle_interfaces(
        &mut bundle,
        &BTreeMap::from([
            (app_id.to_string(), target_app.to_string()),
            (model_id.to_string(), target_model.to_string()),
        ]),
    );
    assert_eq!(
        bundle.tools[0].execution_target.interface_id(),
        Some(format!("published_workflow_operation:{target_app}").as_str())
    );
    assert_eq!(
        bundle.tools[1].execution_target.interface_id(),
        Some(format!("data_model__{target_model}__list_records").as_str())
    );
}
#[test]
fn selecting_nested_tree_preserves_route_ancestors_but_excludes_siblings_and_unrelated_definitions()
{
    let mut all = snapshot();
    all.pages = vec![
        group(1, None),
        group(2, Some(1)),
        group(3, Some(2)),
        group(4, Some(1)),
    ];
    all.pages[0].placement = domain::frontstage::FrontstageNavigationPlacement::Topbar;
    all.pages[0].slug = Some("template-root".into());
    all.data_models = vec![model(20, "unselected")];
    let out = export_selected_template(
        all,
        PortableTemplateSelection {
            page_ids: vec![Uuid::from_u128(2)],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        out.pages.iter().map(|p| p.id.as_u128()).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(out.pages[0].slug.as_deref(), Some("template-root"));
    assert_eq!(out.pages[1].parent_id, Some(Uuid::from_u128(1)));
    assert!(out.data_models.is_empty());
}
#[test]
fn static_runtime_model_reference_adds_only_required_model() {
    let mut all = snapshot();
    let mut page = group(1, None);
    page.kind = domain::FrontstagePageKind::Page;
    page.tabs.push(PortableTab{id:Uuid::from_u128(2),title:None,rank:"a".into(),is_default:true,route_segment:None,document_root_uid:"root".into(),document_payload:json!({"url":"/api/runtime/models/orders","unrelated":"/api/console/settings/mocks"}),blocks:vec![]});
    all.pages.push(page);
    all.data_models = vec![model(20, "orders"), model(21, "private_unselected")];
    let out = export_selected_template(
        all,
        PortableTemplateSelection {
            page_ids: vec![Uuid::from_u128(1)],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(out.data_models.len(), 1);
    assert_eq!(out.data_models[0].code, "orders");
}
#[test]
fn missing_supported_dependency_fails_export() {
    let mut all = snapshot();
    let mut page = group(1, None);
    page.kind = domain::FrontstagePageKind::Page;
    page.tabs.push(PortableTab {
        id: Uuid::from_u128(2),
        title: None,
        rank: "a".into(),
        is_default: true,
        route_segment: None,
        document_root_uid: "root".into(),
        document_payload: json!({"url":"/api/runtime/models/missing"}),
        blocks: vec![],
    });
    all.pages.push(page);
    let error = export_selected_template(
        all,
        PortableTemplateSelection {
            page_ids: vec![Uuid::from_u128(1)],
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("unresolved_model:missing"));
}
#[test]
fn external_or_cross_workspace_selection_does_not_silently_export_empty() {
    let error = export_selected_template(
        snapshot(),
        PortableTemplateSelection {
            data_model_ids: vec![Uuid::from_u128(1)],
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("missing_or_external_source"));
}
#[test]
fn known_identity_rewrite_is_bounded_non_cascading_and_preserves_unrelated_literals() {
    let a = "11111111-1111-1111-1111-111111111111";
    let b = "22222222-2222-2222-2222-222222222222";
    let ids = BTreeMap::from([(a.into(), b.into()), (b.into(), "third".into())]);
    assert_eq!(
        rewrite_template_text(&format!("/api/apps/{a}?q=x"), &ids),
        format!("/api/apps/{b}?q=x")
    );
    let unrelated = format!("const x='33333333-3333-3333-3333-333333333333'; const y='x{a}';");
    assert_eq!(rewrite_template_text(&unrelated, &ids), unrelated);
}
#[test]
fn preview_reports_collision_without_mutating_target() {
    let mut package = snapshot();
    package.data_models.push(model(1, "orders"));
    let mut target = snapshot();
    target.data_models.push(model(2, "orders"));
    let preview = preview_portable_template(&package, &target);
    assert!(!preview.valid);
    assert!(preview
        .failures
        .iter()
        .any(|f| f.contains("model_code_conflict:orders")));
    assert_eq!(target.data_models[0].id, Uuid::from_u128(2));
}
#[test]
fn preview_updates_matching_id_and_uses_recorded_identity_for_repeated_import() {
    let mut package = snapshot();
    package.data_models.push(model(1, "orders"));
    let mut target = snapshot();
    target.data_models.push(model(1, "orders"));
    let direct = preview_portable_template(&package, &target);
    assert!(direct.valid, "{:?}", direct.failures);
    assert_eq!(direct.effects[0].action, "update");

    target.data_models[0].id = Uuid::from_u128(2);
    let mapped = preview_portable_template_with_map(
        &package,
        &target,
        &BTreeMap::from([(
            Uuid::from_u128(1).to_string(),
            Uuid::from_u128(2).to_string(),
        )]),
    );
    assert!(mapped.valid, "{:?}", mapped.failures);
    assert_eq!(
        mapped.effects[0].target_id.as_deref(),
        Some(Uuid::from_u128(2).to_string().as_str())
    );
}
#[test]
fn package_shape_rejects_ownership_external_source_and_payload_data() {
    let mut package = snapshot();
    package.data_models.push(model(1, "orders"));
    let value = serde_json::to_value(&package).unwrap();
    for key in [
        "scope_id",
        "physical_table_name",
        "data_source_instance_id",
        "rows",
        "created_by",
    ] {
        let mut corrupted = value.clone();
        corrupted["data_models"][0][key] = json!("source-secret-or-owner");
        assert!(
            serde_json::from_value::<PortableTemplatePackage>(corrupted).is_err(),
            "{key}"
        );
    }
}
#[test]
fn cyclic_page_graph_and_missing_default_tab_fail_before_writes() {
    let mut package = snapshot();
    package.pages = vec![group(1, Some(2)), group(2, Some(1))];
    package.pages[0].kind = domain::FrontstagePageKind::Page;
    let errors = validate_portable_template(&package);
    assert!(errors.iter().any(|e| e.contains("page_cycle")));
    assert!(errors.iter().any(|e| e.contains("default_tab")));
}

#[test]
fn frontend_plugin_manifest_uses_canonical_camelcase_identity_and_never_packages_artifact_bytes() {
    let mut package = snapshot();
    let mut page = group(1, None);
    page.kind = domain::FrontstagePageKind::Page;
    page.tabs.push(PortableTab{id:Uuid::from_u128(2),title:None,rank:"a".into(),is_default:true,route_segment:None,document_root_uid:"root".into(),document_payload:json!({"catalog":{"providerCode":"acme","installationId":Uuid::from_u128(50)},"contribution":{"pluginId":"acme/ui","pluginVersion":"1.2.3","code":"jsx"}}),blocks:vec![]});
    package.pages.push(page);
    package.plugins.push(PortablePluginDependency {
        plugin_id: "acme/ui".into(),
        plugin_version: "1.2.3".into(),
        checksum: Some("sha256:fixture".into()),
        contribution_code: None,
    });
    let out = export_selected_template(
        package,
        PortableTemplateSelection {
            page_ids: vec![Uuid::from_u128(1)],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(out.plugins.len(), 1);
    assert_eq!(out.plugins[0].contribution_code.as_deref(), Some("jsx"));
    assert_eq!(out.plugins[0].checksum.as_deref(), Some("sha256:fixture"));
}
#[test]
fn provider_only_nonbuiltin_frontend_and_model_templates_fail_closed() {
    let mut package = snapshot();
    let mut m = model(1, "orders");
    m.template_provider = "unknown-vendor".into();
    package.data_models.push(m);
    let mut page = group(2, None);
    page.kind = domain::FrontstagePageKind::Page;
    page.tabs.push(PortableTab {
        id: Uuid::from_u128(3),
        title: None,
        rank: "a".into(),
        is_default: true,
        route_segment: None,
        document_root_uid: "root".into(),
        document_payload: json!({"catalog":{"providerCode":"unknown-vendor"}}),
        blocks: vec![],
    });
    package.pages.push(page);
    let errors = validate_portable_template(&package);
    assert!(errors
        .iter()
        .any(|e| e.contains("unsupported_model_template_provider")));
    assert!(errors
        .iter()
        .any(|e| e.contains("unsupported_frontend_provider")));
}

#[test]
fn page_routes_are_rejected_before_any_owner_writes() {
    let mut package = snapshot();
    let mut page = group(1, None);
    page.slug = Some("illegal-sidebar-route".into());
    package.pages.push(page);
    assert!(validate_portable_template(&package)
        .iter()
        .any(|e| e.contains("page_slug_not_allowed")));
    package.pages[0].placement = domain::frontstage::FrontstageNavigationPlacement::Topbar;
    assert!(validate_portable_template(&package).is_empty());
}

#[test]
fn source_mapping_keys_cannot_alias_different_resource_classes() {
    let mut package = snapshot();
    let mut page = group(1, None);
    page.kind = domain::FrontstagePageKind::Page;
    page.tabs.push(PortableTab {
        id: Uuid::from_u128(2),
        title: None,
        rank: "a".into(),
        is_default: true,
        route_segment: None,
        document_root_uid: "root".into(),
        document_payload: json!({}),
        blocks: vec![PortableBlock {
            block_id: Uuid::from_u128(3).to_string(),
            parent_block_id: None,
            rank: "a".into(),
            presentation: domain::frontstage::FrontstageBlockPresentation::Inline,
            title: None,
            description: None,
            code_ref: "code".into(),
            schema_version: 1,
            input_mapping: BTreeMap::new(),
            output_mapping: BTreeMap::new(),
            runtime_descriptor: json!({}),
            source_code: String::new(),
        }],
    });
    package.pages.push(page);
    assert!(validate_portable_template(&package).is_empty());
    for slot in ["block", "root", "code", "installation"] {
        let mut invalid = package.clone();
        let page_id = invalid.pages[0].id.to_string();
        let tab = &mut invalid.pages[0].tabs[0];
        match slot {
            "block" => tab.blocks[0].block_id = page_id,
            "root" => tab.document_root_uid = page_id,
            "code" => tab.blocks[0].code_ref = page_id,
            _ => {
                tab.blocks[0].runtime_descriptor = json!({"catalog":{"installationId":page_id},"contribution":{"pluginId":"example@1","pluginVersion":"1"}})
            }
        }
        assert!(
            validate_portable_template(&invalid)
                .iter()
                .any(|e| e.contains("identity_collision")),
            "{slot}"
        );
    }
}

#[test]
fn shared_draft_and_publication_flow_identity_has_one_owner() {
    let mut package = snapshot();
    let flow = json!({"meta":{"flowId":Uuid::from_u128(20)}});
    let mut app = PortableApplication {
        id: Uuid::from_u128(1),
        application_type: domain::ApplicationType::Workflow,
        workflow_trigger_type: None,
        name: "Template".into(),
        description: String::new(),
        icon: None,
        icon_type: None,
        icon_background: None,
        flow_document: flow.clone(),
        mapping: None,
        schedule: None,
        dependency_issues: vec![],
        published: Some(PortablePublication {
            flow_document: flow,
            mapping:
                crate::application_public_api::mapping::ApplicationApiMappingConfig::default_native(
                ),
            api_enabled: true,
        }),
    };
    package.applications.push(app.clone());
    assert!(super::identity::validate_identity_namespace(&package).is_empty());
    app.id = Uuid::from_u128(2);
    package.applications.push(app);
    assert!(super::identity::validate_identity_namespace(&package)
        .iter()
        .any(|e| e.contains("identity_collision")));
    package.applications.pop();
    package.data_models.push(model(20, "flow_collision"));
    assert!(super::identity::validate_identity_namespace(&package)
        .iter()
        .any(|e| e.contains("identity_collision")));
}

mod releases;

#[test]
fn selected_translations_can_be_exported_without_native_resources() {
    let mut all = snapshot();
    all.pages.push(group(1, None));
    all.i18n_entries = vec![
        PortableI18nEntry {
            key: "Hello".into(),
            locale: "zh_Hans".into(),
            translation: "你好".into(),
        },
        PortableI18nEntry {
            key: "Other".into(),
            locale: "en_US".into(),
            translation: "Other".into(),
        },
    ];
    let out = export_selected_template(
        all.clone(),
        PortableTemplateSelection {
            i18n_keys: vec!["Hello".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(out.pages.is_empty());
    assert_eq!(out.schema_version, PORTABLE_TEMPLATE_I18N_SCHEMA_VERSION);
    assert_eq!(out.i18n_entries, vec![all.i18n_entries[0].clone()]);
    let native = export_selected_template(
        all.clone(),
        PortableTemplateSelection {
            page_ids: vec![Uuid::from_u128(1)],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(native.schema_version, PORTABLE_TEMPLATE_SCHEMA_VERSION);
    assert!(native.i18n_entries.is_empty());
    assert!(export_selected_template(
        all,
        PortableTemplateSelection {
            i18n_keys: vec!["Missing".into()],
            ..Default::default()
        }
    )
    .unwrap_err()
    .to_string()
    .contains("missing_i18n_key"));
}
