use super::*;

pub(crate) const OPERATIONS: &[&str] = &[
    "applications.logs.otlp.export",
    "applications.logs.otlp.import",
];
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.logs.otlp.export",
        binding_id: "http.console.applications.logs.otlp.export.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/otlp/export",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.logs.otlp.import",
        binding_id: "http.console.applications.logs.otlp.import.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/otlp/import",
        mutating: true,
    },
];
pub(crate) fn compile_registry(
    state: Arc<ApiState>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry_with_event_observation(
        "api-server.console-application-log-exchange",
        "graph:console-application-log-exchange-v1",
        DECLARATIONS,
        Arc::new(ExchangeAdapter {
            store: state.store.clone(),
            files: state.file_storage_registry.clone(),
        }),
        &[crate::routes::application_public_api::agent_logs::event_projection::CONSOLE_IMPORT_OPERATION],
    )
}
