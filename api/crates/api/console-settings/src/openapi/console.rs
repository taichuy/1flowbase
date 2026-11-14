// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::file_storages::delete_file_storage,
    crate::routes::file_storages::update_file_storage,
    crate::routes::file_tables::delete_file_table,
))]
struct Paths8;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::system::terminate_runtime_process,
    crate::routes::system::get_runtime_processes,
    crate::routes::billing::list_pricing_rules,
    crate::routes::billing::create_pricing_rule,
    crate::routes::billing::update_pricing_rule,
    crate::routes::billing::delete_pricing_rule,
    crate::routes::billing::get_pricing_catalog,
    crate::routes::billing::import_pricing_catalog,
    crate::routes::billing::sync_pricing_catalog,
    crate::routes::billing::list_credit_accounts,
    crate::routes::billing::get_credit_account,
    crate::routes::billing::list_credit_ledger,
    crate::routes::billing::grant_credit,
    crate::routes::billing::charge_credit,
    crate::routes::billing::adjust_credit,
    crate::routes::billing::enable_charge,
    crate::routes::billing::disable_charge,
    crate::routes::billing::refund_credit,
))]
struct Paths18;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::docs::get_docs_catalog,
    crate::routes::docs::get_category_operations,
    crate::routes::docs::get_category_openapi,
    crate::routes::docs::get_operation_openapi,
))]
struct Paths68;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::branding::get_system_settings,
    crate::routes::branding::save_system_settings,
    crate::routes::branding::public_interface::get_system_metadata,
))]
struct Paths88;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::auth_center::get_auth_center_overview,
    crate::routes::auth_center::create_auth_center_authenticator,
    crate::routes::auth_center::copy_auth_center_authenticator,
    crate::routes::auth_center::delete_auth_center_authenticator,
    crate::routes::auth_center::reorder_auth_center_authenticators,
    crate::routes::auth_center::update_auth_center_authenticator_enabled,
    crate::routes::auth_center::update_auth_center_authenticator_config,
    crate::routes::auth_center::update_auth_center_authenticator_public_ui_block,
    crate::routes::system::get_release_status,
    crate::routes::system::get_runtime_profile,
    crate::routes::system_templates::catalog,
    crate::routes::system_templates::export,
    crate::routes::system_templates::download,
    crate::routes::system_templates::preview,
    crate::routes::system_templates::install,
    crate::routes::system_backups::get_catalog,
    crate::routes::system_backups::list_backups,
    crate::routes::system_backups::create_backup,
    crate::routes::system_backups::get_backup_job_status,
    crate::routes::system_backups::get_backup,
    crate::routes::system_backups::delete_backup,
    crate::routes::system_backups::verify_backup,
    crate::routes::system_backups::import_backup,
    crate::routes::system_backups::download_backup,
    crate::routes::system_backups::preflight_recovery,
    crate::routes::system_backups::reauthenticate_recovery,
    crate::routes::system_backups::create_recovery_intent,
    crate::routes::system_backups::get_recovery_status,
))]
struct Paths108;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::workspace::get_workspace,
    crate::routes::workspace::patch_workspace,
    crate::routes::i18n_catalog::get_i18n_catalog_state,
    crate::routes::i18n_catalog::get_i18n_catalog_update_status,
    crate::routes::i18n_catalog::activate_i18n_catalog_update,
    crate::routes::i18n_catalog::preview_installed_i18n_catalog,
    crate::routes::i18n_catalog::activate_installed_i18n_catalog,
    crate::routes::i18n_catalog::management::list_catalog_entries,
    crate::routes::i18n_catalog::management::get_catalog_entry,
    crate::routes::i18n_catalog::management::upsert_catalog_override,
    crate::routes::i18n_catalog::management::restore_catalog_override,
    crate::routes::i18n_catalog::management::upsert_custom_catalog_translation,
    crate::routes::i18n_catalog::management::delete_custom_catalog_key,
    crate::routes::i18n_catalog::management::restore_all_catalog_overrides,
    crate::routes::runtime_i18n_catalog::get_runtime_i18n_catalog,
    crate::routes::workspaces::list_workspaces,
    crate::routes::organization::access,
    crate::routes::organization::list,
    crate::routes::organization::create,
    crate::routes::organization::update,
    crate::routes::organization::delete,
    crate::routes::organization::replace,
    crate::routes::members::list_member_role_options,
    crate::routes::members::list_members,
    crate::routes::members::create_member,
    crate::routes::members::update_member,
    crate::routes::members::delete_member,
    crate::routes::members::disable_member,
    crate::routes::members::enable_member,
    crate::routes::members::reset_member,
    crate::routes::members::replace_member_roles,
))]
struct Paths165;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::roles::list_roles,
    crate::routes::roles::create_role,
    crate::routes::roles::update_role,
    crate::routes::roles::delete_role,
    crate::routes::roles::get_role_console_policy,
    crate::routes::roles::replace_role_console_policy,
    crate::routes::roles::get_role_frontstage_routes,
    crate::routes::roles::replace_role_frontstage_routes,
    crate::routes::roles::get_role_permissions,
    crate::routes::roles::replace_role_permissions,
    crate::routes::roles::get_role_data_policy,
    crate::routes::roles::replace_role_data_policy,
    crate::routes::roles::list_data_model_options,
    crate::routes::roles::get_console_policy_catalog,
    crate::routes::roles::replace_console_settings_order,
    crate::routes::navigation::get_console_navigation,
    crate::routes::permissions::list_permissions,
))]
struct Paths241;

#[derive(OpenApi)]
#[openapi(paths(crate::routes::docs::get_data_model_openapi,))]
struct Paths272;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::auth_center::AuthCenterLoginEntryResponse,
    crate::routes::auth_center::AuthCenterConfigFieldResponse,
    crate::routes::auth_center::AuthCenterOverviewResponse,
    crate::routes::auth_center::CreateAuthCenterLoginEntryBody,
    crate::routes::auth_center::CopyAuthCenterLoginEntryBody,
    crate::routes::auth_center::ReorderAuthCenterLoginEntriesBody,
    crate::routes::auth_center::UpdateAuthCenterLoginEntryConfigBody,
    crate::routes::auth_center::UpdateAuthCenterLoginEntryPublicUiBlockBody,
)))]
struct Schemas16;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::i18n_catalog::management::CatalogManagementOriginDto,
    crate::routes::i18n_catalog::management::ListCatalogEntriesQuery,
    crate::routes::i18n_catalog::management::GetCatalogEntryQuery,
    crate::routes::i18n_catalog::management::UpsertCatalogTranslationBody,
    crate::routes::i18n_catalog::management::RestoreCatalogOverrideBody,
    crate::routes::i18n_catalog::management::DeleteCustomCatalogKeyBody,
    crate::routes::i18n_catalog::management::RestoreCatalogOverridesBody,
    crate::routes::i18n_catalog::management::CatalogManagementEntryResponse,
    crate::routes::i18n_catalog::management::CatalogManagementPageResponse,
    crate::routes::i18n_catalog::management::CatalogEntryMutationResponse,
    crate::routes::i18n_catalog::management::CatalogRevisionResponse,
    crate::routes::runtime_i18n_catalog::RuntimeI18nCatalogResponse,
    crate::routes::members::CreateMemberBody,
    crate::routes::members::MemberResponse,
    crate::routes::members::MemberRoleOptionResponse,
    crate::routes::organization::SaveDepartmentBody,
    crate::routes::organization::DepartmentResponse,
    crate::routes::organization::DepartmentTreeItemResponse,
    crate::routes::organization::DepartmentPageResponse,
    crate::routes::organization::OrganizationAccessResponse,
    crate::routes::organization::ReplaceMemberDepartmentsBody,
    crate::routes::members::ReplaceMemberRolesBody,
    crate::routes::members::ResetMemberPasswordBody,
)))]
struct Schemas44;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::navigation::ConsoleNavigationItemResponse,
    crate::routes::navigation::ConsoleNavigationResponse,
    crate::routes::navigation::ConsolePermissionBindingResponse,
    crate::routes::navigation::ConsoleRouteDefinitionResponse,
    crate::routes::permissions::PermissionResponse,
    crate::routes::docs::DataModelOpenApiDocumentResponse,
    crate::routes::roles::CreateRoleBody,
    crate::routes::roles::ReplaceConsoleSettingsOrderBody,
    crate::routes::roles::ConsolePolicyCatalogResponse,
    crate::routes::roles::ReplaceRoleConsolePolicyBody,
    crate::routes::roles::ConsoleRoleConsolePolicyGroupBody,
    crate::routes::roles::ConsoleRoleConsolePolicyOperationBody,
    crate::routes::roles::ConsolePolicyGroupKindBody,
    crate::routes::roles::ConsolePolicyStrategyBody,
    crate::routes::roles::ConsolePolicyRowScopeBody,
    crate::routes::roles::RoleConsolePolicyResponse,
    crate::routes::roles::RoleConsolePolicyGroupResponse,
    crate::routes::roles::RoleConsolePolicyOperationResponse,
    crate::routes::roles::ReplaceRoleFrontstageRoutesBody,
    crate::routes::roles::RoleFrontstageRoutesResponse,
    crate::routes::roles::RoleFrontstageRouteNodeResponse,
    crate::routes::roles::ReplaceRoleDataPolicyBody,
    crate::routes::roles::ReplaceRolePermissionsBody,
    crate::routes::roles::RoleDataModelPolicyBody,
    crate::routes::roles::RoleDataPolicyBody,
    crate::routes::roles::RoleDataPolicyResponse,
    crate::routes::roles::RoleDataModelOptionResponse,
    crate::routes::roles::RolePermissionsResponse,
    crate::routes::roles::RoleResponse,
    crate::routes::roles::UpdateRoleBody,
)))]
struct Schemas88;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::system::LocaleMetaResponse,
    crate::routes::system::LocaleSourceResponse,
    crate::routes::system::ConsoleReleaseInfoResponse,
    crate::routes::system::ConsoleReleaseStatusResponse,
    crate::routes::system::ConsoleReleaseUpgradeCommandsResponse,
    crate::routes::system::SystemRuntimeCpuResponse,
    crate::routes::system::SystemRuntimeHostResponse,
    crate::routes::system::SystemRuntimeMemoryResponse,
    crate::routes::system::SystemRuntimePlatformResponse,
    crate::routes::system::SystemRuntimeProfileResponse,
    crate::routes::system::SystemRuntimeRelationship,
    crate::routes::system::SystemRuntimeServiceResponse,
    crate::routes::system::SystemRuntimeServicesResponse,
    crate::routes::system::SystemRuntimeTopologyResponse,
    crate::routes::workspace::PatchWorkspaceBody,
    crate::routes::workspace::WorkspaceResponse,
    crate::routes::workspaces::WorkspaceSummaryResponse,
)))]
struct Schemas174;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![
        (8, Paths8::openapi()),
        (18, Paths18::openapi()),
        (68, Paths68::openapi()),
        (88, Paths88::openapi()),
        (108, Paths108::openapi()),
        (165, Paths165::openapi()),
        (241, Paths241::openapi()),
        (272, Paths272::openapi()),
    ]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![
        (16, Schemas16::openapi()),
        (44, Schemas44::openapi()),
        (88, Schemas88::openapi()),
        (174, Schemas174::openapi()),
    ]
}
