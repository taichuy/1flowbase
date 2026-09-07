use super::{
    interface::{WebMcpFuture, WebMcpInput, WebMcpOutput, WebMcpPort},
    InvokeWebMcpToolResponse, WebMcpRegistrationResponse, WebMcpToolAnnotationsResponse,
    WebMcpToolResponse,
};
use crate::{
    error_response::ApiError,
    middleware::require_session::{with_server_delegated_request_context, RequestContext},
    routes::mcp_protocol::{virtual_ui, McpToolCallDependencies},
};
use axum::http::HeaderMap;
use control_plane::mcp_management::{
    mcp_llm_instance_registration, McpLlmOperation, McpManagementService,
};
use domain::{McpInstanceStatus, WebMcpExposure};
use interface_runtime::UserPrincipal;
use serde_json::{json, Value};
use std::sync::Arc;
use storage_durable_postgres::MainDurableStore;

struct WebMcpAdapter {
    store: MainDurableStore,
    dispatch: McpToolCallDependencies,
}

pub(crate) fn port(
    store: MainDurableStore,
    dispatch: McpToolCallDependencies,
) -> Arc<dyn WebMcpPort> {
    Arc::new(WebMcpAdapter { store, dispatch })
}

impl WebMcpPort for WebMcpAdapter {
    fn execute<'a>(&'a self, principal: &'a UserPrincipal, input: WebMcpInput) -> WebMcpFuture<'a> {
        Box::pin(async move {
            match input {
                WebMcpInput::Registrations => self.registrations(principal).await,
                WebMcpInput::Tool {
                    instance_id,
                    operation,
                    arguments,
                    forward_headers,
                } => {
                    self.tool(
                        principal,
                        instance_id,
                        operation,
                        arguments,
                        forward_headers,
                    )
                    .await
                }
            }
        })
    }
}

impl WebMcpAdapter {
    async fn registrations(&self, principal: &UserPrincipal) -> Result<WebMcpOutput, ApiError> {
        let catalog = McpManagementService::new(self.store.clone())
            .read_catalog_for_actor(principal.actor())
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
        Ok(WebMcpOutput::Registrations(registrations))
    }
    async fn tool(
        &self,
        principal: &UserPrincipal,
        instance_id: String,
        operation: String,
        arguments: Value,
        forward_headers: Vec<super::interface::WebMcpForwardHeader>,
    ) -> Result<WebMcpOutput, ApiError> {
        let actor = principal.actor().clone();
        let catalog = McpManagementService::new(self.store.clone())
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

        let catalog_dependencies = self
            .dispatch
            .interface_catalog
            .with_interface_registry_snapshot(self.dispatch.interface_registry.snapshot());
        let interface_catalog = virtual_ui::McpInterfaceCatalogSnapshot::new(
            crate::routes::mcp_management::interface_catalog::mcp_interface_catalog_entries_with(
                &catalog_dependencies,
                &actor,
            )
            .await?,
        );
        let mut headers = HeaderMap::new();
        for header in forward_headers {
            headers.append(
                axum::http::HeaderName::from_bytes(header.name.as_bytes())?,
                axum::http::HeaderValue::from_bytes(&header.value)?,
            );
        }
        let user = self
            .store
            .find_user_by_id(actor.user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;
        let request_context = RequestContext::server_delegation(user, actor.clone());
        let outcome = with_server_delegated_request_context(
            request_context,
            virtual_ui::dispatch(
                &self.dispatch.runtime_dependencies,
                &interface_catalog,
                self.dispatch.interface_dispatch.as_ref(),
                &headers,
                &actor,
                &catalog,
                &virtual_ui::VirtualMcpScope::single(instance_id),
                &registration.provider_name,
                arguments,
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
        Ok(WebMcpOutput::Tool(response))
    }
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
