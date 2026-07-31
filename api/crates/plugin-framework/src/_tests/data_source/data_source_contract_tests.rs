use plugin_framework::data_source_contract::{
    validate_native_sql_output, DataSourceCatalogEntry, DataSourceCreateRecordInput,
    DataSourceCrudCapabilities, DataSourceDeleteRecordInput, DataSourceGetRecordInput,
    DataSourceListRecordsInput, DataSourceRecordFilter, DataSourceRecordPage,
    DataSourceRecordScopeContext, DataSourceRecordSort, DataSourceResourceDescriptor,
    DataSourceStdioMethod, DataSourceUpdateRecordInput, NativeSqlColumn, NativeSqlExecutionItem,
    NativeSqlExecutionOutput, NativeSqlLogicalType, NativeSqlValueEncoding,
};
use serde_json::json;

#[test]
fn data_source_stdio_methods_are_stable() {
    let methods = [
        (DataSourceStdioMethod::ValidateConfig, "validate_config"),
        (DataSourceStdioMethod::TestConnection, "test_connection"),
        (DataSourceStdioMethod::DiscoverCatalog, "discover_catalog"),
        (DataSourceStdioMethod::DescribeResource, "describe_resource"),
        (DataSourceStdioMethod::PreviewRead, "preview_read"),
        (DataSourceStdioMethod::ImportSnapshot, "import_snapshot"),
        (DataSourceStdioMethod::ListRecords, "list_records"),
        (DataSourceStdioMethod::GetRecord, "get_record"),
        (DataSourceStdioMethod::CreateRecord, "create_record"),
        (DataSourceStdioMethod::UpdateRecord, "update_record"),
        (DataSourceStdioMethod::DeleteRecord, "delete_record"),
        (DataSourceStdioMethod::ExecuteSql, "execute_sql"),
    ];

    for (method, expected) in methods {
        assert_eq!(
            serde_json::to_string(&method).unwrap(),
            format!("\"{expected}\"")
        );
    }
}

#[test]
fn ac_004_native_sql_contract_rejects_row_width_and_encoding_mismatches() {
    let output = NativeSqlExecutionOutput {
        results: vec![NativeSqlExecutionItem::RowBatch {
            columns: vec![NativeSqlColumn {
                name: "payload".to_string(),
                native_type: "CUSTOM".to_string(),
                logical_type: NativeSqlLogicalType::Native,
                encoding: NativeSqlValueEncoding::Base64,
            }],
            rows: vec![vec![json!("not base64!"), json!("extra")]],
        }],
    };

    let error = validate_native_sql_output(&output).unwrap_err();
    assert!(error.starts_with("invalid_native_sql_result_contract"));
    assert!(error.contains("2 values for 1 columns"));
}

#[test]
fn resource_descriptor_defaults_to_no_crud_capability_snapshot() {
    let descriptor: DataSourceResourceDescriptor = serde_json::from_value(json!({
        "resource_key": "contacts",
        "supports_preview_read": true,
        "supports_import_snapshot": false
    }))
    .unwrap();

    assert_eq!(descriptor.resource_key, "contacts");
    assert_eq!(
        descriptor.capabilities,
        DataSourceCrudCapabilities::default()
    );
}

#[test]
fn catalog_entry_can_carry_resource_capability_snapshot() {
    let entry = DataSourceCatalogEntry {
        resource_key: "contacts".to_string(),
        display_name: "Contacts".to_string(),
        resource_kind: "object".to_string(),
        capabilities: DataSourceCrudCapabilities {
            supports_list: true,
            supports_get: true,
            supports_filter: true,
            supports_scope_filter: true,
            ..Default::default()
        },
        metadata: json!({}),
    };

    let value = serde_json::to_value(entry).unwrap();
    assert_eq!(value["capabilities"]["supports_list"], true);
    assert_eq!(value["capabilities"]["supports_get"], true);
    assert_eq!(value["capabilities"]["supports_filter"], true);
    assert_eq!(value["capabilities"]["supports_scope_filter"], true);
}

#[test]
fn resource_descriptor_declares_read_write_filter_scope_and_transaction_capabilities() {
    let descriptor = DataSourceResourceDescriptor {
        resource_key: "contacts".to_string(),
        primary_key: Some("id".to_string()),
        fields: Vec::new(),
        supports_preview_read: true,
        supports_import_snapshot: true,
        capabilities: DataSourceCrudCapabilities {
            supports_list: true,
            supports_get: true,
            supports_create: true,
            supports_update: true,
            supports_delete: true,
            supports_filter: true,
            supports_sort: true,
            supports_pagination: true,
            supports_owner_filter: true,
            supports_scope_filter: true,
            supports_write: true,
            supports_transactions: true,
        },
        metadata: json!({}),
    };

    let value = serde_json::to_value(&descriptor).unwrap();
    assert_eq!(value["capabilities"]["supports_list"], true);
    assert_eq!(value["capabilities"]["supports_get"], true);
    assert_eq!(value["capabilities"]["supports_create"], true);
    assert_eq!(value["capabilities"]["supports_update"], true);
    assert_eq!(value["capabilities"]["supports_delete"], true);
    assert_eq!(value["capabilities"]["supports_filter"], true);
    assert_eq!(value["capabilities"]["supports_sort"], true);
    assert_eq!(value["capabilities"]["supports_pagination"], true);
    assert_eq!(value["capabilities"]["supports_owner_filter"], true);
    assert_eq!(value["capabilities"]["supports_scope_filter"], true);
    assert_eq!(value["capabilities"]["supports_write"], true);
    assert_eq!(value["capabilities"]["supports_transactions"], true);
}

#[test]
fn list_records_input_carries_filter_sort_pagination_and_owner_scope_context() {
    let input = DataSourceListRecordsInput {
        connection: Default::default(),
        resource_key: "contacts".to_string(),
        context: DataSourceRecordScopeContext {
            owner_id: Some("user-1".to_string()),
            scope_id: Some("workspace-1".to_string()),
        },
        filters: vec![DataSourceRecordFilter {
            field_key: "email".to_string(),
            operator: "eq".to_string(),
            value: json!("person@example.com"),
        }],
        sort: vec![DataSourceRecordSort {
            field_key: "created_at".to_string(),
            descending: true,
        }],
        page: Some(DataSourceRecordPage {
            limit: Some(50),
            cursor: Some("next".to_string()),
            offset: None,
        }),
        options_json: json!({ "include_archived": false }),
    };

    let value = serde_json::to_value(input).unwrap();
    assert_eq!(value["resource_key"], "contacts");
    assert_eq!(value["context"]["owner_id"], "user-1");
    assert_eq!(value["context"]["scope_id"], "workspace-1");
    assert_eq!(value["filters"][0]["field_key"], "email");
    assert_eq!(value["sort"][0]["descending"], true);
    assert_eq!(value["page"]["limit"], 50);
}

#[test]
fn get_record_input_json_shape_is_stable() {
    let input = DataSourceGetRecordInput {
        connection: Default::default(),
        resource_key: "contacts".to_string(),
        record_id: "contact-1".to_string(),
        context: DataSourceRecordScopeContext {
            owner_id: Some("user-1".to_string()),
            scope_id: Some("workspace-1".to_string()),
        },
        options_json: json!({ "projection": ["email"] }),
    };

    let value = serde_json::to_value(input).unwrap();
    assert_eq!(value["resource_key"], "contacts");
    assert_eq!(value["record_id"], "contact-1");
    assert_eq!(value["context"]["owner_id"], "user-1");
    assert_eq!(value["context"]["scope_id"], "workspace-1");
    assert_eq!(value["options_json"]["projection"][0], "email");
}

#[test]
fn create_record_input_json_shape_includes_record_context_and_transaction() {
    let input = DataSourceCreateRecordInput {
        connection: Default::default(),
        resource_key: "contacts".to_string(),
        record: json!({ "email": "person@example.com" }),
        context: DataSourceRecordScopeContext {
            owner_id: Some("user-1".to_string()),
            scope_id: Some("workspace-1".to_string()),
        },
        transaction_id: Some("tx-1".to_string()),
        options_json: json!({ "upsert": false }),
    };

    let value = serde_json::to_value(input).unwrap();
    assert_eq!(value["resource_key"], "contacts");
    assert_eq!(value["record"]["email"], "person@example.com");
    assert_eq!(value["context"]["owner_id"], "user-1");
    assert_eq!(value["context"]["scope_id"], "workspace-1");
    assert_eq!(value["transaction_id"], "tx-1");
    assert_eq!(value["options_json"]["upsert"], false);
}

#[test]
fn update_record_input_json_shape_includes_patch_context_and_transaction() {
    let input = DataSourceUpdateRecordInput {
        connection: Default::default(),
        resource_key: "contacts".to_string(),
        record_id: "contact-1".to_string(),
        patch: json!({ "email": "updated@example.com" }),
        context: DataSourceRecordScopeContext {
            owner_id: Some("user-1".to_string()),
            scope_id: Some("workspace-1".to_string()),
        },
        transaction_id: Some("tx-1".to_string()),
        options_json: json!({ "return_record": true }),
    };

    let value = serde_json::to_value(input).unwrap();
    assert_eq!(value["resource_key"], "contacts");
    assert_eq!(value["record_id"], "contact-1");
    assert_eq!(value["patch"]["email"], "updated@example.com");
    assert_eq!(value["context"]["owner_id"], "user-1");
    assert_eq!(value["context"]["scope_id"], "workspace-1");
    assert_eq!(value["transaction_id"], "tx-1");
    assert_eq!(value["options_json"]["return_record"], true);
}

#[test]
fn delete_record_input_can_target_owner_scope_and_transaction() {
    let input = DataSourceDeleteRecordInput {
        connection: Default::default(),
        resource_key: "contacts".to_string(),
        record_id: "contact-1".to_string(),
        context: DataSourceRecordScopeContext {
            owner_id: Some("user-1".to_string()),
            scope_id: Some("workspace-1".to_string()),
        },
        transaction_id: Some("tx-1".to_string()),
        options_json: json!({ "reason": "test" }),
    };

    let value = serde_json::to_value(input).unwrap();
    assert_eq!(value["record_id"], "contact-1");
    assert_eq!(value["context"]["owner_id"], "user-1");
    assert_eq!(value["context"]["scope_id"], "workspace-1");
    assert_eq!(value["transaction_id"], "tx-1");
}
