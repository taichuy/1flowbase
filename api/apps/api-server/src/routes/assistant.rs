use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json};
use control_plane::{
    application::{ApplicationNonCrudConsoleOperation, ApplicationService},
    application_public_api::{
        native::{NativeExecution, NativeObject, NativeRequestMetadata, NativeRunRequest},
        publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
        run_service::{
            native_result_from_run_detail, ApplicationPublishedRunService,
            CreateAssistantRunCommand,
        },
    },
    mcp_management::McpManagementService,
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
    profile::{ProfileService, UpdateMeMetaCommand},
};
use domain::mcp_management::McpInstanceStatus;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::{
        application_public_api::native::api_provider_runtime,
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
        mcp_management::upstream_client::{
            map_proxy_arguments, map_proxy_result, McpStreamableHttpClient,
        },
        mcp_management::{
            bindable_mcp_interface, debug_execute, McpDebugExecuteBody, McpDebugResponseMode,
        },
    },
};

const ASSISTANT_META_KEY: &str = "embedded_assistant";

#[derive(Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct AssistantPreferenceBody {
    pub application_id: Option<Uuid>,
    #[serde(default)]
    pub mcp_instance_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantPublishedFlowOption {
    pub application_id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantMcpInstanceOption {
    pub instance_id: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantSettingsResponse {
    pub preference: AssistantPreferenceBody,
    pub published_agent_flows: Vec<AssistantPublishedFlowOption>,
    pub enabled_mcp_instances: Vec<AssistantMcpInstanceOption>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartAssistantRunBody {
    pub query: String,
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssistantRunResponse {
    pub id: Uuid,
    pub application_id: Uuid,
    pub status: String,
    pub answer: Option<String>,
    pub output_payload: Value,
    pub error_payload: Option<Value>,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/assistant/settings",
            console_get(get_settings, Authenticated).patch(update_settings, Authenticated),
        )
        .route("/assistant/runs", console_post(start_run, Authenticated))
}

pub async fn get_settings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<AssistantSettingsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let preference = read_preference(
        &state
            .store
            .find_user_by_id(context.user.id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?
            .meta,
        context.actor.current_workspace_id,
    );
    let (published_agent_flows, enabled_mcp_instances) =
        available_targets(&state, context.user.id).await?;
    Ok(Json(ApiSuccess::new(AssistantSettingsResponse {
        preference,
        published_agent_flows,
        enabled_mcp_instances,
    })))
}

pub async fn update_settings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(preference): Json<AssistantPreferenceBody>,
) -> Result<Json<ApiSuccess<AssistantSettingsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    validate_preference(&state, context.user.id, &preference).await?;
    let workspace_id = context.actor.current_workspace_id;
    let meta_patch = json!({
        ASSISTANT_META_KEY: { "workspaces": { workspace_id.to_string(): preference } }
    });
    ProfileService::new(state.store.clone())
        .update_me_meta(UpdateMeMetaCommand {
            actor_user_id: context.user.id,
            tenant_id: context.actor.tenant_id,
            workspace_id,
            meta_patch,
        })
        .await?;
    let (published_agent_flows, enabled_mcp_instances) =
        available_targets(&state, context.user.id).await?;
    Ok(Json(ApiSuccess::new(AssistantSettingsResponse {
        preference,
        published_agent_flows,
        enabled_mcp_instances,
    })))
}

pub async fn start_run(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<StartAssistantRunBody>,
) -> Result<Json<ApiSuccess<AssistantRunResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    if body.query.trim().is_empty() {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("query").into());
    }
    let user = state
        .store
        .find_user_by_id(context.user.id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound("user"))?;
    let preference = read_preference(&user.meta, context.actor.current_workspace_id);
    let application_id =
        preference
            .application_id
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "assistant_application_id",
            ))?;
    validate_preference(&state, context.user.id, &preference).await?;
    ApplicationService::new(state.store.clone())
        .load_application_for_non_crud_console_operation(
            context.user.id,
            application_id,
            ApplicationNonCrudConsoleOperation::Run,
        )
        .await?;
    let catalog = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(context.user.id)
        .await?;
    let mut inputs = NativeObject::default();
    inputs.insert_value(
        "tools",
        Value::Array(assistant_provider_tools(
            &catalog,
            &preference.mcp_instance_ids,
        )),
    );
    let run = ApplicationPublishedRunService::new(state.store.clone())
        .create_assistant_run(CreateAssistantRunCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            application_id,
            request: NativeRunRequest {
                query: body.query,
                system: Vec::new(),
                model: None,
                history: body.history,
                attachments: Vec::new(),
                conversation: NativeObject::default(),
                expand_id: None,
                response_mode: None,
                stream_options: NativeObject::default(),
                execution: NativeExecution::default(),
                metadata: NativeRequestMetadata::default(),
                request_context: Default::default(),
                title: body.title,
                inputs,
                client_protocol_envelope: None,
            },
        })
        .await
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("assistant_run"))?;
    let runtime = OrchestrationRuntimeService::new(
        state.store.clone(),
        api_provider_runtime(&state),
        state.runtime_engine.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
    .with_file_storage_registry(state.file_storage_registry.clone())
    .with_llm_routing_counter_store(state.infrastructure.cache_store())
    .with_provider_request_log_queue(state.infrastructure.task_queue());
    let mut detail = runtime
        .start_published_flow_run(StartPublishedFlowRunCommand {
            application_id,
            flow_run_id: run.id,
            provider_transport_slot: None,
        })
        .await?;
    while detail.flow_run.status == domain::FlowRunStatus::WaitingCallback {
        let callback = detail
            .callback_tasks
            .iter()
            .find(|task| {
                task.status == domain::CallbackTaskStatus::Pending
                    && task.callback_kind == "llm_tool_calls"
            })
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "assistant_callback",
            ))?;
        let mut tool_results = Vec::new();
        for call in callback
            .request_payload
            .get("tool_calls")
            .and_then(Value::as_array)
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "assistant_callback",
            ))?
        {
            tool_results.push(
                assistant_tool_result(
                    &catalog,
                    &preference.mcp_instance_ids,
                    &state,
                    &headers,
                    context.user.id,
                    call,
                )
                .await?,
            );
        }
        detail = runtime
            .complete_callback_task(
                control_plane::orchestration_runtime::CompleteCallbackTaskCommand {
                    actor_user_id: context.user.id,
                    application_id,
                    callback_task_id: callback.id,
                    response_payload: json!({"tool_results": tool_results}),
                },
            )
            .await?;
    }
    let native_result = native_result_from_run_detail(&detail, json!({}));
    Ok(Json(ApiSuccess::new(AssistantRunResponse {
        id: detail.flow_run.id,
        application_id,
        status: detail.flow_run.status.as_str().to_string(),
        answer: native_result.answer,
        output_payload: detail.flow_run.output_payload,
        error_payload: detail.flow_run.error_payload,
    })))
}

fn assistant_provider_tools(
    catalog: &domain::McpCatalogSnapshot,
    selected_instance_ids: &[String],
) -> Vec<Value> {
    catalog
        .bindings
        .iter()
        .filter(|binding| binding.visible)
        .filter(|binding| {
            catalog.groups.iter().any(|group| {
                group.instance_record_id == binding.instance_record_id
                    && group.path == binding.group_path
                    && group.enabled
            })
        })
        .filter_map(|binding| {
            let instance = catalog.instances.iter().find(|instance| {
                instance.id == binding.instance_record_id
                    && instance.status == McpInstanceStatus::Enabled
                    && selected_instance_ids.contains(&instance.instance_id)
            })?;
            let tool = catalog.tools.iter().find(|tool| {
                tool.id == binding.tool_record_id && tool.status == domain::McpToolStatus::Enabled
            })?;
            Some(json!({
                "type": "function",
                "function": {
                    "name": assistant_tool_name(instance, tool),
                    "description": tool.full_description,
                    "parameters": tool.parameter_schema,
                }
            }))
        })
        .collect()
}

fn assistant_tool_name(
    instance: &domain::McpInstanceRecord,
    tool: &domain::McpToolRecord,
) -> String {
    format!("mcp_{}_{}", instance.id.simple(), tool.id.simple())
}

async fn assistant_tool_result(
    catalog: &domain::McpCatalogSnapshot,
    selected_instance_ids: &[String],
    state: &Arc<ApiState>,
    headers: &HeaderMap,
    actor_user_id: Uuid,
    call: &Value,
) -> Result<Value, ApiError> {
    let id = call.get("id").and_then(Value::as_str).ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("assistant_callback"),
    )?;
    let name = call.get("name").and_then(Value::as_str).unwrap_or_default();
    let available = catalog
        .bindings
        .iter()
        .filter(|binding| binding.visible)
        .filter(|binding| {
            catalog.groups.iter().any(|group| {
                group.instance_record_id == binding.instance_record_id
                    && group.path == binding.group_path
                    && group.enabled
            })
        })
        .find_map(|binding| {
            let instance = catalog.instances.iter().find(|instance| {
                instance.id == binding.instance_record_id
                    && instance.status == McpInstanceStatus::Enabled
                    && selected_instance_ids.contains(&instance.instance_id)
            })?;
            let tool = catalog.tools.iter().find(|tool| {
                tool.id == binding.tool_record_id && tool.status == domain::McpToolStatus::Enabled
            })?;
            (assistant_tool_name(instance, tool) == name).then_some(tool)
        });
    let Some(tool) = available else {
        return Ok(json!({"tool_call_id": id, "content": "Tool is unavailable", "is_error": true}));
    };
    let arguments = call
        .get("arguments")
        .cloned()
        .map(|value| match value {
            Value::String(value) => serde_json::from_str(&value).unwrap_or_else(|_| json!({})),
            value => value,
        })
        .unwrap_or_else(|| json!({}));
    let result = match &tool.execution_target {
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            let interface =
                bindable_mcp_interface(state.as_ref(), actor_user_id, interface_id).await;
            match interface {
                Ok(interface) => match debug_execute::execute(
                    state.clone(),
                    headers.clone(),
                    interface,
                    McpDebugExecuteBody {
                        interface_id: interface_id.clone(),
                        debug_response_mode: McpDebugResponseMode::ToolResult,
                        mcp_arguments: arguments,
                        input_mapping: tool.input_mapping.clone(),
                        output_mapping: tool.output_mapping.clone(),
                    },
                )
                .await
                {
                    Ok(value) => json!({"content": value}),
                    Err(_) => {
                        json!({"content": "MCP interface execution failed", "is_error": true})
                    }
                },
                Err(_) => json!({"content": "MCP interface is unavailable", "is_error": true}),
            }
        }
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            ..
        } => {
            let service = McpManagementService::new(state.store.clone());
            let upstream = async {
                let availability = service
                    .upstream_proxy_availability(
                        actor_user_id,
                        *upstream_connection_id,
                        remote_tool_name,
                    )
                    .await?;
                if availability != domain::McpToolAvailabilityStatus::Available {
                    anyhow::bail!("upstream tool unavailable: {}", availability.as_str());
                }
                let connection = service
                    .get_upstream_connection(actor_user_id, *upstream_connection_id)
                    .await?;
                let secret = service
                    .upstream_secret_for_execution(
                        actor_user_id,
                        *upstream_connection_id,
                        &state.provider_secret_master_key,
                    )
                    .await?;
                let remote_arguments = map_proxy_arguments(&arguments, &tool.input_mapping)?;
                let client = McpStreamableHttpClient::connect(&connection, secret.as_ref()).await?;
                let result = client.call_tool(remote_tool_name, remote_arguments).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(map_proxy_result(
                    &result,
                    &tool.output_mapping,
                )?)?)
            }
            .await;
            match upstream {
                Ok(value) => json!({"content": value}),
                Err(_) => json!({"content": "Upstream MCP execution failed", "is_error": true}),
            }
        }
    };
    Ok(
        json!({"tool_call_id": id, "name": name, "content": result["content"], "is_error": result["is_error"].as_bool().unwrap_or(false)}),
    )
}

async fn available_targets(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
) -> Result<
    (
        Vec<AssistantPublishedFlowOption>,
        Vec<AssistantMcpInstanceOption>,
    ),
    ApiError,
> {
    let applications = ApplicationService::new(state.store.clone())
        .list_applications(actor_user_id)
        .await?;
    let mut published_agent_flows = Vec::new();
    for application in applications
        .into_iter()
        .filter(|application| application.application_type == domain::ApplicationType::AgentFlow)
    {
        if ApplicationPublicationService::new(state.store.clone())
            .load_active_publication(LoadActiveApplicationPublicationCommand {
                application_id: application.id,
            })
            .await
            .is_ok()
        {
            published_agent_flows.push(AssistantPublishedFlowOption {
                application_id: application.id,
                name: application.name,
            });
        }
    }
    let catalog = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(actor_user_id)
        .await?;
    let enabled_mcp_instances = catalog
        .instances
        .into_iter()
        .filter(|instance| instance.status == McpInstanceStatus::Enabled)
        .map(|instance| AssistantMcpInstanceOption {
            instance_id: instance.instance_id,
            name: instance.name,
        })
        .collect();
    Ok((published_agent_flows, enabled_mcp_instances))
}

async fn validate_preference(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
    preference: &AssistantPreferenceBody,
) -> Result<(), ApiError> {
    if let Some(application_id) = preference.application_id {
        let application = ApplicationService::new(state.store.clone())
            .get_application(actor_user_id, application_id)
            .await?;
        if application.application_type != domain::ApplicationType::AgentFlow {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "assistant_application_id",
            )
            .into());
        }
        ApplicationPublicationService::new(state.store.clone())
            .load_active_publication(LoadActiveApplicationPublicationCommand { application_id })
            .await?;
    }
    let catalog = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(actor_user_id)
        .await?;
    for instance_id in &preference.mcp_instance_ids {
        if !catalog.instances.iter().any(|instance| {
            instance.instance_id == *instance_id && instance.status == McpInstanceStatus::Enabled
        }) {
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("mcp_instance_ids").into(),
            );
        }
    }
    Ok(())
}

fn read_preference(meta: &Value, workspace_id: Uuid) -> AssistantPreferenceBody {
    meta.get(ASSISTANT_META_KEY)
        .and_then(|value| value.get("workspaces"))
        .and_then(|value| value.get(workspace_id.to_string()))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_is_scoped_to_current_workspace() {
        let current = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);
        let application = Uuid::from_u128(3);
        let meta = json!({ ASSISTANT_META_KEY: { "workspaces": {
            current.to_string(): { "application_id": application, "mcp_instance_ids": ["alpha"] },
            other.to_string(): { "application_id": Uuid::from_u128(4), "mcp_instance_ids": ["beta"] }
        }}});
        assert_eq!(
            read_preference(&meta, current).application_id,
            Some(application)
        );
        assert_eq!(
            read_preference(&meta, current).mcp_instance_ids,
            vec!["alpha"]
        );
    }

    #[test]
    fn assistant_routes_are_authenticated_console_routes() {
        let bindings = route_assembly().bindings();
        assert!(bindings.iter().any(|binding| {
            binding.route.method == "GET"
                && binding.route.path == "/api/console/assistant/settings"
                && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
        }));
        assert!(bindings.iter().any(|binding| {
            binding.route.method == "POST"
                && binding.route.path == "/api/console/assistant/runs"
                && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
        }));
    }
}
