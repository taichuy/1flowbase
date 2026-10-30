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
    local_command::{process::CompletedCommand, HostCommandExecutor},
};
use axum::{extract::State, http::HeaderMap, response::Response, Json};
mod response;
use control_plane::local_command::LocalCommandService;
use interface_runtime::InterfaceContract;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalCommandBody {
    /// One shell command, executed in the API host's environment.
    pub command: String,
}

#[derive(Serialize, ToSchema)]
pub struct LocalCommandResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i64>,
    pub signal: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub truncated: bool,
    pub discarded_bytes: u64,
    pub encoding_lossy: bool,
    pub output_complete: bool,
    pub cleanup_complete: bool,
}

pub(crate) struct LocalCommandExecution(CompletedCommand);

impl InterfaceContract for LocalCommandBody {
    const CONTRACT_ID: &'static str = "console-local-command-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(crate::extension_bus::managed_projection::object_schema([(
            "command_bytes",
            crate::extension_bus::managed_projection::count_schema(),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"command_bytes": self.command.len()}))
    }
}

impl InterfaceContract for LocalCommandExecution {
    const CONTRACT_ID: &'static str = "console-local-command-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema([
            ("duration_ms", mp::count_schema()),
            ("timed_out", serde_json::json!({"type":"boolean"})),
            ("truncated", serde_json::json!({"type":"boolean"})),
            ("cleanup_complete", serde_json::json!({"type":"boolean"})),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(
            serde_json::json!({"duration_ms": self.0.metadata.duration_ms, "timed_out": self.0.metadata.timed_out,
            "truncated": self.0.metadata.truncated, "cleanup_complete": self.0.metadata.cleanup_complete}),
        )
    }
}

struct LocalCommandAdapter<R>(LocalCommandService<R, HostCommandExecutor>);
impl<R: control_plane::ports::AuthRepository + 'static>
    ConsoleInterfacePort<LocalCommandBody, LocalCommandExecution> for LocalCommandAdapter<R>
{
    fn execute<'a>(
        &'a self,
        principal: &'a interface_runtime::UserPrincipal,
        input: LocalCommandBody,
    ) -> ConsoleInterfaceFuture<'a, LocalCommandExecution> {
        Box::pin(async move {
            self.0
                .execute(principal.actor(), input.command)
                .await
                .map(LocalCommandExecution)
                .map_err(|error| ConsoleInterfaceTargetError(error.into()))
        })
    }
}

pub(crate) fn compile_registry<R: control_plane::ports::AuthRepository + 'static>(
    service: LocalCommandService<R, HostCommandExecutor>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-local-commands",
        "graph:console-local-commands-v1",
        &[ConsoleInterfaceDeclaration {
            interface_id: "local_commands.execute",
            binding_id: "http.console.local-commands.execute.v1",
            method: "POST",
            path: "/api/console/local-commands/execute",
            mutating: true,
        }],
        Arc::new(LocalCommandAdapter(service)),
    )
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new().route(
        "/local-commands/execute",
        console_post(
            execute_local_command,
            access_control::ConsoleRouteOwnership::ConsoleOperation(
                "local_commands.execute".to_owned(),
            ),
        ),
    )
}

/// Execute one local shell command.
/// Requires the local command operation permission. Uses the backend OS account, with no deployment opt-in or automatic retry.
#[utoipa::path(post, path = "/api/console/local-commands/execute", request_body = LocalCommandBody,
    responses((status = 200, body = LocalCommandResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 500, body = crate::error_response::ErrorBody)))]
pub async fn execute_local_command(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<LocalCommandBody>,
) -> Result<Response, ApiError> {
    // Disconnecting HTTP does not abandon the Kernel's completion/audit owner.
    let output = tokio::spawn(async move {
        console_interface::invoke::<_, LocalCommandExecution>(
            Arc::clone(&state),
            "http.console.local-commands.execute.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state,
                headers,
            },
            body,
        )
        .await
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(response::response(output.0)?)
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
