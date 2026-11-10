use std::collections::BTreeSet;

use access_control::ConsoleRouteOwnership;

use super::{
    compile_complete_migrated_console_operation_registry, console_health_route_assembly,
    migrated_core_console_contract_bindings, migrated_core_console_route_assembly,
    validate_operation_spec_exact_set, ConsoleRouteAssembly,
};

fn route_keys<S>(assembly: &ConsoleRouteAssembly<S>) -> BTreeSet<(String, String, String)>
where
    S: Clone + Send + Sync + 'static,
{
    assembly
        .bindings()
        .iter()
        .map(|binding| {
            (
                binding.route.method.clone(),
                binding.route.path.clone(),
                match &binding.ownership {
                    ConsoleRouteOwnership::Authenticated => "authenticated".to_string(),
                    ConsoleRouteOwnership::ConsoleOperation(operation_id) => operation_id.clone(),
                },
            )
        })
        .collect()
}

pub fn migrated_assembly_contains_every_console_router_owner_assembly() {
    let expected = ConsoleRouteAssembly::new()
        .merge(console_health_route_assembly())
        .merge(crate::routes::session::route_assembly())
        .merge(crate::routes::local_commands::route_assembly())
        .merge(crate::routes::terminal::route_assembly())
        .merge(crate::routes::me::route_assembly())
        .merge(crate::routes::external_auth::console_route_assembly())
        .merge(crate::routes::me_authentication_methods::route_assembly())
        .merge(crate::routes::assistant::route_assembly())
        .merge(crate::routes::navigation::route_assembly())
        .merge(crate::routes::application_management::route_assembly())
        .merge(crate::routes::applications::route_assembly())
        .merge(crate::routes::application_api::route_assembly())
        .merge(crate::routes::application_orchestration::route_assembly())
        .merge(crate::routes::application_runtime::route_assembly())
        .merge(crate::routes::data_models::route_assembly())
        .merge(crate::routes::docs::route_assembly())
        .merge(crate::routes::data_sources::route_assembly())
        .merge(crate::routes::files::route_assembly())
        .merge(crate::routes::file_storages::route_assembly())
        .merge(crate::routes::file_tables::route_assembly())
        .merge(crate::routes::host_infrastructure::route_assembly())
        .merge(crate::routes::runtime_i18n_catalog::route_assembly())
        .merge(crate::routes::i18n_catalog::route_assembly())
        .merge(crate::routes::mcp_management::route_assembly())
        .merge(crate::routes::user_api_keys::route_assembly())
        .merge(crate::routes::workspace::route_assembly())
        .merge(crate::routes::members::route_assembly())
        .merge(crate::routes::notifications::route_assembly())
        .merge(crate::routes::communications::route_assembly())
        .merge(crate::routes::events::route_assembly())
        .merge(crate::routes::organization::route_assembly())
        .merge(crate::routes::model_definitions::route_assembly())
        .merge(crate::routes::model_providers::route_assembly())
        .merge(crate::routes::network_center::route_assembly())
        .merge(crate::routes::frontend_block_catalog::route_assembly())
        .merge(crate::routes::js_dependencies::route_assembly())
        .merge(crate::routes::node_contributions::route_assembly())
        .merge(crate::routes::ui_management::route_assembly())
        .merge(crate::routes::roles::route_assembly())
        .merge(crate::routes::permissions::route_assembly())
        .merge(crate::routes::frontstage::route_assembly())
        .merge(crate::routes::plugins::route_assembly())
        .merge(crate::routes::auth_center::route_assembly())
        .merge(crate::routes::billing::route_assembly())
        .merge(crate::routes::branding::route_assembly())
        .merge(crate::routes::system::route_assembly())
        .merge(crate::routes::system_backups::route_assembly())
        .merge(crate::routes::system_templates::route_assembly())
        .merge(crate::routes::workspaces::route_assembly());

    assert_eq!(
        route_keys(&migrated_core_console_route_assembly()),
        route_keys(&expected)
    );
}

pub fn complete_operation_specifications_match_the_production_route_set() {
    let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let bindings = migrated_core_console_contract_bindings();

    compile_complete_migrated_console_operation_registry(&settings, &bindings, &[]).unwrap();
}

pub fn operation_specification_exact_set_reports_every_difference() {
    let error = validate_operation_spec_exact_set(
        [
            "operation.present",
            "operation.missing",
            "operation.present",
        ]
        .into_iter()
        .map(str::to_string),
        ["operation.present", "operation.extra", "operation.extra"]
            .into_iter()
            .map(str::to_string),
    )
    .unwrap_err();
    let message = error.to_string();

    assert!(message.contains("missing=[operation.missing]"));
    assert!(message.contains("extra=[operation.extra]"));
    assert!(message.contains("duplicate=[operation.extra]"));
}
