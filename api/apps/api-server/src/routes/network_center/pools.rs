use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use control_plane::network_egress::{
    CreateNetworkEgressProxyCommand, NetworkEgressProviderService,
};
use control_plane::network_egress_pool::{
    AddProviderEgressesToPoolCommand, AddStaticHttpProxyToPoolCommand,
    CreateNetworkEgressPoolCommand, CreateNetworkEgressPoolMemberCommand,
    NetworkEgressPoolMemberView, NetworkEgressPoolService, NetworkEgressPoolView,
    RecordNetworkEgressPoolMemberProbeCommand, UpdateNetworkEgressPoolCommand,
    UpdateNetworkEgressPoolMemberCommand,
};
use control_plane::network_egress_secret::ProviderRegistryNetworkEgressSecretResolver;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    network_egress_probe::test_network_egress_connection,
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNetworkEgressPoolBody {
    pub display_name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNetworkEgressPoolBody {
    pub display_name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNetworkEgressPoolMemberBody {
    pub provider_id: String,
    pub provider_egress_key: String,
    pub enabled: bool,
    pub sequence: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddStaticHttpProxyToPoolBody {
    pub display_name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    pub enabled: bool,
    pub sequence: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNetworkEgressProxyBody {
    pub provider_code: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddProviderEgressesToPoolBody {
    pub provider_id: String,
    pub enabled: bool,
    pub sequence: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNetworkEgressPoolMemberBody {
    pub enabled: bool,
    pub sequence: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkEgressPoolMemberResponse {
    pub id: String,
    pub provider_id: String,
    pub provider_egress_key: String,
    pub enabled: bool,
    pub sequence: i32,
    /// Computed from the provider registry snapshot; never user-controlled pool state.
    pub health: String,
    pub provider_code: String,
    pub display_name: String,
    pub address_summary: Option<String>,
    pub region: Option<String>,
    pub probe_status: String,
    pub probe_http_status: String,
    pub probe_https_status: String,
    pub probe_latency_ms: i32,
    pub probe_exit_ip: Option<String>,
    pub probe_exit_region: Option<String>,
    pub probe_error_code: Option<String>,
    pub last_probed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkEgressPoolResponse {
    pub id: String,
    pub display_name: String,
    pub owner_provider_id: Option<String>,
    pub selection_strategy: String,
    pub members: Vec<NetworkEgressPoolMemberResponse>,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/network-center/pools",
            console_get(
                list_network_egress_pools,
                ConsoleOperation("network_egress_pools.list".to_string()),
            )
            .post(
                create_network_egress_pool,
                ConsoleOperation("network_egress_pools.create".to_string()),
            ),
        )
        .route(
            "/network-center/pools/proxies",
            console_post(
                create_network_egress_proxy,
                ConsoleOperation("network_egress_proxies.create".to_string()),
            ),
        )
        .route(
            "/network-center/pools/:pool_id",
            console_patch(
                update_network_egress_pool,
                ConsoleOperation("network_egress_pools.update".to_string()),
            )
            .delete(
                delete_network_egress_pool,
                ConsoleOperation("network_egress_pools.delete".to_string()),
            ),
        )
        .route(
            "/network-center/pools/:pool_id/members",
            console_post(
                create_network_egress_pool_member,
                ConsoleOperation("network_egress_pool_members.create".to_string()),
            ),
        )
        .route(
            "/network-center/pools/:pool_id/members/static-http",
            console_post(
                add_static_http_proxy_to_pool,
                ConsoleOperation("network_egress_pool_members.create".to_string()),
            ),
        )
        .route(
            "/network-center/pools/:pool_id/members/provider",
            console_post(
                add_provider_egresses_to_pool,
                ConsoleOperation("network_egress_pool_members.create".to_string()),
            ),
        )
        .route(
            "/network-center/pools/:pool_id/members/:member_id/test-connection",
            console_post(
                test_network_egress_pool_member_connection,
                ConsoleOperation("network_egress_pool_members.test_connection".to_string()),
            ),
        )
        .route(
            "/network-center/pools/:pool_id/members/:member_id",
            console_patch(
                update_network_egress_pool_member,
                ConsoleOperation("network_egress_pool_members.update".to_string()),
            )
            .delete(
                delete_network_egress_pool_member,
                ConsoleOperation("network_egress_pool_members.delete".to_string()),
            ),
        )
}

fn service(state: &ApiState) -> NetworkEgressPoolService<storage_durable::MainDurableStore> {
    NetworkEgressPoolService::with_secret_master_key(
        state.store.clone(),
        state.provider_secret_master_key.clone(),
    )
}

fn proxy_service(
    state: &ApiState,
) -> NetworkEgressProviderService<
    storage_durable::MainDurableStore,
    ApiProviderRuntime,
    ProviderRegistryNetworkEgressSecretResolver<storage_durable::MainDurableStore>,
> {
    NetworkEgressProviderService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        ProviderRegistryNetworkEgressSecretResolver::new(
            state.store.clone(),
            state.provider_secret_master_key.clone(),
        ),
        state.provider_secret_master_key.clone(),
        state.api_node_id.clone(),
    )
}

fn parse_uuid(value: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn member_response(view: NetworkEgressPoolMemberView) -> NetworkEgressPoolMemberResponse {
    NetworkEgressPoolMemberResponse {
        id: view.member.id.to_string(),
        provider_id: view.member.provider_id.to_string(),
        provider_egress_key: view.member.provider_egress_key,
        enabled: view.member.enabled,
        sequence: view.member.sequence,
        health: view.health.as_str().to_string(),
        provider_code: view.provider_code,
        display_name: view.display_name,
        address_summary: view.address_summary,
        region: view.region,
        probe_status: view.member.probe_status.as_str().to_string(),
        probe_http_status: view.member.probe_http_status.as_str().to_string(),
        probe_https_status: view.member.probe_https_status.as_str().to_string(),
        probe_latency_ms: view.member.probe_latency_ms,
        probe_exit_ip: view.member.probe_exit_ip,
        probe_exit_region: view.member.probe_exit_region,
        probe_error_code: view.member.probe_error_code,
        last_probed_at: view.member.last_probed_at.map(|value| {
            value
                .format(&time::format_description::well_known::Rfc3339)
                .expect("RFC3339 formatting must succeed")
        }),
    }
}

fn response(view: NetworkEgressPoolView) -> NetworkEgressPoolResponse {
    NetworkEgressPoolResponse {
        id: view.pool.id.to_string(),
        display_name: view.pool.display_name,
        owner_provider_id: view.pool.owner_provider_id.map(|id| id.to_string()),
        selection_strategy: view.pool.selection_strategy.as_str().to_string(),
        members: view.members.into_iter().map(member_response).collect(),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/network-center/pools",
    operation_id = "network_egress_pools_list",
    responses((status = 200, body = [NetworkEgressPoolResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_network_egress_pools(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<NetworkEgressPoolResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let pools = service(&state).list_global(context.user.id).await?;
    Ok(Json(ApiSuccess::new(
        pools.into_iter().map(response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/network-center/pools/proxies",
    operation_id = "network_egress_proxies_create",
    request_body = CreateNetworkEgressProxyBody,
    responses((status = 201, body = super::NetworkEgressProviderResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn create_network_egress_proxy(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateNetworkEgressProxyBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<super::NetworkEgressProviderResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let proxy = proxy_service(&state)
        .create_proxy(CreateNetworkEgressProxyCommand {
            actor_user_id: context.user.id,
            provider_code: body.provider_code,
            display_name: body.display_name,
            description: body.description,
            config: body.config,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(super::response(proxy))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/network-center/pools/{pool_id}/members/{member_id}/test-connection",
    operation_id = "network_egress_pool_members_test_connection",
    params(("pool_id" = String, Path, description = "Global network egress pool id"), ("member_id" = String, Path, description = "Network egress pool member id")),
    responses((status = 200, body = NetworkEgressPoolMemberResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn test_network_egress_pool_member_connection(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((pool_id, member_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<NetworkEgressPoolMemberResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let pool_id = parse_uuid(&pool_id, "pool_id")?;
    let member_id = parse_uuid(&member_id, "member_id")?;
    let member = service(&state).member(pool_id, member_id).await?;
    let probe = test_network_egress_connection(
        &state,
        control_plane::network_egress_pool::NetworkEgressPoolSelection {
            pool_id,
            member_id,
            provider_id: member.provider_id,
            provider_egress_key: member.provider_egress_key,
        },
    )
    .await;
    let member = service(&state)
        .record_probe(RecordNetworkEgressPoolMemberProbeCommand {
            actor_user_id: context.user.id,
            pool_id,
            member_id,
            status: probe.status,
            http_status: probe.http_status,
            https_status: probe.https_status,
            latency_ms: probe.latency_ms,
            exit_ip: probe.exit_ip,
            exit_region: probe.exit_region,
            error_code: probe.error_code,
        })
        .await?;
    Ok(Json(ApiSuccess::new(member_response(member))))
}

#[utoipa::path(
    post,
    path = "/api/console/network-center/pools",
    operation_id = "network_egress_pools_create",
    request_body = CreateNetworkEgressPoolBody,
    responses((status = 201, body = NetworkEgressPoolResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_network_egress_pool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateNetworkEgressPoolBody>,
) -> Result<(StatusCode, Json<ApiSuccess<NetworkEgressPoolResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let pool = service(&state)
        .create(CreateNetworkEgressPoolCommand {
            actor_user_id: context.user.id,
            display_name: body.display_name,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response(pool)))))
}

#[utoipa::path(
    patch,
    path = "/api/console/network-center/pools/{pool_id}",
    operation_id = "network_egress_pools_update",
    params(("pool_id" = String, Path, description = "Network egress pool id")),
    request_body = UpdateNetworkEgressPoolBody,
    responses((status = 200, body = NetworkEgressPoolResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_network_egress_pool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(pool_id): Path<String>,
    Json(body): Json<UpdateNetworkEgressPoolBody>,
) -> Result<Json<ApiSuccess<NetworkEgressPoolResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let pool = service(&state)
        .update(UpdateNetworkEgressPoolCommand {
            actor_user_id: context.user.id,
            pool_id: parse_uuid(&pool_id, "pool_id")?,
            display_name: body.display_name,
        })
        .await?;
    Ok(Json(ApiSuccess::new(response(pool))))
}

#[utoipa::path(
    delete,
    path = "/api/console/network-center/pools/{pool_id}",
    operation_id = "network_egress_pools_delete",
    params(("pool_id" = String, Path, description = "Network egress pool id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_network_egress_pool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(pool_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    service(&state)
        .delete(context.user.id, parse_uuid(&pool_id, "pool_id")?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/console/network-center/pools/{pool_id}/members",
    operation_id = "network_egress_pool_members_create",
    params(("pool_id" = String, Path, description = "Network egress pool id")),
    request_body = CreateNetworkEgressPoolMemberBody,
    responses((status = 201, body = NetworkEgressPoolMemberResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn create_network_egress_pool_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(pool_id): Path<String>,
    Json(body): Json<CreateNetworkEgressPoolMemberBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<NetworkEgressPoolMemberResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let member = service(&state)
        .add_member(CreateNetworkEgressPoolMemberCommand {
            actor_user_id: context.user.id,
            pool_id: parse_uuid(&pool_id, "pool_id")?,
            provider_id: parse_uuid(&body.provider_id, "provider_id")?,
            provider_egress_key: body.provider_egress_key,
            enabled: body.enabled,
            sequence: body.sequence,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(member_response(member))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/network-center/pools/{pool_id}/members/static-http",
    operation_id = "network_egress_pool_members_create_static_http",
    params(("pool_id" = String, Path, description = "Network egress pool id")),
    request_body = AddStaticHttpProxyToPoolBody,
    responses((status = 201, body = NetworkEgressPoolMemberResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn add_static_http_proxy_to_pool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(pool_id): Path<String>,
    Json(body): Json<AddStaticHttpProxyToPoolBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<NetworkEgressPoolMemberResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let member = service(&state)
        .add_static_http_proxy(AddStaticHttpProxyToPoolCommand {
            actor_user_id: context.user.id,
            pool_id: parse_uuid(&pool_id, "pool_id")?,
            display_name: body.display_name,
            host: body.host,
            port: body.port,
            username: body.username,
            password: body.password,
            enabled: body.enabled,
            sequence: body.sequence,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(member_response(member))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/network-center/pools/{pool_id}/members/provider",
    operation_id = "network_egress_pool_members_add_provider_egresses",
    params(("pool_id" = String, Path, description = "Network egress pool id")),
    request_body = AddProviderEgressesToPoolBody,
    responses((status = 201, body = [NetworkEgressPoolMemberResponse]), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn add_provider_egresses_to_pool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(pool_id): Path<String>,
    Json(body): Json<AddProviderEgressesToPoolBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<Vec<NetworkEgressPoolMemberResponse>>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let members = service(&state)
        .add_provider_egresses(AddProviderEgressesToPoolCommand {
            actor_user_id: context.user.id,
            pool_id: parse_uuid(&pool_id, "pool_id")?,
            provider_id: parse_uuid(&body.provider_id, "provider_id")?,
            enabled: body.enabled,
            sequence: body.sequence,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(
            members.into_iter().map(member_response).collect(),
        )),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/console/network-center/pools/{pool_id}/members/{member_id}",
    operation_id = "network_egress_pool_members_update",
    params(("pool_id" = String, Path, description = "Network egress pool id"), ("member_id" = String, Path, description = "Network egress pool member id")),
    request_body = UpdateNetworkEgressPoolMemberBody,
    responses((status = 200, body = NetworkEgressPoolMemberResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_network_egress_pool_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((pool_id, member_id)): Path<(String, String)>,
    Json(body): Json<UpdateNetworkEgressPoolMemberBody>,
) -> Result<Json<ApiSuccess<NetworkEgressPoolMemberResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let member = service(&state)
        .update_member(UpdateNetworkEgressPoolMemberCommand {
            actor_user_id: context.user.id,
            pool_id: parse_uuid(&pool_id, "pool_id")?,
            member_id: parse_uuid(&member_id, "member_id")?,
            enabled: body.enabled,
            sequence: body.sequence,
        })
        .await?;
    Ok(Json(ApiSuccess::new(member_response(member))))
}

#[utoipa::path(
    delete,
    path = "/api/console/network-center/pools/{pool_id}/members/{member_id}",
    operation_id = "network_egress_pool_members_delete",
    params(("pool_id" = String, Path, description = "Network egress pool id"), ("member_id" = String, Path, description = "Network egress pool member id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_network_egress_pool_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((pool_id, member_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    service(&state)
        .delete_member(
            context.user.id,
            parse_uuid(&pool_id, "pool_id")?,
            parse_uuid(&member_id, "member_id")?,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
