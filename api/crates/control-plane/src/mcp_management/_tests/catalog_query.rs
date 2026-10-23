use super::list_catalog_items;
use domain::{
    McpCatalogSnapshot, McpGroupRecord, McpInstanceDiscoveryPolicyRecord, McpInstanceRecord,
    McpInstanceStatus, McpListItemKind, McpListItemSummary, McpRiskLevel, McpToolBindingRecord,
    McpToolExecutionTarget, McpToolRecord, McpToolStatus, WebMcpExposure,
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

const INSTANCE: Uuid = Uuid::from_u128(1);
const NOW: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

fn catalog() -> McpCatalogSnapshot {
    McpCatalogSnapshot {
        instances: vec![McpInstanceRecord {
            id: INSTANCE,
            workspace_id: INSTANCE,
            instance_id: "catalog".into(),
            name: "Catalog".into(),
            description_short: None,
            status: McpInstanceStatus::Enabled,
            default_entry_path: "/ops".into(),
            webmcp_exposure: WebMcpExposure::Disabled,
            managed_by: None,
            created_by: INSTANCE,
            updated_by: INSTANCE,
            created_at: NOW,
            updated_at: NOW,
        }],
        discovery_policies: vec![McpInstanceDiscoveryPolicyRecord {
            id: INSTANCE,
            workspace_id: INSTANCE,
            instance_record_id: INSTANCE,
            list_default_limit: 20,
            list_max_depth: 2,
            list_regex_enabled: true,
            list_regex_max_length: 32,
            list_return_fields: json!(["id", "name"]),
            created_by: INSTANCE,
            updated_by: INSTANCE,
            created_at: NOW,
            updated_at: NOW,
        }],
        groups: vec![],
        tools: vec![],
        bindings: vec![],
    }
}

fn group(id: u128, path: &str, name: &str) -> McpGroupRecord {
    McpGroupRecord {
        id: Uuid::from_u128(id),
        instance_record_id: INSTANCE,
        path: path.into(),
        display_name: name.into(),
        description_short: None,
        enabled: true,
        sort_order: -(id as i32),
        created_by: INSTANCE,
        updated_by: INSTANCE,
        created_at: NOW,
        updated_at: NOW,
    }
}

fn tool(id: u128, name: &str, status: McpToolStatus) -> McpToolRecord {
    McpToolRecord {
        id: Uuid::from_u128(id),
        workspace_id: INSTANCE,
        tool_id: format!("tool_{id}"),
        name: name.into(),
        short_description: "Fetch detail".into(),
        full_description: "Full description is not searched or returned".into(),
        execution_target: McpToolExecutionTarget::InterfaceWrapper {
            interface_id: "catalog.read".into(),
        },
        parameter_schema: json!({}),
        result_schema: json!({}),
        input_mapping: json!({}),
        output_mapping: json!({}),
        max_inline_chars: None,
        response_fields: None,
        permission_code: None,
        risk_level: McpRiskLevel::Low,
        des_id: "description".into(),
        des_id_required: false,
        status,
        revision: 1,
        managed_by: None,
        created_by: INSTANCE,
        updated_by: INSTANCE,
        created_at: NOW,
        updated_at: NOW,
    }
}

fn binding(id: u128, tool_id: u128, path: &str, alias: Option<&str>) -> McpToolBindingRecord {
    McpToolBindingRecord {
        id: Uuid::from_u128(id),
        instance_record_id: INSTANCE,
        tool_record_id: Uuid::from_u128(tool_id),
        group_path: path.into(),
        tool_id: "binding_id_is_not_the_output_id".into(),
        display_alias: alias.map(str::to_owned),
        visible: true,
        sort_order: -(id as i32),
        created_by: INSTANCE,
        updated_by: INSTANCE,
        created_at: NOW,
        updated_at: NOW,
    }
}

#[test]
fn catalog_preserves_visibility_aliases_and_distinct_duplicate_id_rules() {
    use McpListItemKind::{Group, Tool};
    use McpRiskLevel::{High, Low};
    use McpToolStatus::{Disabled, Enabled};

    let mut catalog = catalog();
    catalog.groups = vec![
        group(12, "/ops/z", "Zed"),
        group(10, "/ops", "Operations"),
        group(11, "/ops/a", "Alpha"),
        group(13, "/hidden", "Hidden"),
        group(14, "/ops/foreign", "Foreign"),
    ];
    catalog.groups[1].description_short = Some("Operations directory".into());
    catalog.groups[3].enabled = false;
    catalog.groups[4].instance_record_id = Uuid::from_u128(2);
    catalog.tools = vec![
        tool(100, "Reader", Enabled),
        tool(101, "Disabled", Disabled),
        tool(102, "First disabled", Disabled),
        tool(102, "Later enabled", Enabled),
        tool(103, "First enabled", Enabled),
        tool(103, "Later replacement", Enabled),
    ];
    catalog.tools[4].short_description = "First record".into();
    catalog.tools[4].risk_level = High;
    catalog.tools[5].tool_id = "replacement".into();
    catalog.tools[5].risk_level = McpRiskLevel::Critical;
    catalog.bindings = vec![
        binding(20, 100, "/ops/a", Some("Alias")),
        binding(21, 100, "/ops", None),
        binding(22, 101, "/ops", None),
        binding(23, 999, "/ops", None),
        binding(24, 100, "/ops", Some("Invisible")),
        binding(25, 102, "/ops", None),
        binding(26, 103, "/ops", None),
        binding(27, 100, "/ops/a", Some("Second alias")),
        binding(28, 100, "/hidden", None),
        binding(29, 100, "/ops", Some("Foreign")),
    ];
    catalog.bindings[4].visible = false;
    catalog.bindings[9].instance_record_id = Uuid::from_u128(2);

    let items = list_catalog_items(&catalog, "catalog", None, None, None, None, None).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| (item.id.as_str(), item.path.as_str(), item.name.as_str()))
            .collect::<Vec<_>>(),
        [
            ("00000000-0000-0000-0000-00000000000c", "/ops/z", "Zed"),
            ("00000000-0000-0000-0000-00000000000a", "/ops", "Operations"),
            ("00000000-0000-0000-0000-00000000000b", "/ops/a", "Alpha"),
            ("tool_100", "/ops/a", "Alias"),
            ("tool_100", "/ops", "Reader"),
            ("tool_103", "/ops", "First enabled"),
            ("tool_100", "/ops/a", "Second alias"),
        ]
    );
    assert_eq!(
        items
            .iter()
            .map(|item| (
                item.item_kind,
                item.description_short.as_deref(),
                item.children_count,
                item.risk_level
            ))
            .collect::<Vec<_>>(),
        [
            (Group, None, 0, None),
            (Group, Some("Operations directory"), 5, None),
            (Group, None, 2, None),
            (Tool, Some("Fetch detail"), 0, Some(Low)),
            (Tool, Some("Fetch detail"), 0, Some(Low)),
            (Tool, Some("First record"), 0, Some(High)),
            (Tool, Some("Fetch detail"), 0, Some(Low)),
        ]
    );
    let hidden_group =
        list_catalog_items(&catalog, "catalog", Some("/hidden"), None, None, None, None).unwrap();
    assert_eq!(
        hidden_group,
        vec![McpListItemSummary {
            path: "/hidden".into(),
            ..items[4].clone()
        }]
    );
    let limited = list_catalog_items(&catalog, "catalog", None, None, None, None, Some(2)).unwrap();
    assert_eq!(limited, items[..2]);
}

#[test]
fn catalog_reverse_ordered_tool_lookup_keeps_binding_order_and_final_limits() {
    let mut catalog = catalog();
    catalog.instances[0].default_entry_path = "/".into();
    catalog.tools = (0..64)
        .rev()
        .map(|id| tool(id, "Reader", McpToolStatus::Enabled))
        .collect();
    catalog.bindings = (0..64).map(|id| binding(id + 100, id, "/", None)).collect();
    let items = list_catalog_items(&catalog, "catalog", None, None, None, None, Some(64)).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
            .join(","),
        concat!(
            "tool_0,tool_1,tool_2,tool_3,tool_4,tool_5,tool_6,tool_7,",
            "tool_8,tool_9,tool_10,tool_11,tool_12,tool_13,tool_14,tool_15,",
            "tool_16,tool_17,tool_18,tool_19,tool_20,tool_21,tool_22,tool_23,",
            "tool_24,tool_25,tool_26,tool_27,tool_28,tool_29,tool_30,tool_31,",
            "tool_32,tool_33,tool_34,tool_35,tool_36,tool_37,tool_38,tool_39,",
            "tool_40,tool_41,tool_42,tool_43,tool_44,tool_45,tool_46,tool_47,",
            "tool_48,tool_49,tool_50,tool_51,tool_52,tool_53,tool_54,tool_55,",
            "tool_56,tool_57,tool_58,tool_59,tool_60,tool_61,tool_62,tool_63"
        )
    );
    catalog.discovery_policies[0].list_default_limit = 2;
    let items = list_catalog_items(&catalog, "catalog", None, None, None, None, None).unwrap();
    assert_eq!(
        items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["tool_0", "tool_1"]
    );
    assert!(
        list_catalog_items(&catalog, "catalog", None, None, None, None, Some(0))
            .unwrap()
            .is_empty()
    );
}
