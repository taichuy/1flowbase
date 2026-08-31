use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use control_plane::network_egress_pool::{NetworkEgressPoolMemberView, NetworkEgressPoolView};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
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

pub(super) fn parse_uuid(value: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

pub(super) fn member_response(
    view: NetworkEgressPoolMemberView,
) -> NetworkEgressPoolMemberResponse {
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

pub(super) fn response(view: NetworkEgressPoolView) -> NetworkEgressPoolResponse {
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pools.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        super::pools_interface::NetworkPoolsInput::List,
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Pools(pools) = output else {
        unreachable!("network pools binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(pools)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-proxies.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::CreateProxy(body),
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Provider(provider) = output else {
        unreachable!("network proxy create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(provider))))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pool-members.test-connection.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::TestMember { pool_id, member_id },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Member(member) = output else {
        unreachable!("network pool member test binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(member)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pools.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::Create(body),
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Pool(pool) = output else {
        unreachable!("network pool create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(pool))))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pools.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::Update { pool_id, body },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Pool(pool) = output else {
        unreachable!("network pool update binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(pool)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pools.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::Delete { pool_id },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Deleted = output else {
        unreachable!("network pool delete binding returned a different output")
    };
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pool-members.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::CreateMember { pool_id, body },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Member(member) = output else {
        unreachable!("network pool member create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(member))))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pool-members.create-static-http.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::AddStatic { pool_id, body },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Member(member) = output else {
        unreachable!("static network pool member binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(member))))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pool-members.add-provider-egresses.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::AddProvider { pool_id, body },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Members(members) = output else {
        unreachable!("provider network pool members binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(members))))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pool-members.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::UpdateMember {
            pool_id,
            member_id,
            body,
        },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Member(member) = output else {
        unreachable!("network pool member update binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(member)))
}

#[utoipa::path(
    delete,
    path = "/api/console/network-center/pools/{pool_id}/members/{member_id}",
    operation_id = "network_egress_pool_members_delete",
    params(("pool_id" = String, Path, description = "Network egress pool id"), ("member_id" = String, Path, description = "Network egress pool member id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn delete_network_egress_pool_member(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((pool_id, member_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.network-egress-pool-members.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        super::pools_interface::NetworkPoolsInput::DeleteMember { pool_id, member_id },
    )
    .await?;
    let super::pools_interface::NetworkPoolsOutput::Deleted = output else {
        unreachable!("network pool member delete binding returned a different output")
    };
    Ok(StatusCode::NO_CONTENT)
}
