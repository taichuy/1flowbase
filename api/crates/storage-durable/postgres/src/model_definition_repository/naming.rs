use crate::physical_schema_repository::sanitize_identifier_fragment;
use uuid::Uuid;

pub(super) fn registered_system_table_name(
    scope_kind: domain::DataModelScopeKind,
    source_kind: domain::DataModelSourceKind,
    protection: &domain::DataModelProtection,
    code: &str,
) -> Option<&'static str> {
    if scope_kind != domain::DataModelScopeKind::System
        || source_kind != domain::DataModelSourceKind::MainSource
        || protection.owner_kind != domain::DataModelOwnerKind::Core
        || !protection.is_protected
    {
        return None;
    }

    domain::builtin_data_model_contract(code).map(|contract| contract.physical_table_name)
}

pub(super) fn is_registered_system_table(model: &domain::ModelDefinitionRecord) -> bool {
    registered_system_table_name(
        model.scope_kind,
        model.source_kind,
        &model.protection,
        &model.code,
    )
    .is_some()
}

pub(super) fn build_physical_table_name(
    scope_kind: domain::DataModelScopeKind,
    code: &str,
) -> String {
    let prefix = match scope_kind {
        domain::DataModelScopeKind::Workspace => "workspace",
        domain::DataModelScopeKind::System => "system",
    };
    let suffix = Uuid::now_v7().simple().to_string();

    format!(
        "rtm_{prefix}_{}_{}",
        &suffix[suffix.len() - 8..],
        sanitize_identifier_fragment(code)
    )
}

pub(super) fn build_physical_column_name(code: &str) -> String {
    sanitize_identifier_fragment(code)
}

pub(super) fn nullable_actor_user_id(actor_user_id: Uuid) -> Option<Uuid> {
    (!actor_user_id.is_nil()).then_some(actor_user_id)
}
