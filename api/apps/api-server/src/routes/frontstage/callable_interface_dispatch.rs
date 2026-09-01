use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{ACCEPT_LANGUAGE, AUTHORIZATION, COOKIE},
        HeaderMap, HeaderName, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    frontstage::{FrontstageBlockScopeCommand, FrontstagePageService},
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use uuid::Uuid;

use super::callable_interfaces::{host_injected_parameters, DispatchFrontstageCallableBody};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    openapi_interface::{
        get_openapi_capability_by_route_with, CallableDispatchError, CallableDispatchForwarding,
        CallableDispatchHttpResponse, CallableDispatchPort, CallableDispatchResult,
        OpenApiCapabilityCatalogDependencies,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");
const CALLABLE_DEPTH_HEADER: HeaderName = HeaderName::from_static("x-1flowbase-callable-depth");

pub(crate) struct FrontstageCallableDispatchInput {
    pub(crate) page_id: String,
    pub(crate) tab_id: String,
    pub(crate) body: DispatchFrontstageCallableBody,
    pub(crate) forwarding: CallableDispatchForwarding,
}

impl InterfaceContract for FrontstageCallableDispatchInput {
    const CONTRACT_ID: &'static str = "console-frontstage-callable-dispatch-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum FrontstageCallableDispatchOutput {
    Json(Value),
    NoContent,
    Media(CallableDispatchHttpResponse),
    Target(CallableDispatchHttpResponse),
}

impl InterfaceContract for FrontstageCallableDispatchOutput {
    const CONTRACT_ID: &'static str = "console-frontstage-callable-dispatch-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct FrontstageCallableDispatchDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) openapi: OpenApiCapabilityCatalogDependencies,
    pub(crate) dispatcher: Arc<dyn CallableDispatchPort>,
}

struct FrontstageCallableDispatchAdapter(FrontstageCallableDispatchDependencies);

impl FrontstageCallableDispatchAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FrontstageCallableDispatchInput,
    ) -> Result<FrontstageCallableDispatchOutput, ApiError> {
        let actor = principal.actor();
        if !actor.has_permission("frontstage.page.design") {
            return Err(ControlPlaneError::PermissionDenied("frontstage.page.design").into());
        }

        let page_id = super::parse_uuid(&input.page_id, "page_id")?;
        let tab_id = super::parse_uuid(&input.tab_id, "tab_id")?;
        let workspace_id = actor.current_workspace_id;
        let node = FrontstagePageService::for_actor(self.0.store.clone(), actor.clone())
            .get_block_node(FrontstageBlockScopeCommand {
                actor_user_id: actor.user_id,
                workspace_id,
                page_id,
                block_id: input.body.block_id,
            })
            .await?;
        if node.tab_id != tab_id {
            return Err(ControlPlaneError::NotFound("frontstage_block").into());
        }

        let route = canonical_route_key(&input.body.method, &input.body.path)?;
        let callable = get_openapi_capability_by_route_with(
            &self.0.openapi,
            workspace_id,
            &route.method,
            &route.path,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("frontstage_callable"))?;
        let injected_path = injected_path_parameters(
            &host_injected_parameters(&callable.interface),
            workspace_id,
            page_id,
            tab_id,
        );

        match self
            .0
            .dispatcher
            .dispatch(
                &callable.interface,
                input.body.request,
                injected_path,
                input.forwarding,
            )
            .await
        {
            Ok(CallableDispatchResult::Json(value)) => {
                Ok(FrontstageCallableDispatchOutput::Json(value))
            }
            Ok(CallableDispatchResult::NoContent) => {
                Ok(FrontstageCallableDispatchOutput::NoContent)
            }
            Ok(CallableDispatchResult::Media(response)) => {
                Ok(FrontstageCallableDispatchOutput::Media(response))
            }
            Err(CallableDispatchError::Api(error)) => Err(ApiError::from(error)),
            Err(CallableDispatchError::Target(response)) => {
                Ok(FrontstageCallableDispatchOutput::Target(response))
            }
        }
    }
}

impl ConsoleInterfacePort<FrontstageCallableDispatchInput, FrontstageCallableDispatchOutput>
    for FrontstageCallableDispatchAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontstageCallableDispatchInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstageCallableDispatchOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) fn port(
    dependencies: FrontstageCallableDispatchDependencies,
) -> Arc<dyn ConsoleInterfacePort<FrontstageCallableDispatchInput, FrontstageCallableDispatchOutput>>
{
    Arc::new(FrontstageCallableDispatchAdapter(dependencies))
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[ConsoleInterfaceDeclaration {
    interface_id: "frontstage.callable_interfaces.dispatch",
    binding_id: "http.console.frontstage.callable-interfaces.dispatch.post.v1",
    method: "POST",
    path: "/api/console/frontstage/pages/:page_id/tabs/:tab_id/callable-interfaces/dispatch",
    mutating: true,
}];

pub(crate) fn compile_registry(
    port: Arc<
        dyn ConsoleInterfacePort<FrontstageCallableDispatchInput, FrontstageCallableDispatchOutput>,
    >,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-frontstage-callable-dispatch",
        "graph:console-frontstage-callable-dispatch-v1",
        DECLARATIONS,
        port,
    )
}

#[utoipa::path(
    post,
    path = "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/callable-interfaces/dispatch",
    request_body = DispatchFrontstageCallableBody,
    params(
        ("page_id" = String, Path, description = "Page id"),
        ("tab_id" = String, Path, description = "Tab id")
    ),
    responses(
        (status = 200, body = Object),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn dispatch_frontstage_callable_interface(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, tab_id)): Path<(String, String)>,
    Json(body): Json<DispatchFrontstageCallableBody>,
) -> Result<Response, ApiError> {
    let forwarding = forwarding_from_headers(&headers)?;
    let output = console_interface::invoke(
        Arc::clone(&state),
        "http.console.frontstage.callable-interfaces.dispatch.post.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        FrontstageCallableDispatchInput {
            page_id,
            tab_id,
            body,
            forwarding,
        },
    )
    .await?;
    match output {
        FrontstageCallableDispatchOutput::Json(value) => Ok(Json(
            crate::response::ApiSuccess::new(value.get("data").cloned().unwrap_or(value)),
        )
        .into_response()),
        FrontstageCallableDispatchOutput::NoContent => Ok(StatusCode::NO_CONTENT.into_response()),
        FrontstageCallableDispatchOutput::Media(response)
        | FrontstageCallableDispatchOutput::Target(response) => response_from_dispatch(response),
    }
}

fn forwarding_from_headers(headers: &HeaderMap) -> Result<CallableDispatchForwarding, ApiError> {
    let callable_depth = headers
        .get(&CALLABLE_DEPTH_HEADER)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| ControlPlaneError::InvalidInput("callable_dispatch_depth"))?
                .parse::<u8>()
                .map_err(|_| ControlPlaneError::InvalidInput("callable_dispatch_depth"))
        })
        .transpose()?
        .unwrap_or(0);
    Ok(CallableDispatchForwarding {
        cookie: headers.get(&COOKIE).map(|value| value.as_bytes().to_vec()),
        authorization: headers
            .get(&AUTHORIZATION)
            .map(|value| value.as_bytes().to_vec()),
        accept_language: headers
            .get(&ACCEPT_LANGUAGE)
            .map(|value| value.as_bytes().to_vec()),
        csrf_token: headers
            .get(&CSRF_HEADER)
            .map(|value| value.as_bytes().to_vec()),
        callable_depth,
    })
}

fn response_from_dispatch(response: CallableDispatchHttpResponse) -> Result<Response, ApiError> {
    let mut builder = Response::builder().status(response.status);
    let headers = builder
        .headers_mut()
        .ok_or(ControlPlaneError::InvalidInput("callable_response"))?;
    for header in response.headers {
        headers.append(
            HeaderName::try_from(header.name)
                .map_err(|_| ControlPlaneError::InvalidInput("callable_response_header"))?,
            HeaderValue::from_bytes(&header.value)
                .map_err(|_| ControlPlaneError::InvalidInput("callable_response_header"))?,
        );
    }
    builder
        .body(Body::from(response.body))
        .map_err(|_| ControlPlaneError::InvalidInput("callable_response").into())
}

struct CanonicalRouteKey {
    method: String,
    path: String,
}

fn canonical_route_key(method: &str, path: &str) -> Result<CanonicalRouteKey, ApiError> {
    let method = method.trim().to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    ) {
        return Err(ControlPlaneError::InvalidInput("method").into());
    }
    if path.is_empty()
        || path != path.trim()
        || !path.starts_with('/')
        || path.starts_with("//")
        || path.contains('?')
        || path.contains('#')
        || path.split('/').any(|segment| matches!(segment, "." | ".."))
    {
        return Err(ControlPlaneError::InvalidInput("path").into());
    }
    Ok(CanonicalRouteKey {
        method,
        path: path.to_string(),
    })
}

fn injected_path_parameters(
    parameters: &[&str],
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
) -> BTreeMap<String, String> {
    parameters
        .iter()
        .filter_map(|parameter| {
            let value = match *parameter {
                "workspace_id" => workspace_id,
                "page_id" => page_id,
                "tab_id" | "tab_reference" => tab_id,
                _ => return None,
            };
            Some(((*parameter).to_string(), value.to_string()))
        })
        .collect()
}

#[cfg(test)]
struct UnavailableFrontstageCallableDispatchPort;

#[cfg(test)]
impl ConsoleInterfacePort<FrontstageCallableDispatchInput, FrontstageCallableDispatchOutput>
    for UnavailableFrontstageCallableDispatchPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: FrontstageCallableDispatchInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstageCallableDispatchOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("frontstage callable dispatch fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f12c3_registry_freezes_callable_dispatch_binding() {
        let registry =
            compile_registry(Arc::new(UnavailableFrontstageCallableDispatchPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared callable dispatch binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }

    #[test]
    fn f12c3_route_key_requires_a_supported_method_and_canonical_relative_path() {
        let route = canonical_route_key("get", "/api/console/applications/catalog").unwrap();
        assert_eq!(route.method, "GET");
        assert_eq!(route.path, "/api/console/applications/catalog");
        for path in [
            "https://example.com/api/console/applications",
            "//example.com/api/console/applications",
            "/api/console/applications?limit=20",
            "/api/console/../private",
        ] {
            assert!(canonical_route_key("GET", path).is_err(), "{path}");
        }
        assert!(canonical_route_key("TRACE", "/api/console/applications").is_err());
    }
}
