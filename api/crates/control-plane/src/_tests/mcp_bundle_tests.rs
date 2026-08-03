use std::collections::BTreeMap;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::mcp_bundle::{
    compare_system_versions, official_builtin_interface_disposition, retain_official_builtin_tools,
    OfficialBuiltinInterfaceDisposition,
};

fn interface(
    interface_id: &str,
    source: domain::McpInterfaceCatalogSource,
) -> domain::McpInterfaceCatalogEntry {
    domain::McpInterfaceCatalogEntry {
        interface_id: interface_id.into(),
        source,
        method: "GET".into(),
        path: format!("/api/{interface_id}"),
        name: interface_id.into(),
        short_description: interface_id.into(),
        parameter_descriptors: Vec::new(),
        parameter_schema: serde_json::json!({}),
        result_schema: serde_json::json!({}),
        permission_code: None,
        security: serde_json::json!([]),
        risk_level: domain::McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    }
}

fn tool(
    id: u128,
    tool_id: &str,
    execution_target: domain::McpToolExecutionTarget,
) -> domain::McpToolRecord {
    domain::McpToolRecord {
        id: Uuid::from_u128(id),
        workspace_id: Uuid::nil(),
        tool_id: tool_id.into(),
        name: tool_id.into(),
        short_description: tool_id.into(),
        full_description: tool_id.into(),
        execution_target,
        parameter_schema: serde_json::json!({}),
        result_schema: serde_json::json!({}),
        input_mapping: serde_json::json!({}),
        output_mapping: serde_json::json!({}),
        permission_code: None,
        risk_level: domain::McpRiskLevel::Low,
        des_id: tool_id.into(),
        des_id_required: false,
        status: domain::McpToolStatus::Enabled,
        revision: 1,
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn binding(id: u128, tool_id: &str) -> domain::McpToolBindingRecord {
    domain::McpToolBindingRecord {
        id: Uuid::from_u128(id + 100),
        instance_record_id: Uuid::nil(),
        tool_record_id: Uuid::from_u128(id),
        group_path: "/".into(),
        tool_id: tool_id.into(),
        display_alias: None,
        visible: true,
        sort_order: id as i32,
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
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
