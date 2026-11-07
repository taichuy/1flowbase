use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};
use interface_runtime::{InterfaceContract, UserPrincipal};

#[derive(Clone)]
pub(crate) struct McpDirectoryDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
}

pub(crate) enum McpDirectoryInput {
    Read(String),
    Replace(String, ReplaceMcpInstanceDirectoryBody),
}

pub(crate) struct McpDirectoryOutput(pub(super) McpInstanceDirectoryResponse);

impl InterfaceContract for McpDirectoryInput {
    const CONTRACT_ID: &'static str = "console-mcp-directory-input";
    const CONTRACT_VERSION: &'static str = "1";

    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([
            ("action", mp::text_schema()),
            ("instance_id", mp::text_schema()),
            ("group_count", mp::count_schema()),
            ("binding_count", mp::count_schema()),
        ]))
    }

    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        let (action, instance, groups, bindings) = match self {
            Self::Read(instance) => ("read", instance, 0, 0),
            Self::Replace(instance, body) => {
                ("replace", instance, body.groups.len(), body.bindings.len())
            }
        };
        Some(mp::object_value([
            ("action", mp::text(action)?),
            ("instance_id", mp::text(instance)?),
            ("group_count", serde_json::json!(groups)),
            ("binding_count", serde_json::json!(bindings)),
        ]))
    }
}

impl InterfaceContract for McpDirectoryOutput {
    const CONTRACT_ID: &'static str = "console-mcp-directory-output";
    const CONTRACT_VERSION: &'static str = "1";

    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([
            ("instance_record_id", mp::text_schema()),
            ("group_count", mp::count_schema()),
            ("binding_count", mp::count_schema()),
        ]))
    }

    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value([
            ("instance_record_id", mp::text(&self.0.instance_record_id)?),
            ("group_count", serde_json::json!(self.0.groups.len())),
            ("binding_count", serde_json::json!(self.0.bindings.len())),
        ]))
    }
}

struct McpDirectoryAdapter(McpDirectoryDependencies);

impl ConsoleInterfacePort<McpDirectoryInput, McpDirectoryOutput> for McpDirectoryAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpDirectoryInput,
    ) -> ConsoleInterfaceFuture<'a, McpDirectoryOutput> {
        Box::pin(async move {
            let service = McpManagementService::new(self.0.store.clone());
            let result: Result<_, ApiError> = async {
                let snapshot = match input {
                    McpDirectoryInput::Read(instance) => {
                        service
                            .read_instance_directory_for_actor(principal.actor(), &instance)
                            .await?
                    }
                    McpDirectoryInput::Replace(instance, body) => {
                        service
                            .replace_instance_directory_for_actor(
                                principal.actor(),
                                to_command(instance, body)?,
                            )
                            .await?
                    }
                };
                Ok(McpDirectoryOutput(to_response(snapshot)))
            }
            .await;
            result.map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.directory.view",
        binding_id: "http.console.mcp.directory.get.v1",
        method: "GET",
        path: "/api/console/mcp/instances/:instance_id/directory",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.directory.replace",
        binding_id: "http.console.mcp.directory.put.v1",
        method: "PUT",
        path: "/api/console/mcp/instances/:instance_id/directory",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: McpDirectoryDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-directory",
        "graph:console-mcp-directory-v1",
        DECLARATIONS,
        Arc::new(McpDirectoryAdapter(dependencies)),
    )
}
