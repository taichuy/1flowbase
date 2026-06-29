use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinDataModelKind {
    Core,
    RuntimeRead,
}

impl BuiltinDataModelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::RuntimeRead => "runtime_read",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataModelRecordCapabilities {
    pub can_list: bool,
    pub can_get: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
}

impl DataModelRecordCapabilities {
    pub const fn read_write() -> Self {
        Self {
            can_list: true,
            can_get: true,
            can_create: true,
            can_update: true,
            can_delete: true,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            can_list: true,
            can_get: true,
            can_create: false,
            can_update: false,
            can_delete: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataModelCapabilities {
    pub can_delete: bool,
    pub can_add_user_field: bool,
    pub can_update_lifecycle_status: bool,
    pub record: DataModelRecordCapabilities,
}

impl DataModelCapabilities {
    pub const fn custom() -> Self {
        Self {
            can_delete: true,
            can_add_user_field: true,
            can_update_lifecycle_status: true,
            record: DataModelRecordCapabilities::read_write(),
        }
    }

    const fn core_builtin() -> Self {
        Self {
            can_delete: false,
            can_add_user_field: true,
            can_update_lifecycle_status: false,
            record: DataModelRecordCapabilities::read_write(),
        }
    }

    const fn runtime_read_builtin() -> Self {
        Self {
            can_delete: false,
            can_add_user_field: false,
            can_update_lifecycle_status: false,
            record: DataModelRecordCapabilities::read_only(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataModelFieldOwnership {
    SystemOwned,
    UserAdded,
}

impl DataModelFieldOwnership {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemOwned => "system_owned",
            Self::UserAdded => "user_added",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataModelFieldCapabilities {
    pub ownership: DataModelFieldOwnership,
    pub can_update_presentation_metadata: bool,
    pub can_update_physical_metadata: bool,
    pub can_delete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinDataModelFieldContract {
    pub code: &'static str,
    pub physical_column_name: &'static str,
    pub field_kind: crate::ModelFieldKind,
    pub is_required: bool,
    pub is_unique: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinDataModelContract {
    pub code: &'static str,
    pub physical_table_name: &'static str,
    pub kind: BuiltinDataModelKind,
    pub system_field_codes: &'static [&'static str],
    pub capabilities: DataModelCapabilities,
}

impl BuiltinDataModelContract {
    pub fn owns_field_code(self, field_code: &str) -> bool {
        self.system_field_codes.contains(&field_code)
    }

    pub fn field_contract(self, field_code: &str) -> Option<BuiltinDataModelFieldContract> {
        if !self.owns_field_code(field_code) {
            return None;
        }

        Some(BuiltinDataModelFieldContract {
            code: field_code_to_static(self.code, field_code)?,
            physical_column_name: field_code_to_static(self.code, field_code)?,
            field_kind: builtin_field_kind(self.code, field_code),
            is_required: builtin_field_required(self.code, field_code),
            is_unique: builtin_field_unique(self.code, field_code),
        })
    }
}

fn field_code_to_static(model_code: &'static str, field_code: &str) -> Option<&'static str> {
    builtin_data_model_contract(model_code)?
        .system_field_codes
        .iter()
        .copied()
        .find(|code| *code == field_code)
}

fn builtin_field_kind(model_code: &str, field_code: &str) -> crate::ModelFieldKind {
    match field_code {
        "meta" => crate::ModelFieldKind::Json,
        "email_login_enabled"
        | "phone_login_enabled"
        | "is_builtin"
        | "is_editable"
        | "auto_grant_new_permissions"
        | "is_default_member_role" => crate::ModelFieldKind::Boolean,
        "size"
        | "total_tokens"
        | "unique_node_count"
        | "tool_callback_count"
        | "input_tokens"
        | "output_tokens"
        | "reasoning_tokens"
        | "input_cache_hit_tokens"
        | "input_cache_write_tokens"
        | "input_cache_hit_rate"
        | "sequence" => crate::ModelFieldKind::Number,
        "created_at" | "updated_at" | "started_at" | "finished_at" | "completed_at" => {
            crate::ModelFieldKind::Datetime
        }
        "introduction" | "content" | "reason" => crate::ModelFieldKind::Text,
        "scope_id"
        | "workspace_id"
        | "application_id"
        | "api_key_id"
        | "publication_version_id"
        | "conversation_id"
        | "flow_run_id"
            if model_code != "node_runs" =>
        {
            crate::ModelFieldKind::ManyToOne
        }
        "node_run_id"
            if matches!(
                model_code,
                "application_conversation_messages"
                    | "flow_run_events"
                    | "flow_run_checkpoints"
                    | "flow_run_callback_tasks"
            ) =>
        {
            crate::ModelFieldKind::ManyToOne
        }
        _ => crate::ModelFieldKind::String,
    }
}

fn builtin_field_required(model_code: &str, field_code: &str) -> bool {
    match model_code {
        "attachments" => !matches!(field_code, "title" | "extname" | "url"),
        "users" => !matches!(
            field_code,
            "phone" | "avatar_url" | "preferred_locale" | "default_display_role"
        ),
        "roles" => !matches!(field_code, "workspace_id" | "system_kind"),
        "application_run_log_summaries" => !matches!(
            field_code,
            "target_node_id"
                | "external_user"
                | "authorized_account"
                | "api_key_id"
                | "api_key_name_snapshot"
                | "publication_version_id"
                | "external_conversation_id"
                | "external_trace_id"
                | "compatibility_mode"
                | "idempotency_key"
                | "total_tokens"
                | "finished_at"
                | "input_tokens"
                | "output_tokens"
                | "reasoning_tokens"
                | "input_cache_hit_tokens"
                | "input_cache_write_tokens"
                | "input_cache_hit_rate"
        ),
        "application_conversations" => {
            !matches!(field_code, "external_user" | "api_key_id" | "title")
        }
        "application_conversation_messages" => !matches!(
            field_code,
            "flow_run_id" | "node_run_id" | "started_at" | "finished_at"
        ),
        "node_runs" => !matches!(field_code, "finished_at"),
        "flow_run_events" => !matches!(field_code, "node_run_id"),
        "flow_run_checkpoints" => !matches!(field_code, "node_run_id"),
        "flow_run_callback_tasks" => !matches!(field_code, "completed_at"),
        _ => false,
    }
}

fn builtin_field_unique(model_code: &str, field_code: &str) -> bool {
    matches!(field_code, "id")
        || matches!(
            (model_code, field_code),
            ("users", "account" | "email" | "phone")
        )
        || matches!((model_code, field_code), ("roles", "code"))
        || matches!(
            (model_code, field_code),
            ("application_run_log_summaries", "flow_run_id")
        )
}

const ATTACHMENTS_FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "created_by",
    "updated_by",
    "created_at",
    "updated_at",
    "title",
    "filename",
    "extname",
    "size",
    "mimetype",
    "path",
    "meta",
    "url",
    "storage_id",
];

const USERS_FIELDS: &[&str] = &[
    "id",
    "created_by",
    "updated_by",
    "created_at",
    "updated_at",
    "account",
    "email",
    "phone",
    "name",
    "nickname",
    "avatar_url",
    "introduction",
    "preferred_locale",
    "meta",
    "default_display_role",
    "email_login_enabled",
    "phone_login_enabled",
    "status",
];

const ROLES_FIELDS: &[&str] = &[
    "id",
    "created_by",
    "updated_by",
    "created_at",
    "updated_at",
    "scope_id",
    "scope_kind",
    "workspace_id",
    "code",
    "name",
    "introduction",
    "is_builtin",
    "is_editable",
    "auto_grant_new_permissions",
    "is_default_member_role",
    "system_kind",
];

const APPLICATION_RUN_LOG_SUMMARIES_FIELDS: &[&str] = &[
    "id",
    "flow_run_id",
    "scope_id",
    "application_id",
    "run_mode",
    "status",
    "target_node_id",
    "title",
    "external_user",
    "authorized_account",
    "api_key_id",
    "api_key_name_snapshot",
    "publication_version_id",
    "external_conversation_id",
    "external_trace_id",
    "compatibility_mode",
    "idempotency_key",
    "total_tokens",
    "unique_node_count",
    "tool_callback_count",
    "started_at",
    "finished_at",
    "created_at",
    "updated_at",
    "input_tokens",
    "output_tokens",
    "reasoning_tokens",
    "input_cache_hit_tokens",
    "input_cache_write_tokens",
    "input_cache_hit_rate",
];

const APPLICATION_CONVERSATIONS_FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "application_id",
    "external_conversation_id",
    "external_user",
    "api_key_id",
    "title",
    "created_at",
    "updated_at",
];

const APPLICATION_CONVERSATION_MESSAGES_FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "conversation_id",
    "application_id",
    "flow_run_id",
    "node_run_id",
    "role",
    "content",
    "sequence",
    "status",
    "started_at",
    "finished_at",
    "created_at",
    "updated_at",
];

const NODE_RUNS_FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "flow_run_id",
    "node_run_id",
    "node_id",
    "node_type",
    "node_alias",
    "status",
    "started_at",
    "finished_at",
    "created_at",
];

const FLOW_RUN_EVENTS_FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "flow_run_id",
    "node_run_id",
    "sequence",
    "event_type",
    "created_at",
];

const FLOW_RUN_CHECKPOINTS_FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "flow_run_id",
    "node_run_id",
    "status",
    "reason",
    "created_at",
];

const FLOW_RUN_CALLBACK_TASKS_FIELDS: &[&str] = &[
    "id",
    "scope_id",
    "flow_run_id",
    "node_run_id",
    "callback_kind",
    "status",
    "created_at",
    "completed_at",
];

pub fn builtin_data_model_contract(code: &str) -> Option<BuiltinDataModelContract> {
    let core_capabilities = DataModelCapabilities::core_builtin();
    let runtime_read_capabilities = DataModelCapabilities::runtime_read_builtin();
    Some(match code {
        "attachments" => BuiltinDataModelContract {
            code: "attachments",
            physical_table_name: "attachments",
            kind: BuiltinDataModelKind::Core,
            system_field_codes: ATTACHMENTS_FIELDS,
            capabilities: core_capabilities,
        },
        "users" => BuiltinDataModelContract {
            code: "users",
            physical_table_name: "users",
            kind: BuiltinDataModelKind::Core,
            system_field_codes: USERS_FIELDS,
            capabilities: core_capabilities,
        },
        "roles" => BuiltinDataModelContract {
            code: "roles",
            physical_table_name: "roles",
            kind: BuiltinDataModelKind::Core,
            system_field_codes: ROLES_FIELDS,
            capabilities: core_capabilities,
        },
        "application_run_log_summaries" => BuiltinDataModelContract {
            code: "application_run_log_summaries",
            physical_table_name: "application_run_log_summaries",
            kind: BuiltinDataModelKind::RuntimeRead,
            system_field_codes: APPLICATION_RUN_LOG_SUMMARIES_FIELDS,
            capabilities: runtime_read_capabilities,
        },
        "application_conversations" => BuiltinDataModelContract {
            code: "application_conversations",
            physical_table_name: "application_conversations",
            kind: BuiltinDataModelKind::RuntimeRead,
            system_field_codes: APPLICATION_CONVERSATIONS_FIELDS,
            capabilities: runtime_read_capabilities,
        },
        "application_conversation_messages" => BuiltinDataModelContract {
            code: "application_conversation_messages",
            physical_table_name: "application_conversation_messages",
            kind: BuiltinDataModelKind::RuntimeRead,
            system_field_codes: APPLICATION_CONVERSATION_MESSAGES_FIELDS,
            capabilities: runtime_read_capabilities,
        },
        "node_runs" => BuiltinDataModelContract {
            code: "node_runs",
            physical_table_name: "node_runs",
            kind: BuiltinDataModelKind::RuntimeRead,
            system_field_codes: NODE_RUNS_FIELDS,
            capabilities: runtime_read_capabilities,
        },
        "flow_run_events" => BuiltinDataModelContract {
            code: "flow_run_events",
            physical_table_name: "flow_run_events",
            kind: BuiltinDataModelKind::RuntimeRead,
            system_field_codes: FLOW_RUN_EVENTS_FIELDS,
            capabilities: runtime_read_capabilities,
        },
        "flow_run_checkpoints" => BuiltinDataModelContract {
            code: "flow_run_checkpoints",
            physical_table_name: "flow_run_checkpoints",
            kind: BuiltinDataModelKind::RuntimeRead,
            system_field_codes: FLOW_RUN_CHECKPOINTS_FIELDS,
            capabilities: runtime_read_capabilities,
        },
        "flow_run_callback_tasks" => BuiltinDataModelContract {
            code: "flow_run_callback_tasks",
            physical_table_name: "flow_run_callback_tasks",
            kind: BuiltinDataModelKind::RuntimeRead,
            system_field_codes: FLOW_RUN_CALLBACK_TASKS_FIELDS,
            capabilities: runtime_read_capabilities,
        },
        _ => return None,
    })
}

pub fn data_model_capabilities(model: &crate::ModelDefinitionRecord) -> DataModelCapabilities {
    builtin_data_model_contract(&model.code)
        .filter(|_| {
            model.scope_kind == crate::DataModelScopeKind::System
                && model.scope_id == crate::SYSTEM_SCOPE_ID
                && model.source_kind == crate::DataModelSourceKind::MainSource
                && model.protection.owner_kind == crate::DataModelOwnerKind::Core
                && model.protection.is_protected
        })
        .map(|contract| contract.capabilities)
        .unwrap_or_else(DataModelCapabilities::custom)
}

pub fn builtin_contract_for_model(
    model: &crate::ModelDefinitionRecord,
) -> Option<BuiltinDataModelContract> {
    builtin_data_model_contract(&model.code).filter(|_| {
        model.scope_kind == crate::DataModelScopeKind::System
            && model.scope_id == crate::SYSTEM_SCOPE_ID
            && model.source_kind == crate::DataModelSourceKind::MainSource
            && model.protection.owner_kind == crate::DataModelOwnerKind::Core
            && model.protection.is_protected
    })
}

pub fn data_model_field_capabilities(
    model: &crate::ModelDefinitionRecord,
    field: &crate::ModelFieldRecord,
) -> DataModelFieldCapabilities {
    let builtin_contract = builtin_contract_for_model(model);
    let system_owned = field.is_system
        || builtin_contract.is_some_and(|contract| contract.owns_field_code(&field.code));
    let ownership = if system_owned {
        DataModelFieldOwnership::SystemOwned
    } else {
        DataModelFieldOwnership::UserAdded
    };
    let builtin_system_field =
        builtin_contract.is_some_and(|contract| contract.owns_field_code(&field.code));
    let can_update_presentation_metadata = !field.is_system || builtin_system_field;
    let can_update_physical_metadata =
        !system_owned && data_model_capabilities(model).can_add_user_field;

    DataModelFieldCapabilities {
        ownership,
        can_update_presentation_metadata,
        can_update_physical_metadata,
        can_delete: can_update_physical_metadata,
    }
}
