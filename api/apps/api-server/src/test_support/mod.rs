//! Opt-in fixtures; no production constructor switches behavior with this feature.
#[path = "../_tests/application/application_runtime_routes.rs"]
pub mod application;
mod resources;
#[path = "../_tests/support/mod.rs"]
pub mod support;
pub(crate) use resources::TestResources;

pub async fn refresh_next_trace_projection(
    state: &crate::app_state::ApiState,
) -> anyhow::Result<bool> {
    crate::workers::trace_projection::refresh_next_trace_projection(state).await
}

pub async fn rebuild_trace_projection(
    store: &storage_durable_postgres::MainDurableStore,
    application_id: uuid::Uuid,
    flow_run_id: uuid::Uuid,
) -> anyhow::Result<()> {
    crate::workers::trace_projection::rebuild_trace_projection(store, application_id, flow_run_id)
        .await
}

/// Shared assembly assertions; only the thin integration target registers tests.
#[doc(hidden)]
#[cfg(not(test))]
pub mod console_route_assembly {
    pub use crate::routes::console_route_assembly::test_assertions::{
        complete_operation_specifications_match_the_production_route_set,
        migrated_assembly_contains_every_console_router_owner_assembly,
        operation_specification_exact_set_reports_every_difference,
    };
    pub use crate::routes::console_route_assembly_assertions::data_model_and_docs::data_model_docs_and_data_source_routes_compile_exact_operations;
    pub use crate::routes::console_route_assembly_assertions::infrastructure_and_mcp::infrastructure_mcp_and_user_api_key_routes_compile_exact_operations;
    pub use crate::routes::console_route_assembly_assertions::plugins_and_models::ac_002_ac_013_plugins_and_models_owner_routes_have_explicit_assembly_ownership;
    pub use crate::routes::console_route_assembly_assertions::{
        application_api_orchestration_runtime_routes_compile_exact_operations,
        applications_closed_set_rejects_duplicate_or_missing_binding,
        applications_routes_compile_exact_operations_and_resource_metadata,
        console_route_assembly_duplicate_ownership_fails_coverage,
        console_route_assembly_unclassified_route_fails_coverage,
        migrated_real_core_console_route_assembly_has_compiled_coverage,
    };
    pub use crate::routes::file_console_route_assembly_assertions::{
        file_owner_routes_compile_from_exact_assemblies,
        file_owner_routes_preserve_authenticated_data_acl_and_settings_operations,
        temporary_file_contracts_compile_without_projecting_credentials,
    };
    pub use crate::routes::settings_owner_console_route_assembly_assertions::{
        ac_001_ui_management_route_bindings_are_explicit_and_granular,
        ac_002_013_auth_center_route_bindings_are_explicit_and_stable,
        ac_002_013_members_route_bindings_are_explicit_and_stable,
        ac_002_013_permissions_route_binding_is_explicit_and_stable,
        ac_002_013_roles_route_bindings_are_explicit_and_stable,
        ac_002_013_system_route_bindings_are_explicit_and_stable,
        ac_002_013_workspace_route_bindings_are_explicit_and_stable,
        ac_002_013_workspaces_route_binding_is_explicit_and_stable,
        organization_console_route_assembly_keeps_each_operation_independent,
    };
}
