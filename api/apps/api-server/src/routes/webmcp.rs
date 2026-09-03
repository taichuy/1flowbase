use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap},
    routing::{get, post},
    Json, Router,
};
use control_plane::mcp_management::{
    mcp_llm_instance_registration, McpLlmOperation, McpManagementService,
};
use domain::{McpInstanceStatus, WebMcpExposure};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{
        require_csrf::require_csrf,
        require_session::{require_session, with_server_delegated_request_context, RequestContext},
    },
    response::ApiSuccess,
    routes::{mcp_management, mcp_protocol::virtual_ui},
};

#[derive(Debug, Serialize)]
pub struct WebMcpRegistrationResponse {
    pub instance_id: String,
    pub tools: Vec<WebMcpToolResponse>,
}

#[derive(Debug, Serialize)]
pub struct WebMcpToolResponse {
    pub operation: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub annotations: WebMcpToolAnnotationsResponse,
}

#[derive(Debug, Serialize)]
pub struct WebMcpToolAnnotationsResponse {
    pub read_only_hint: bool,
    pub untrusted_content_hint: bool,
}

#[derive(Debug, Deserialize)]
pub struct InvokeWebMcpToolBody {
    #[serde(default = "empty_arguments")]
    pub arguments: Value,
}

#[derive(Debug, Serialize)]
pub struct InvokeWebMcpToolResponse {
    pub content: Value,
    pub is_error: bool,
}

fn empty_arguments() -> Value {
    json!({})
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/webmcp/registrations", get(list_registrations))
        .route("/webmcp/:instance_id/tools/:operation", post(invoke_tool))
}

async fn list_registrations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<WebMcpRegistrationResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    let catalog = McpManagementService::new(state.store.clone())
        .read_catalog_for_actor(&context.actor)
        .await?;
    let registrations = catalog
        .instances
        .iter()
        .filter(|instance| {
            instance.status == McpInstanceStatus::Enabled
                && instance.webmcp_exposure == WebMcpExposure::AuthenticatedSession
        })
        .map(|instance| {
            let tools = mcp_llm_instance_registration(&instance.instance_id)
                .into_iter()
                .map(|mut registration| {
                    virtual_ui::apply_catalog_registration_capabilities(
                        &catalog,
                        &mut registration,
                    );
                    let function = &registration.provider_tool["function"];
                    WebMcpToolResponse {
                        operation: registration.operation.as_str().to_string(),
                        name: registration.provider_name,
                        title: webmcp_tool_title(&instance.name, registration.operation),
                        description: function["description"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        input_schema: function["parameters"].clone(),
                        annotations: WebMcpToolAnnotationsResponse {
                            read_only_hint: registration.operation != McpLlmOperation::Call,
                            untrusted_content_hint: true,
                        },
                    }
                })
                .collect();
            WebMcpRegistrationResponse {
                instance_id: instance.instance_id.clone(),
                tools,
            }
        })
        .collect();
    Ok(Json(ApiSuccess::new(registrations)))
}

async fn invoke_tool(
    State(state): State<Arc<ApiState>>,
    Path((instance_id, operation)): Path<(String, String)>,
    mut headers: HeaderMap,
    Json(body): Json<InvokeWebMcpToolBody>,
) -> Result<Json<ApiSuccess<InvokeWebMcpToolResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    context.cookie_session()?;
    require_csrf(&headers, &context)?;
    let actor = context.actor.clone();
    let catalog = McpManagementService::new(state.store.clone())
        .read_catalog_for_actor(&actor)
        .await?;
    let instance = catalog
        .instances
        .iter()
        .find(|instance| {
            instance.instance_id == instance_id
                && instance.status == McpInstanceStatus::Enabled
                && instance.webmcp_exposure == WebMcpExposure::AuthenticatedSession
        })
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "webmcp_registration",
        ))?;
    let registration = mcp_llm_instance_registration(&instance.instance_id)
        .into_iter()
        .find(|registration| registration.operation.as_str() == operation)
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
            "webmcp_operation",
        ))?;

    let interface_catalog = virtual_ui::McpInterfaceCatalogSnapshot::new(
        mcp_management::mcp_interface_catalog_entries(&state, &actor).await?,
    );
    let interface_dispatch = virtual_ui::ConsoleRouterMcpInterfaceDispatchPort::new(
        crate::console_router(Arc::clone(&state), true),
    );
    let dependencies = virtual_ui::RuntimeInternalToolInvokerDependencies::new(
        state.store.clone(),
        state.infrastructure.cache_store(),
        state.provider_secret_master_key.clone(),
    );
    headers.remove(AUTHORIZATION);
    headers.remove(axum::http::header::COOKIE);
    headers.remove("x-csrf-token");
    let request_context = RequestContext::server_delegation(context.user, actor.clone());
    let outcome = with_server_delegated_request_context(
        request_context,
        virtual_ui::dispatch(
            &dependencies,
            &interface_catalog,
            &interface_dispatch,
            &headers,
            &actor,
            &catalog,
            &virtual_ui::VirtualMcpScope::single(instance_id),
            &registration.provider_name,
            body.arguments,
            None,
        ),
    )
    .await?;
    let response = match outcome {
        virtual_ui::VirtualToolOutcome::Success(content) => InvokeWebMcpToolResponse {
            content,
            is_error: false,
        },
        virtual_ui::VirtualToolOutcome::Error {
            code,
            message,
            data,
        } => InvokeWebMcpToolResponse {
            content: json!({"code": code, "message": message, "data": data}),
            is_error: true,
        },
    };
    Ok(Json(ApiSuccess::new(response)))
}

fn webmcp_tool_title(instance_name: &str, operation: McpLlmOperation) -> String {
    let action = match operation {
        McpLlmOperation::List => "Browse tools",
        McpLlmOperation::Get => "Inspect tool",
        McpLlmOperation::Result => "Continue result",
        McpLlmOperation::Call => "Call tool",
    };
    format!("{instance_name}: {action}")
}
