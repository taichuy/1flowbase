use domain::DataModelScopeKind;

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

pub(super) fn build_physical_table_name(_scope_kind: DataModelScopeKind, code: &str) -> String {
    code.to_string()
}

pub(super) fn build_physical_column_name(code: &str) -> String {
    code.replace('-', "_")
}
