use super::{
    console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    console_route_assembly::{console_post, ConsoleRouteAssembly},
};
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{extract::State, http::HeaderMap, Json};
use control_plane::local_command::{LocalCommandOutput, LocalCommandService};
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
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub truncated: bool,
    pub discarded_bytes: u64,
    pub encoding_lossy: bool,
    pub output_complete: bool,
    pub cleanup_complete: bool,
}

impl From<LocalCommandOutput> for LocalCommandResponse {
    fn from(value: LocalCommandOutput) -> Self {
        let LocalCommandOutput {
            stdout,
            stderr,
            exit_code,
            signal,
            duration_ms,
            timed_out,
            truncated,
            discarded_bytes,
            encoding_lossy,
            output_complete,
            cleanup_complete,
        } = value;
        Self {
            stdout,
            stderr,
            exit_code,
            signal,
            duration_ms,
            timed_out,
            truncated,
            discarded_bytes,
            encoding_lossy,
            output_complete,
            cleanup_complete,
        }
    }
}

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

impl InterfaceContract for LocalCommandResponse {
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
            serde_json::json!({"duration_ms": self.duration_ms, "timed_out": self.timed_out,
            "truncated": self.truncated, "cleanup_complete": self.cleanup_complete}),
        )
    }
}

struct LocalCommandAdapter<R>(LocalCommandService<R>);
impl<R: control_plane::ports::AuthRepository + 'static>
    ConsoleInterfacePort<LocalCommandBody, LocalCommandResponse> for LocalCommandAdapter<R>
{
    fn execute<'a>(
        &'a self,
        principal: &'a interface_runtime::UserPrincipal,
        input: LocalCommandBody,
    ) -> ConsoleInterfaceFuture<'a, LocalCommandResponse> {
        Box::pin(async move {
            self.0
                .execute(principal.actor(), input.command)
                .await
                .map(LocalCommandResponse::from)
                .map_err(|error| ConsoleInterfaceTargetError(error.into()))
        })
    }
}

pub(crate) fn compile_registry<R: control_plane::ports::AuthRepository + 'static>(
    service: LocalCommandService<R>,
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
/// System root only. Requires deployment configuration. Uses the backend OS account and does not retry.
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
) -> Result<Json<ApiSuccess<LocalCommandResponse>>, ApiError> {
    // Disconnecting HTTP does not abandon the Kernel's completion/audit owner.
    let output = tokio::spawn(async move {
        console_interface::invoke::<_, LocalCommandResponse>(
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
    Ok(Json(ApiSuccess::new(output)))
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
