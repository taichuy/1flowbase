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
        "application_run_log_summaries",
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
        ("application_id", ManyToOne, false, false),
        ("conversation_id", String, false, false),
        ("application_name", String, true, false),
        ("attempt_index", Number, true, false),
        ("is_retry", Boolean, true, false),
        ("retry_reason", String, false, false),
        ("provider_instance_id", ManyToOne, false, false),
        ("provider_instance_display_name", String, false, false),
        ("provider_code", String, true, false),
        ("protocol", String, true, false),
        ("upstream_model_id", String, true, false),
        ("reasoning_effort", String, false, false),
        ("status", String, true, false),
        ("error_code", String, false, false),
        ("failed_after_first_token", Boolean, true, false),
        ("input_tokens", Number, false, false),
        ("output_tokens", Number, false, false),
        ("total_tokens", Number, false, false),
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

    let migration = include_str!(
        "../../../storage-durable/postgres/migrations/20260713130000_register_model_provider_request_logs_runtime_read.sql"
    );
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

        let seeded_row = migration
            .lines()
            .find(|line| {
                line.trim_start().starts_with("('") && line.contains(&format!(", '{code}', "))
            })
            .unwrap_or_else(|| panic!("missing migration seed row for {code}"));
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
    }
}
