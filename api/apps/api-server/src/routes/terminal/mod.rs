use super::{
    console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    console_route_assembly::{console_post, ConsoleRouteAssembly},
};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    local_command::jobs::{JobError, JobOwner, JobSnapshot, ReadInput, ReadOutput, StartInput},
    local_terminal::{self, dto::*},
};
use axum::{extract::State, http::HeaderMap, Json};
use interface_runtime::InterfaceContract;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Weak};
use utoipa::ToSchema;
use uuid::Uuid;
mod handlers;
mod managed_projection;
#[path = "../../local_terminal/transfer.rs"]
mod transfer;
pub use handlers::*;
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InfoInput {}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JobInput {
    pub execution_id: Uuid,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JobReadInput {
    pub execution_id: Uuid,
    pub stream: crate::local_command::jobs::OutputStream,
    pub offset: u64,
    pub max_bytes: usize,
}
#[derive(Serialize, ToSchema)]
pub struct ImportOutput {
    pub file_table_id: String,
    pub storage_id: String,
    pub record: serde_json::Value,
}
pub(crate) enum TerminalInput {
    Info(InfoInput),
    List(ListInput),
    Stat(PathInput),
    Text(TextInput),
    Range(RangeInput),
    Search(SearchInput),
    Image(MediaInput),
    Audio(MediaInput),
    Resource(ResourceInput),
    Import(ImportInput),
    Download(DownloadInput),
    Start(StartInput),
    Get(JobInput),
    Read(JobReadInput),
    Cancel(JobInput),
}
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum TerminalOutput {
    Info(InfoOutput),
    List(ListOutput),
    Stat(FileMetadata),
    Text(TextOutput),
    Range(RangeOutput),
    Search(SearchOutput),
    Media(MediaOutput),
    Resource(ResourceOutput),
    Import(ImportOutput),
    Download(DownloadOutput),
    Job(JobSnapshot),
    Read(ReadOutput),
}
impl InterfaceContract for TerminalInput {
    const CONTRACT_ID: &'static str = "console-terminal-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(managed_projection::input_schema())
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(managed_projection::input(self))
    }
}
impl InterfaceContract for TerminalOutput {
    const CONTRACT_ID: &'static str = "console-terminal-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(managed_projection::output_schema())
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(managed_projection::output(self))
    }
}
struct TerminalAdapter {
    state: Weak<ApiState>,
}
impl ConsoleInterfacePort<TerminalInput, TerminalOutput> for TerminalAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a interface_runtime::UserPrincipal,
        input: TerminalInput,
    ) -> ConsoleInterfaceFuture<'a, TerminalOutput> {
        Box::pin(async move {
            self.run(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}
impl TerminalAdapter {
    async fn run(
        &self,
        principal: &interface_runtime::UserPrincipal,
        input: TerminalInput,
    ) -> Result<TerminalOutput, ApiError> {
        let state = self
            .state
            .upgrade()
            .ok_or_else(|| invalid("terminal_host_unavailable"))?;
        // Resource replay captures the selected MCP scope inside this canonical handler.
        let input = match input {
            TerminalInput::Import(body) => {
                let result = crate::resource_import::import_for_principal(
                    state,
                    principal,
                    &body.resource_uri,
                    body.file_table_id,
                )
                .await
                .map_err(|error| match error.code.as_str() {
                    "file_import_target_permission_denied" | "file_import_caller_invalid" => {
                        ApiError::from(control_plane::errors::ControlPlaneError::PermissionDenied(
                            "terminal_resource_import",
                        ))
                    }
                    _ => invalid("terminal_resource_import_failed"),
                })?;
                return Ok(TerminalOutput::Import(ImportOutput {
                    file_table_id: result.file_table_id,
                    storage_id: result.storage_id,
                    record: result.record,
                }));
            }
            TerminalInput::Download(body) => {
                return transfer::download(&state, principal.actor().clone(), body)
                    .await
                    .map(TerminalOutput::Download)
            }
            other => other,
        };
        let actor = principal.actor();
        let owner = JobOwner {
            user_id: actor.user_id,
            tenant_id: actor.tenant_id,
            current_workspace_id: actor.current_workspace_id,
        };
        let jobs = state.terminal_jobs.clone();
        tokio::task::spawn_blocking(move || match input {
            TerminalInput::Info(_) => Ok(TerminalOutput::Info(local_terminal::info())),
            TerminalInput::List(body) => local_terminal::list(body)
                .map(TerminalOutput::List)
                .map_err(file_error),
            TerminalInput::Stat(body) => local_terminal::stat(body)
                .map(TerminalOutput::Stat)
                .map_err(file_error),
            TerminalInput::Text(body) => local_terminal::text(body)
                .map(TerminalOutput::Text)
                .map_err(file_error),
            TerminalInput::Range(body) => local_terminal::range(body)
                .map(TerminalOutput::Range)
                .map_err(file_error),
            TerminalInput::Search(body) => local_terminal::search(body)
                .map(TerminalOutput::Search)
                .map_err(file_error),
            TerminalInput::Image(body) => local_terminal::media(body, true)
                .map(TerminalOutput::Media)
                .map_err(file_error),
            TerminalInput::Audio(body) => local_terminal::media(body, false)
                .map(TerminalOutput::Media)
                .map_err(file_error),
            TerminalInput::Resource(body) => local_terminal::resource(body)
                .map(TerminalOutput::Resource)
                .map_err(file_error),
            TerminalInput::Start(body) => jobs
                .start(owner, body)
                .map(TerminalOutput::Job)
                .map_err(job_error),
            TerminalInput::Get(body) => jobs
                .get(&owner, body.execution_id)
                .map(TerminalOutput::Job)
                .map_err(job_error),
            TerminalInput::Read(body) => jobs
                .read(
                    &owner,
                    body.execution_id,
                    ReadInput {
                        stream: body.stream,
                        offset: body.offset,
                        max_bytes: body.max_bytes,
                    },
                )
                .map(TerminalOutput::Read)
                .map_err(job_error),
            TerminalInput::Cancel(body) => jobs
                .cancel(&owner, body.execution_id)
                .map(TerminalOutput::Job)
                .map_err(job_error),
            TerminalInput::Import(_) | TerminalInput::Download(_) => {
                unreachable!("async transfers handled before blocking IO")
            }
        })
        .await
        .map_err(anyhow::Error::from)?
    }
}
fn job_error(error: JobError) -> ApiError {
    use control_plane::errors::ControlPlaneError;
    match error {
        JobError::InvalidInput(_) => invalid("terminal_job_input_invalid"),
        JobError::NotFound => ControlPlaneError::NotFound("terminal_job").into(),
        JobError::Conflict => ControlPlaneError::Conflict("terminal_job_request_conflict").into(),
        JobError::Io(error) => error.into(),
    }
}
pub(crate) const INTERFACE_IDS: &[&str] = &[
    "terminal.info",
    "terminal.files.list",
    "terminal.files.stat",
    "terminal.files.read_text",
    "terminal.files.read_range",
    "terminal.files.search",
    "terminal.files.read_image",
    "terminal.files.read_audio",
    "terminal.files.read_resource",
    "terminal.files.import_resource",
    "terminal.files.download_file",
    "terminal.jobs.start",
    "terminal.jobs.get",
    "terminal.jobs.read",
    "terminal.jobs.cancel",
];
const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.info",
        binding_id: "http.console.terminal.info.v1",
        method: "POST",
        path: "/api/console/terminal/info",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.list",
        binding_id: "http.console.terminal.files.list.v1",
        method: "POST",
        path: "/api/console/terminal/files/list",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.stat",
        binding_id: "http.console.terminal.files.stat.v1",
        method: "POST",
        path: "/api/console/terminal/files/stat",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.read_text",
        binding_id: "http.console.terminal.files.read_text.v1",
        method: "POST",
        path: "/api/console/terminal/files/read-text",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.read_range",
        binding_id: "http.console.terminal.files.read_range.v1",
        method: "POST",
        path: "/api/console/terminal/files/read-range",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.search",
        binding_id: "http.console.terminal.files.search.v1",
        method: "POST",
        path: "/api/console/terminal/files/search",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.read_image",
        binding_id: "http.console.terminal.files.read_image.v1",
        method: "POST",
        path: "/api/console/terminal/files/read-image",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.read_audio",
        binding_id: "http.console.terminal.files.read_audio.v1",
        method: "POST",
        path: "/api/console/terminal/files/read-audio",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.read_resource",
        binding_id: "http.console.terminal.files.read_resource.v1",
        method: "POST",
        path: "/api/console/terminal/files/read-resource",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.import_resource",
        binding_id: "http.console.terminal.files.import_resource.v1",
        method: "POST",
        path: "/api/console/terminal/files/import-resource",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.files.download_file",
        binding_id: "http.console.terminal.files.download_file.v1",
        method: "POST",
        path: "/api/console/terminal/files/download-file",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.jobs.start",
        binding_id: "http.console.terminal.jobs.start.v1",
        method: "POST",
        path: "/api/console/terminal/jobs/start",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.jobs.get",
        binding_id: "http.console.terminal.jobs.get.v1",
        method: "POST",
        path: "/api/console/terminal/jobs/get",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.jobs.read",
        binding_id: "http.console.terminal.jobs.read.v1",
        method: "POST",
        path: "/api/console/terminal/jobs/read",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "terminal.jobs.cancel",
        binding_id: "http.console.terminal.jobs.cancel.v1",
        method: "POST",
        path: "/api/console/terminal/jobs/cancel",
        mutating: true,
    },
];
pub(crate) fn compile_registry(
    state: &Arc<ApiState>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-terminal",
        "graph:console-terminal-v1",
        DECLARATIONS,
        Arc::new(TerminalAdapter {
            state: Arc::downgrade(state),
        }),
    )
}
pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/terminal/info",
            console_post(
                info,
                access_control::ConsoleRouteOwnership::ConsoleOperation("terminal.info".into()),
            ),
        )
        .route(
            "/terminal/files/list",
            console_post(
                files_list,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.list".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/stat",
            console_post(
                files_stat,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.stat".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/read-text",
            console_post(
                files_read_text,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.read_text".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/read-range",
            console_post(
                files_read_range,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.read_range".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/search",
            console_post(
                files_search,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.search".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/read-image",
            console_post(
                files_read_image,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.read_image".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/read-audio",
            console_post(
                files_read_audio,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.read_audio".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/read-resource",
            console_post(
                files_read_resource,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.read_resource".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/import-resource",
            console_post(
                files_import_resource,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.import_resource".into(),
                ),
            ),
        )
        .route(
            "/terminal/files/download-file",
            console_post(
                files_download_file,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.files.download_file".into(),
                ),
            ),
        )
        .route(
            "/terminal/jobs/start",
            console_post(
                jobs_start,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.jobs.start".into(),
                ),
            ),
        )
        .route(
            "/terminal/jobs/get",
            console_post(
                jobs_get,
                access_control::ConsoleRouteOwnership::ConsoleOperation("terminal.jobs.get".into()),
            ),
        )
        .route(
            "/terminal/jobs/read",
            console_post(
                jobs_read,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.jobs.read".into(),
                ),
            ),
        )
        .route(
            "/terminal/jobs/cancel",
            console_post(
                jobs_cancel,
                access_control::ConsoleRouteOwnership::ConsoleOperation(
                    "terminal.jobs.cancel".into(),
                ),
            ),
        )
}
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;

fn invalid(message: &'static str) -> ApiError {
    file_error(local_terminal::invalid(message))
}
fn file_error(error: local_terminal::FileError) -> ApiError {
    use control_plane::errors::ControlPlaneError;
    match error {
        local_terminal::FileError::InvalidInput(message) => {
            ControlPlaneError::InvalidInput(message).into()
        }
        local_terminal::FileError::Conflict(message) => ControlPlaneError::Conflict(message).into(),
        local_terminal::FileError::Io(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ControlPlaneError::NotFound("terminal_file").into()
        }
        local_terminal::FileError::Io(error)
            if error.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            ControlPlaneError::PermissionDenied("terminal_file").into()
        }
        local_terminal::FileError::Io(error) => error.into(),
    }
}
