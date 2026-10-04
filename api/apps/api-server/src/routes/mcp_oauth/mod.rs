//! Finite OAuth protocol controls. No console password/session authentication is used.
use crate::{
    app_state::ApiState,
    external_route_assembly::{get, post, ExternalRouteAssembly},
};
use axum::{
    body::Bytes,
    extract::{
        rejection::{FormRejection, QueryRejection},
        Form, Path, Query, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::mcp_oauth::{
    self as oauth, AuthorizationRequest, OAuthError, Registration, TokenRequest,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
const COOKIE: &str = "mcp_oauth_browser";
const PREFIX: &str = "/api/public/mcp-oauth";
type Service = crate::app_state::ApiMcpOAuthService;
#[derive(Debug)]
pub struct ProtocolError(pub OAuthError);
impl From<OAuthError> for ProtocolError {
    fn from(e: OAuthError) -> Self {
        Self(e)
    }
}
impl IntoResponse for ProtocolError {
    fn into_response(self) -> Response {
        let status = match self.0.error {
            "server_error" | "temporarily_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
            "slow_down" => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::BAD_REQUEST,
        };
        no_store(
            (
                status,
                Json(json!({"error":self.0.error,"error_description":self.0.description})),
            )
                .into_response(),
        )
    }
}
fn invalid() -> ProtocolError {
    OAuthError::invalid().into()
}
fn no_store(mut r: Response) -> Response {
    r.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    r.headers_mut()
        .insert(header::PRAGMA, header::HeaderValue::from_static("no-cache"));
    r
}
fn issuer(state: &ApiState) -> Result<&str, ProtocolError> {
    state.mcp_oauth_issuer.as_deref().ok_or_else(|| {
        ProtocolError(OAuthError {
            error: "temporarily_unavailable",
            description: "MCP OAuth is not configured.",
        })
    })
}
fn service(state: &ApiState) -> Result<Service, ProtocolError> {
    Ok(Service::new(state.store.clone(), issuer(state)?.into()))
}
fn browser(headers: &HeaderMap) -> Result<String, ProtocolError> {
    let value = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .find_map(|p| p.trim().strip_prefix(&format!("{COOKIE}=")))
        })
        .ok_or_else(invalid)?;
    if value.len() != 43 {
        return Err(invalid());
    }
    Ok(value.into())
}
fn json_input<T: serde::de::DeserializeOwned>(
    state: &ApiState,
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<T, ProtocolError> {
    if headers.get(header::ORIGIN).and_then(|h| h.to_str().ok()) != Some(issuer(state)?)
        || headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.split(';').next())
            != Some("application/json")
        || body.len() > 16 * 1024
    {
        return Err(invalid());
    }
    serde_json::from_slice(body).map_err(|_| invalid())
}
async fn rate_limit(state: &ApiState, key: &str, limit: u64) -> Result<(), ProtocolError> {
    let result = state
        .infrastructure
        .rate_limit_store()
        .consume(
            &format!("mcp-oauth:{key}"),
            limit,
            time::Duration::minutes(1),
        )
        .await
        .map_err(|_| {
            ProtocolError(OAuthError {
                error: "server_error",
                description: "The authorization service is unavailable.",
            })
        })?;
    if !result.allowed {
        return Err(ProtocolError(OAuthError {
            error: "slow_down",
            description: "Too many authorization requests. Please try again shortly.",
        }));
    }
    Ok(())
}
pub(crate) fn route_assembly() -> ExternalRouteAssembly<Arc<ApiState>> {
    ExternalRouteAssembly::new()
        .route("/.well-known/oauth-authorization-server", get(metadata))
        .route(
            "/.well-known/oauth-protected-resource/api/mcp/:instance_id",
            get(protected_resource),
        )
        .route("/api/public/mcp-oauth/config", get(config))
        .route("/api/public/mcp-oauth/register", post(register))
        .route("/api/public/mcp-oauth/authorize", get(authorize))
        .route("/api/public/mcp-oauth/authorization", get(authorization))
        .route("/api/public/mcp-oauth/verify", post(verify))
        .route("/api/public/mcp-oauth/decision", post(decision))
        .route("/api/public/mcp-oauth/token", post(token))
}
async fn metadata(State(state): State<Arc<ApiState>>) -> Result<Response, ProtocolError> {
    let i = issuer(&state)?;
    Ok(no_store(Json(json!({"issuer":i,"authorization_endpoint":format!("{i}{PREFIX}/authorize"),"token_endpoint":format!("{i}{PREFIX}/token"),"registration_endpoint":format!("{i}{PREFIX}/register"),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"token_endpoint_auth_methods_supported":["none"],"code_challenge_methods_supported":["S256"],"scopes_supported":[oauth::SCOPE],"authorization_response_iss_parameter_supported":true})).into_response()))
}
async fn protected_resource(
    State(state): State<Arc<ApiState>>,
    Path(instance): Path<String>,
) -> Result<Response, ProtocolError> {
    let i = issuer(&state)?;
    Ok(no_store(Json(json!({"resource":oauth::resource_url(i,&instance)?,"authorization_servers":[i],"scopes_supported":[oauth::SCOPE],"bearer_methods_supported":["header"]})).into_response()))
}
#[derive(Deserialize)]
struct InstanceQuery {
    instance_id: String,
}
async fn config(
    State(state): State<Arc<ApiState>>,
    query: Result<Query<InstanceQuery>, QueryRejection>,
) -> Result<Response, ProtocolError> {
    let Query(q) = query.map_err(|_| invalid())?;
    let url = state
        .mcp_oauth_issuer
        .as_deref()
        .map(|i| oauth::resource_url(i, &q.instance_id))
        .transpose()?;
    Ok(no_store(Json(json!({"enabled":url.is_some(),"server_url":url,"registration_method":"dynamic_client_registration","scope":oauth::SCOPE})).into_response()))
}
async fn register(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProtocolError> {
    rate_limit(&state, "registration", 60).await?;
    // DCR is server-to-server, so unlike browser consent it has no Origin requirement.
    if body.len() > 16 * 1024
        || headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.split(';').next())
            != Some("application/json")
    {
        return Err(invalid());
    }
    let registration: Registration = serde_json::from_slice(&body).map_err(|_| invalid())?;
    let client = service(&state)?.register(registration).await?;
    Ok(no_store(
        (StatusCode::CREATED, Json(client)).into_response(),
    ))
}
async fn authorize(
    State(state): State<Arc<ApiState>>,
    query: Result<Query<AuthorizationRequest>, QueryRejection>,
) -> Result<Response, ProtocolError> {
    let Query(q) = query.map_err(|_| invalid())?;
    rate_limit(&state, "authorization", 120).await?;
    let cookie = oauth::random_token();
    let id = service(&state)?.begin(q, &cookie).await?;
    let i = issuer(&state)?;
    let secure = if i.starts_with("https:") {
        "; Secure"
    } else {
        ""
    };
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("{i}/mcp/authorize?request_id={id}")
            .parse()
            .map_err(|_| invalid())?,
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        format!("{COOKIE}={cookie}; HttpOnly; SameSite=Lax; Path={PREFIX}; Max-Age=600{secure}")
            .parse()
            .map_err(|_| invalid())?,
    );
    Ok(no_store(response))
}
#[derive(Deserialize)]
struct RequestQuery {
    request_id: String,
}
async fn authorization(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    query: Result<Query<RequestQuery>, QueryRejection>,
) -> Result<Response, ProtocolError> {
    let Query(q) = query.map_err(|_| invalid())?;
    Ok(no_store(
        Json(
            service(&state)?
                .authorization(&q.request_id, &browser(&headers)?)
                .await?,
        )
        .into_response(),
    ))
}
#[derive(Deserialize)]
struct VerifyInput {
    request_id: String,
    api_key: String,
}
async fn verify(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProtocolError> {
    let input: VerifyInput = json_input(&state, &headers, &body)?;
    rate_limit(
        &state,
        &format!("verify:{}", oauth::hash(&browser(&headers)?)),
        20,
    )
    .await?;
    Ok(no_store(
        Json(
            service(&state)?
                .verify(&input.request_id, &browser(&headers)?, &input.api_key)
                .await?,
        )
        .into_response(),
    ))
}
#[derive(Deserialize)]
struct DecisionInput {
    request_id: String,
    approval_token: Option<String>,
    approved: bool,
}
async fn decision(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProtocolError> {
    let input: DecisionInput = json_input(&state, &headers, &body)?;
    let redirect_uri = service(&state)?
        .decision(
            &input.request_id,
            &browser(&headers)?,
            input.approval_token.as_deref(),
            input.approved,
        )
        .await?;
    Ok(no_store(
        Json(json!({"redirect_uri":redirect_uri})).into_response(),
    ))
}
async fn token(
    State(state): State<Arc<ApiState>>,
    form: Result<Form<TokenRequest>, FormRejection>,
) -> Result<Response, ProtocolError> {
    let Form(input) = form.map_err(|_| invalid())?;
    Ok(no_store(
        Json(service(&state)?.token(input).await?).into_response(),
    ))
}
#[derive(Debug, thiserror::Error)]
#[error("MCP authentication required")]
pub(crate) struct McpAuthenticationRequired {
    pub metadata_url: String,
}
pub(crate) fn authentication_challenge(error: &McpAuthenticationRequired) -> Response {
    let mut response=(StatusCode::UNAUTHORIZED,Json(json!({"error":"invalid_token","error_description":"A valid MCP credential is required."}))).into_response();
    if let Ok(value) = format!(
        "Bearer resource_metadata=\"{}\", scope=\"{}\"",
        error.metadata_url,
        oauth::SCOPE
    )
    .parse()
    {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    no_store(response)
}
