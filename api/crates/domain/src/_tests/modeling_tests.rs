use crate::{DataModelOwnerKind, DataModelSourceKind, DataModelStatus};

#[test]
fn modeling_status_values_are_stable_db_strings() {
    assert_eq!(DataModelStatus::Draft.as_str(), "draft");
    assert_eq!(DataModelStatus::Published.as_str(), "published");
    assert_eq!(DataModelStatus::Disabled.as_str(), "disabled");
    assert_eq!(DataModelStatus::Broken.as_str(), "broken");

    assert_eq!(DataModelStatus::from_db("draft"), DataModelStatus::Draft);
    assert_eq!(
        DataModelStatus::from_db("published"),
        DataModelStatus::Published
    );
    assert_eq!(
        DataModelStatus::from_db("disabled"),
        DataModelStatus::Disabled
    );
    assert_eq!(DataModelStatus::from_db("broken"), DataModelStatus::Broken);
}

#[test]
fn owner_kind_values_are_stable_db_strings() {
    assert_eq!(DataModelOwnerKind::Core.as_str(), "core");
    assert_eq!(DataModelOwnerKind::HostExtension.as_str(), "host_extension");
    assert_eq!(
        DataModelOwnerKind::RuntimeExtension.as_str(),
        "runtime_extension"
    );

    assert_eq!(
        DataModelOwnerKind::from_db("core"),
        DataModelOwnerKind::Core
    );
    assert_eq!(
        DataModelOwnerKind::from_db("host_extension"),
        DataModelOwnerKind::HostExtension
    );
    assert_eq!(
        DataModelOwnerKind::from_db("runtime_extension"),
        DataModelOwnerKind::RuntimeExtension
    );
    assert_eq!(
        DataModelOwnerKind::from_db("unknown_owner"),
        DataModelOwnerKind::Core
    );
}

#[test]
fn source_kind_values_are_stable_db_strings() {
    assert_eq!(DataModelSourceKind::MainSource.as_str(), "main_source");
    assert_eq!(
        DataModelSourceKind::ExternalSource.as_str(),
        "external_source"
    );

    assert_eq!(
        DataModelSourceKind::from_db("main_source"),
        DataModelSourceKind::MainSource
    );
    assert_eq!(
        DataModelSourceKind::from_db("external_source"),
        DataModelSourceKind::ExternalSource
    );
    assert_eq!(
        DataModelSourceKind::from_db("unknown_source"),
        DataModelSourceKind::MainSource
    );
}

#[test]
fn builtin_data_model_contract_covers_core_and_runtime_read_models() {
    let expected_codes = [
        "attachments",
        "users",
        "roles",
        "model_pricing_rules",
        "application_run_log_summaries",
        // #2035: the task projection is a builtin runtime read model.
        "application_run_log_tasks",
        "application_conversations",
        "application_conversation_messages",
        "node_runs",
        "flow_run_events",
        "flow_run_checkpoints",
        "flow_run_callback_tasks",
        // AC-009/AC-010: provider request logs are a builtin runtime read model.
        "model_provider_request_logs",
    ];

    for code in expected_codes {
        let contract = crate::builtin_data_model_contract(code)
            .unwrap_or_else(|| panic!("missing builtin data model contract for {code}"));
        assert!(!contract.capabilities.can_delete);
        assert!(!contract.capabilities.can_update_lifecycle_status);
    }

    let runtime_contract = crate::builtin_data_model_contract("application_run_log_summaries")
        .expect("runtime read model contract");
    assert!(runtime_contract.capabilities.record.can_list);
    assert!(runtime_contract.capabilities.record.can_get);
    assert!(!runtime_contract.capabilities.record.can_create);
    assert!(!runtime_contract.capabilities.record.can_update);
    assert!(!runtime_contract.capabilities.record.can_delete);
    assert!(runtime_contract.owns_field_code("created_by"));

    let request_logs_contract = crate::builtin_data_model_contract("model_provider_request_logs")
        .expect("model provider request logs runtime read contract");
    assert_eq!(
        request_logs_contract.kind,
        crate::BuiltinDataModelKind::RuntimeRead
    );
    assert_eq!(
        request_logs_contract.physical_table_name,
        "model_provider_request_logs"
    );
    assert!(request_logs_contract.owns_field_code("attempt_id"));
    assert!(request_logs_contract.owns_field_code("user_id"));
    assert!(request_logs_contract.owns_field_code("user_account"));
    assert!(request_logs_contract.owns_field_code("created_at"));
    assert!(
        request_logs_contract
            .field_contract("attempt_id")
            .expect("attempt_id field contract")
            .is_unique
    );

    let user_account = crate::builtin_data_model_contract("users")
        .expect("users contract")
        .field_contract("account")
        .expect("users.account field contract");
    assert_eq!(user_account.physical_column_name, "account");
    assert_eq!(user_account.field_kind, crate::ModelFieldKind::String);
    assert!(user_account.is_required);
    assert!(user_account.is_unique);
    for field_code in ["created_by", "updated_by"] {
        assert!(
            !crate::builtin_data_model_contract("users")
                .expect("users contract")
                .field_contract(field_code)
                .unwrap_or_else(|| panic!("users.{field_code} field contract"))
                .is_required,
            "users.{field_code} must allow system bootstrap principals without a user owner"
        );
    }

    let attachment_scope = crate::builtin_data_model_contract("attachments")
        .expect("attachments contract")
        .field_contract("scope_id")
        .expect("attachments.scope_id field contract");
    assert_eq!(attachment_scope.physical_column_name, "scope_id");
    assert_eq!(
        attachment_scope.field_kind,
        crate::ModelFieldKind::ManyToOne
    );
    assert!(attachment_scope.is_required);
    assert!(!attachment_scope.is_unique);

    let run_tokens = runtime_contract
        .field_contract("total_tokens")
        .expect("runtime token field contract");
    assert_eq!(run_tokens.physical_column_name, "total_tokens");
    assert_eq!(run_tokens.field_kind, crate::ModelFieldKind::Number);
    assert!(!run_tokens.is_required);
    assert!(!run_tokens.is_unique);
}

#[test]
fn model_provider_request_log_contract_matches_all_seeded_physical_fields() {
    // AC-009: this runtime read contract must not inherit type guesses from other models.
    use crate::ModelFieldKind::{Boolean, Datetime, ManyToOne, Number, String};

    let expected = [
        ("id", String, true, true),
        ("scope_id", ManyToOne, true, false),
        ("attempt_id", String, true, true),
        ("flow_run_id", ManyToOne, true, false),
        ("node_run_id", ManyToOne, false, false),
        ("user_id", ManyToOne, false, false),
        ("user_account", String, false, false),
        ("application_id", ManyToOne, false, false),
        ("conversation_id", String, false, false),
        ("application_name", String, true, false),
        ("attempt_index", Number, true, false),
        ("is_retry", Boolean, true, false),
        ("retry_reason", String, false, false),
        ("provider_instance_id", ManyToOne, false, false),
        ("provider_instance_display_name", String, false, false),
        ("provider_code", String, true, false),
        ("plugin_id", String, false, false),
        ("protocol", String, true, false),
        ("upstream_model_id", String, true, false),
        ("pricing_provider_code", String, false, false),
        ("pricing_model_id", String, false, false),
        ("total_cost", Number, false, false),
        ("currency_code", String, false, false),
        ("billing_status", String, false, false),
        ("reasoning_effort", String, false, false),
        ("status", String, true, false),
        ("error_code", String, false, false),
        ("failed_after_first_token", Boolean, true, false),
        ("input_tokens", Number, false, false),
        ("output_tokens", Number, false, false),
        ("total_tokens", Number, false, false),
        ("input_cache_hit_tokens", Number, false, false),
        ("input_cache_hit_rate", Number, false, false),
        ("cache_write_tokens", Number, false, false),
        ("started_at", Datetime, true, false),
        ("first_token_at", Datetime, false, false),
        ("finished_at", Datetime, false, false),
        ("time_to_first_token_ms", Number, false, false),
        ("total_duration_ms", Number, false, false),
        ("created_at", Datetime, true, false),
    ];
    let contract = crate::builtin_data_model_contract("model_provider_request_logs")
        .expect("model provider request logs contract");
    assert_eq!(contract.system_field_codes.len(), expected.len());

    let migrations = [
        include_str!(
            "../../../storage/durable/postgres/migrations/20260713130000_register_model_provider_request_logs_runtime_read.sql"
        ),
        include_str!(
            "../../../storage/durable/postgres/migrations/20260802100000_add_node_run_to_provider_request_logs.sql"
        ),
        include_str!(
            "../../../storage/durable/postgres/migrations/20260808230000_add_user_attribution_to_provider_request_logs.sql"
        ),
        include_str!(
            "../../../storage/durable/postgres/migrations/20260816130000_add_provider_request_log_plugin_cache_snapshots.sql"
        ),
        include_str!(
            "../../../storage/durable/postgres/migrations/20260818120000_add_provider_request_log_billing_snapshot.sql"
        ),
        include_str!(
            "../../../storage/durable/postgres/migrations/20260818130000_add_provider_request_log_billing_status.sql"
        ),
        include_str!(
            "../../../storage/durable/postgres/migrations/20260909080000_add_request_log_cache_write_tokens.sql"
        ),
    ];
    for (code, field_kind, is_required, is_unique) in expected {
        let field = contract
            .field_contract(code)
            .unwrap_or_else(|| panic!("missing field contract for {code}"));
        assert_eq!(field.code, code);
        assert_eq!(field.physical_column_name, code);
        assert_eq!(
            field.field_kind, field_kind,
            "field kind mismatch for {code}"
        );
        assert_eq!(
            field.is_required, is_required,
            "required mismatch for {code}"
        );
        assert_eq!(field.is_unique, is_unique, "unique mismatch for {code}");

        let seeded_row = migrations
            .iter()
            .flat_map(|migration| migration.lines())
            .find(|line| {
                line.trim_start().starts_with("('") && line.contains(&format!(", '{code}', "))
            });
        let Some(seeded_row) = seeded_row else {
            // billing_status is seeded via a dedicated single-field insert
            // whose contract flags are column-position literals, not a values
            // row.
            assert_eq!(
                code, "billing_status",
                "missing migration seed row for {code}"
            );
            let migration = migrations
                .iter()
                .find(|migration| migration.contains("'billing_status',"))
                .expect("missing billing_status seed migration");
            assert!(
                migration.contains(&format!("'{}',", field_kind.as_str())),
                "billing_status seed migration must declare kind {}",
                field_kind.as_str()
            );
            assert!(!is_required && !is_unique);
            continue;
        };
        if seeded_row.contains(" true,") || seeded_row.contains(" false,") {
            let expected_seed_contract = format!(
                ", '{}', {}, {},",
                field_kind.as_str(),
                is_required,
                is_unique
            );
            assert!(
                seeded_row.contains(&expected_seed_contract),
                "migration seed mismatch for {code}: expected {expected_seed_contract} in {seeded_row}"
            );
        } else {
            // Billing snapshot rows carry only (id, code, title, kind,
            // sort_order); is_required / is_unique are select-level `false`
            // literals in that migration.
            assert!(
                seeded_row.contains(&format!(", '{}', ", field_kind.as_str())),
                "migration seed kind mismatch for {code}: expected {} in {seeded_row}",
                field_kind.as_str()
            );
            assert!(
                !is_required && !is_unique,
                "flagless seed row for {code} only supports optional non-unique fields"
            );
        }
    }
}

#[test]
fn historical_configured_models_default_to_the_global_zero_pricing_target() {
    let model: crate::ModelProviderConfiguredModel = serde_json::from_value(serde_json::json!({
        "model_id": "legacy-model",
        "enabled": true,
        "context_window_override_tokens": null
    }))
    .unwrap();

    assert_eq!(
        model.pricing_provider_code,
        crate::DEFAULT_MODEL_PRICING_PROVIDER_CODE
    );
    assert_eq!(
        model.pricing_model_id,
        crate::DEFAULT_MODEL_PRICING_MODEL_ID
    );
}

#[test]
fn application_conversation_source_identity_is_a_read_only_builtin_contract() {
    let contract = crate::builtin_data_model_contract("application_conversations")
        .expect("canonical conversation read model");
    assert_eq!(contract.kind, crate::BuiltinDataModelKind::RuntimeRead);
    assert_eq!(
        contract.capabilities.record,
        crate::DataModelRecordCapabilities::read_only()
    );
    let expected = [
        ("source_kind", true),
        ("source_id", false),
        ("source_client", false),
        ("source_session_id", false),
    ];
    let migration=include_str!("../../../storage/durable/postgres/migrations/20261012100000_imported_canonical_conversations.sql");
    let metadata_seed = migration
        .split("-- Canonical imported identity belongs")
        .nth(1)
        .expect("conversation metadata seed");
    for (code, required) in expected {
        let field = contract
            .field_contract(code)
            .unwrap_or_else(|| panic!("missing canonical identity field {code}"));
        assert_eq!(field.physical_column_name, code);
        assert_eq!(field.field_kind, crate::ModelFieldKind::String);
        assert_eq!(
            field.is_required, required,
            "native optional source identity: {code}"
        );
        assert!(!field.is_unique, "identity uniqueness is composite: {code}");
        let seed = metadata_seed
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("('{code}',")))
            .unwrap_or_else(|| panic!("missing canonical metadata seed {code}"));
        assert!(
            seed.contains(&format!(", {required},")),
            "metadata nullable contract: {code}"
        );
    }
    assert!(metadata_seed.contains("'string',true,false,f.required,f.required,false"));
    assert!(metadata_seed.contains("and not exists(select 1 from model_fields old"));
    assert!(!metadata_seed.to_lowercase().contains("do update"));
    assert!(!metadata_seed.to_lowercase().contains("update model_fields"));
    assert!(migration.contains("add column source_kind text not null default 'native'"));
    for code in ["source_id", "source_client", "source_session_id"] {
        assert!(migration.contains(&format!("add column {code} text;")));
    }
}

#[test]
fn application_log_task_source_id_is_nullable_system_owned_string() {
    let contract = crate::builtin_data_model_contract("application_run_log_tasks").unwrap();
    assert!(contract.owns_field_code("source_id"));
    assert_eq!(
        contract.capabilities.record,
        crate::DataModelRecordCapabilities::read_only()
    );
    let field = contract.field_contract("source_id").unwrap();
    assert_eq!(field.physical_column_name, "source_id");
    assert_eq!(field.field_kind, crate::ModelFieldKind::String);
    assert!(!field.is_required);
    assert!(!field.is_unique);
    let migration=include_str!("../../../storage/durable/postgres/migrations/20261012100000_imported_canonical_conversations.sql");
    assert!(migration
        .contains("'source_id','Source ID','source_id','string',true,false,false,false,false"));
}

#[test]
fn event_configuration_models_protect_physical_contract_and_dedicated_service_writes() {
    for code in ["event_definitions", "workflow_event_triggers"] {
        let contract = crate::builtin_data_model_contract(code).unwrap();
        assert_eq!(contract.kind, crate::BuiltinDataModelKind::Core);
        assert_eq!(contract.physical_table_name, code);
        assert!(!contract.capabilities.can_delete);
        assert!(!contract.capabilities.can_add_user_field);
        assert!(!contract.capabilities.can_update_lifecycle_status);
        assert_eq!(
            contract.capabilities.record,
            crate::DataModelRecordCapabilities::read_only()
        );
        assert!(contract.owns_field_code("workspace_id"));
        assert!(!contract.owns_field_code("scope_id"));
        for field in contract.system_field_codes {
            let field_contract = contract.field_contract(field).unwrap();
            assert_eq!(
                field_contract.is_required,
                !(code == "workflow_event_triggers" && *field == "source")
            );
            assert_eq!(field_contract.physical_column_name, *field);
            assert_eq!(field_contract.is_unique, *field == "id");
        }
    }
    assert_eq!(
        crate::builtin_data_model_contract("event_definitions")
            .unwrap()
            .field_contract("payload_schema")
            .unwrap()
            .field_kind,
        crate::ModelFieldKind::Json
    );
    assert_eq!(
        crate::builtin_data_model_contract("workflow_event_triggers")
            .unwrap()
            .field_contract("input_mapping")
            .unwrap()
            .field_kind,
        crate::ModelFieldKind::Json
    );
}

#[test]
fn workflow_event_trigger_source_is_owned_nullable_json() {
    let contract = crate::builtin_data_model_contract("workflow_event_triggers").unwrap();
    assert!(contract.owns_field_code("source"));
    let field = contract.field_contract("source").unwrap();
    assert_eq!(field.field_kind, crate::ModelFieldKind::Json);
    assert!(!field.is_required);
    assert_eq!(field.physical_column_name, "source");
    assert!(!crate::builtin_data_model_contract("event_definitions")
        .unwrap()
        .owns_field_code("source"));
}
