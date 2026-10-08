use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use control_plane::agent_logs::AgentLogsService;
use control_plane_contracts::ports::{AgentLogsDeleteReceipt, AgentLogsDeleteScope};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::Serialize;
use serde_json::json;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};

pub(crate) struct LogsDeleteInput {
    pub application_id: Uuid,
    pub scope: AgentLogsDeleteScope,
}
impl InterfaceContract for LogsDeleteInput {
    const CONTRACT_ID: &'static str = "console-application-logs-delete-input";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[
            ("application_id", mp::text_schema()),
            (
                "scope",
                mp::union_schema(vec![
                    mp::object_schema(&[("mode", mp::tag_schema("all_time"))]),
                    mp::object_schema(&[
                        ("mode", mp::tag_schema("time_range")),
                        ("started_at_from", mp::text_schema()),
                        ("started_at_to", mp::text_schema()),
                    ]),
                ]),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        let scope = match &self.scope {
            AgentLogsDeleteScope::AllTime {} => {
                mp::object_value(&[("mode", serde_json::json!("all_time"))])
            }
            AgentLogsDeleteScope::TimeRange {
                started_at_from,
                started_at_to,
            } => mp::object_value(&[
                ("mode", serde_json::json!("time_range")),
                ("started_at_from", mp::text(started_at_from)?),
                ("started_at_to", mp::text(started_at_to)?),
            ]),
        };
        Some(mp::object_value(&[
            ("application_id", serde_json::json!(self.application_id)),
            ("scope", scope),
        ]))
    }
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct LogsDeleteOutput {
    pub deleted_records: u64,
}
impl From<AgentLogsDeleteReceipt> for LogsDeleteOutput {
    fn from(receipt: AgentLogsDeleteReceipt) -> Self {
        Self {
            deleted_records: receipt.deleted_records,
        }
    }
}
impl InterfaceContract for LogsDeleteOutput {
    const CONTRACT_ID: &'static str = "console-application-logs-delete-output";
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "deleted_records",
            mp::count_schema(),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"deleted_records":self.deleted_records}))
    }
}
struct LogsDeleteAdapter {
    store: MainDurableStore,
}
impl ConsoleInterfacePort<LogsDeleteInput, LogsDeleteOutput> for LogsDeleteAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: LogsDeleteInput,
    ) -> ConsoleInterfaceFuture<'a, LogsDeleteOutput> {
        Box::pin(async move {
            let actor = principal.actor();
            AgentLogsService::new(self.store.for_actor(actor.clone()))
                .delete(actor, input.application_id, input.scope)
                .await
                .map(Into::into)
                .map_err(|error| ConsoleInterfaceTargetError(error.into()))
        })
    }
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "applications.logs.delete",
    binding_id: "http.console.applications.logs.delete.v1",
    method: "DELETE",
    path: "/api/console/applications/:id/logs",
    mutating: true,
}];
pub(crate) fn compile_registry(
    store: MainDurableStore,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-logs-delete",
        "graph:console-application-logs-delete-v1",
        DECLARATIONS,
        Arc::new(LogsDeleteAdapter { store }),
    )
}

/// OpenAPI projection of the stable port DTO; the HTTP body deserializes that DTO directly.
struct LogsDeleteScopeSchema;
impl utoipa::ToSchema for LogsDeleteScopeSchema {}
impl utoipa::PartialSchema for LogsDeleteScopeSchema {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        serde_json::from_value(json!({"oneOf":[
            {"type":"object","additionalProperties":false,"required":["mode"],"properties":{"mode":{"type":"string","enum":["all_time"]}}},
            {"type":"object","additionalProperties":false,"required":["mode","started_at_from","started_at_to"],"properties":{"mode":{"type":"string","enum":["time_range"]},"started_at_from":{"type":"string","format":"date-time"},"started_at_to":{"type":"string","format":"date-time"}}}
        ]})).expect("static agent logs deletion schema")
    }
}

#[utoipa::path(
    delete,
    path = "/api/console/applications/{id}/logs",
    summary = "Delete imported agent log records",
    description = "Delete complete imported turns from an agent_logs application only. Require explicit all_time mode or time_range with RFC3339 started_at_from and started_at_to, using [from,to). Preserve the application, keys and credits.",
    request_body(content = inline(LogsDeleteScopeSchema), description = "Required typed scope: {mode: all_time} or {mode: time_range, started_at_from: RFC3339, started_at_to: RFC3339}; from must precede to.", example = json!({"mode":"time_range","started_at_from":"2026-10-07T00:00:00Z","started_at_to":"2026-10-08T00:00:00Z"})),
    params(("id" = String, Path, description = "Agent logs application ID")),
    responses((status = 200, body = LogsDeleteOutput), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 415, description = "JSON body required"), (status = 422, description = "Invalid deletion scope shape"))
)]
pub(crate) async fn delete_logs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(scope): Json<AgentLogsDeleteScope>,
) -> Result<Json<ApiSuccess<LogsDeleteOutput>>, ApiError> {
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.logs.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        LogsDeleteInput {
            application_id,
            scope,
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output)))
}
