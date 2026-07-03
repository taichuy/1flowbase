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

    let user_account = crate::builtin_data_model_contract("users")
        .expect("users contract")
        .field_contract("account")
        .expect("users.account field contract");
    assert_eq!(user_account.physical_column_name, "account");
    assert_eq!(user_account.field_kind, crate::ModelFieldKind::String);
    assert!(user_account.is_required);
    assert!(user_account.is_unique);

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
