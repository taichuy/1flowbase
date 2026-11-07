//! Generic settings API for plugin-declared record permissions.
use super::{record_acl, ManagedServiceRegistration};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::{
        console_interface::{
            self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
            ConsoleInterfaceTargetError,
        },
        console_route_assembly::*,
    },
};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use control_plane::errors::ControlPlaneError as Error;
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Weak};

#[derive(Clone, Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecordQuery {
    pub plugin_code: String,
    pub resource_code: String,
    pub record_id: String,
}
#[derive(Clone, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CapabilitiesBody {
    pub plugin_code: String,
    pub resource_code: String,
    pub record_ids: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GrantBody {
    pub role_id: uuid::Uuid,
    pub actions: Vec<String>,
}
#[derive(Clone, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplaceBody {
    #[schema(value_type=String, example="restricted")]
    pub mode: domain::RecordPermissionMode,
    pub grants: Vec<GrantBody>,
    pub expected_revision: Option<i64>,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct ActionResponse {
    pub action_code: String,
    pub label: String,
    pub description: String,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct ResourceResponse {
    pub resource_code: String,
    pub label: String,
    pub description: String,
    pub actions: Vec<ActionResponse>,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct SavedPolicyResponse {
    #[schema(value_type=String)]
    pub mode: domain::RecordPermissionMode,
    #[schema(value_type=Vec<GrantBody>)]
    pub grants: Vec<domain::RecordPermissionGrant>,
    pub revision: i64,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct RoleResponse {
    pub role_id: uuid::Uuid,
    pub code: String,
    pub name: String,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct CapabilityResponse {
    pub record_id: String,
    pub allowed_actions: Vec<String>,
    pub can_manage_permissions: bool,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct CapabilitiesResponse {
    pub items: Vec<CapabilityResponse>,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct PolicyResponse {
    pub resource: ResourceResponse,
    #[schema(value_type=Vec<RoleResponse>)]
    pub roles: Vec<domain::RecordPermissionRole>,
    #[schema(value_type=String)]
    pub mode: domain::RecordPermissionMode,
    #[schema(value_type=Vec<GrantBody>)]
    pub grants: Vec<domain::RecordPermissionGrant>,
    pub revision: i64,
}
pub(crate) enum Input {
    Get(RecordQuery),
    Replace(RecordQuery, ReplaceBody),
    Capabilities(CapabilitiesBody),
}
pub(crate) struct Output(Value);
macro_rules! safe_contract { ($type:ty, $id:literal) => { impl InterfaceContract for $type {
    const CONTRACT_ID: &'static str = $id;
    const CONTRACT_VERSION: &'static str = "1";
    fn managed_projection_schema() -> Option<Value> { Some(json!({"type":"object","properties":{"record_permissions":{"type":"boolean"}},"required":["record_permissions"],"additionalProperties":false})) }
    fn project_for_managed_hook(&self) -> Option<Value> { Some(json!({"record_permissions":true})) }
} }; }
safe_contract!(Input, "record-permissions-input");
safe_contract!(Output, "record-permissions-output");
struct Adapter(Weak<ApiState>);
fn registration<'a>(
    state: &'a ApiState,
    plugin_code: &str,
) -> Result<&'a ManagedServiceRegistration, ApiError> {
    state
        .console_surface_registry
        .managed_services()
        .iter()
        .find(|s| s.plugin_code == plugin_code)
        .ok_or_else(|| Error::NotFound("record_permission_plugin").into())
}
impl ConsoleInterfacePort<Input, Output> for Adapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}
impl Adapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: Input,
    ) -> Result<Output, ApiError> {
        let state = self
            .0
            .upgrade()
            .ok_or(Error::UpstreamUnavailable("record_permission_host"))?;
        let actor = principal.actor();
        let service =
            control_plane::record_permissions::RecordPermissionService::new(state.store.clone());
        match input {
            Input::Get(query) | Input::Replace(query, _) if query.record_id.is_empty() => {
                Err(Error::InvalidInput("record_id").into())
            }
            Input::Capabilities(body) => {
                if body.record_ids.len() > 100 {
                    return Err(Error::InvalidInput("record_ids").into());
                }
                let registration = registration(&state, &body.plugin_code)?;
                record_acl::ensure_active(&state, registration).await?;
                let resource = record_acl::resource(registration, &body.resource_code)?;
                let mut eligible = Vec::new();
                for action in &resource.actions {
                    for id in &action.operation_ids {
                        let op = registration
                            .declaration
                            .operations
                            .iter()
                            .find(|op| &op.interface_id == id)
                            .ok_or(Error::Conflict("record_permission_operation"))?;
                        if record_acl::operation_allowed(&state, actor, op).await? {
                            eligible.push(action.action_code.clone());
                            break;
                        }
                    }
                }
                let mut items = Vec::new();
                for id in body.record_ids {
                    record_acl::ensure_record(&state, registration, actor, resource, &id).await?;
                    items.push(
                        service
                            .capabilities(
                                actor,
                                &record_acl::key(registration, actor, resource, &id)?,
                                &eligible,
                            )
                            .await?,
                    );
                }
                Ok(Output(json!({"items":items})))
            }
            input => {
                let (query, replace) = match input {
                    Input::Get(query) => (query, None),
                    Input::Replace(query, body) => (query, Some(body)),
                    _ => unreachable!(),
                };
                let registration = registration(&state, &query.plugin_code)?;
                let resource = record_acl::resource(registration, &query.resource_code)?;
                record_acl::ensure_record(&state, registration, actor, resource, &query.record_id)
                    .await?;
                let key = record_acl::key(registration, actor, resource, &query.record_id)?;
                let policy = if let Some(body) = replace {
                    let input = domain::ReplaceRecordPermissions {
                        mode: body.mode,
                        grants: body
                            .grants
                            .into_iter()
                            .map(|g| domain::RecordPermissionGrant {
                                role_id: g.role_id,
                                actions: g.actions,
                            })
                            .collect(),
                        expected_revision: body.expected_revision,
                    };
                    let policy = service
                        .replace(
                            actor,
                            &key,
                            &input,
                            &resource
                                .actions
                                .iter()
                                .map(|a| a.action_code.clone())
                                .collect::<Vec<_>>(),
                        )
                        .await?;
                    super::stream::revoke_installation(registration.installation_id);
                    return Ok(Output(serde_json::to_value(policy)?));
                } else {
                    service.get(actor, &key).await?
                };
                let roles = service.roles(actor).await?;
                Ok(Output(serde_json::to_value(PolicyResponse {
                    resource: ResourceResponse {
                        resource_code: resource.resource_code.clone(),
                        label: resource.label.clone(),
                        description: resource.description.clone(),
                        actions: resource
                            .actions
                            .iter()
                            .map(|a| ActionResponse {
                                action_code: a.action_code.clone(),
                                label: a.label.clone(),
                                description: a.description.clone(),
                            })
                            .collect(),
                    },
                    roles,
                    mode: policy.mode,
                    grants: policy.grants,
                    revision: policy.revision,
                })?))
            }
        }
    }
}
pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "roles.record_permissions.view",
        binding_id: "http.roles.record-permissions.view.v1",
        method: "GET",
        path: "/api/console/settings/roles/record-permissions",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.record_permissions.replace",
        binding_id: "http.roles.record-permissions.replace.v1",
        method: "PUT",
        path: "/api/console/settings/roles/record-permissions",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "roles.record_capabilities.get",
        binding_id: "http.roles.record-capabilities.get.v1",
        method: "POST",
        path: "/api/console/settings/roles/record-capabilities",
        mutating: false,
    },
];
pub(crate) fn compile_registry(
    state: &Arc<ApiState>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.record-permissions",
        "graph:record-permissions-v1",
        DECLARATIONS,
        Arc::new(Adapter(Arc::downgrade(state))),
    )
}
pub(crate) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;
    ConsoleRouteAssembly::new()
        .route(
            "/settings/roles/record-permissions",
            console_get(
                get,
                ConsoleOperation("roles.record_permissions.view".into()),
            )
            .put(
                replace,
                ConsoleOperation("roles.record_permissions.replace".into()),
            ),
        )
        .route(
            "/settings/roles/record-capabilities",
            console_post(
                capabilities,
                ConsoleOperation("roles.record_capabilities.get".into()),
            ),
        )
}
async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    input: Input,
    index: usize,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let credential = if DECLARATIONS[index].mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: state.clone(),
            headers,
        }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: state.clone(),
            headers,
        }
    };
    let Output(output) = console_interface::invoke(
        state.clone(),
        DECLARATIONS[index].binding_id,
        credential,
        input,
    )
    .await?;
    Ok(Json(ApiSuccess::new(output)))
}
#[utoipa::path(get,path="/api/console/settings/roles/record-permissions",operation_id="roles.record_permissions.view",tag="Settings",params(RecordQuery),summary="Read plugin record permissions",description="Reads the current workspace role grants and declared actions for an existing active plugin record.",responses((status=200,body=PolicyResponse)))]
pub(crate) async fn get(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<RecordQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, Input::Get(query), 0).await
}
#[utoipa::path(put,path="/api/console/settings/roles/record-permissions",operation_id="roles.record_permissions.replace",tag="Settings",params(RecordQuery),summary="Replace plugin record permissions",description="Atomically replaces current workspace role grants for an existing active plugin record with revision checking.",request_body=ReplaceBody,responses((status=200,body=SavedPolicyResponse)))]
pub(crate) async fn replace(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<RecordQuery>,
    Json(body): Json<ReplaceBody>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, Input::Replace(query, body), 1).await
}
#[utoipa::path(post,path="/api/console/settings/roles/record-capabilities",operation_id="roles.record_capabilities.get",tag="Settings",summary="Read current caller record capabilities",description="Returns only the authenticated caller's current operation and record capabilities for existing active plugin records.",request_body=CapabilitiesBody,responses((status=200,body=CapabilitiesResponse)))]
pub(crate) async fn capabilities(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CapabilitiesBody>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, Input::Capabilities(body), 2).await
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/record_permissions.rs"]
mod tests;

/// Role UI catalog and OpenAPI consume the API owner's same static English metadata.
pub(crate) fn metadata(interface_id: &str) -> Option<(String, String)> {
    let operation = match interface_id {
        "roles.record_permissions.view" => <__path_get as utoipa::Path>::operation(),
        "roles.record_permissions.replace" => <__path_replace as utoipa::Path>::operation(),
        "roles.record_capabilities.get" => <__path_capabilities as utoipa::Path>::operation(),
        _ => return None,
    };
    Some((operation.summary?, operation.description?))
}
