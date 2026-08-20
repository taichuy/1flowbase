use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use control_plane::network_egress::{
    CreateNetworkEgressProviderCommand, NetworkEgressProviderService, NetworkEgressProviderView,
    UpdateNetworkEgressProviderLifecycleCommand,
};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    provider_runtime::ApiProviderRuntime,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
};

pub mod pools;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNetworkEgressProviderBody {
    pub installation_id: String,
    pub display_name: String,
    /// An opaque `secret://` locator; secret values are not accepted by this API.
    pub secret_ref: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNetworkEgressProviderLifecycleBody {
    pub lifecycle: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkEgressProjectionResponse {
    pub provider_egress_key: String,
    pub display_name: String,
    pub region: Option<String>,
    pub tags: Vec<String>,
    pub availability: String,
    pub synced_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkEgressProviderResponse {
    pub id: String,
    pub installation_id: String,
    pub provider_code: String,
    pub display_name: String,
    pub lifecycle: String,
    pub health_status: String,
    pub secret_configured: bool,
    pub last_sync_error: Option<String>,
    pub last_synced_at: Option<String>,
    pub egresses: Vec<NetworkEgressProjectionResponse>,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/network-center/providers",
            console_get(
                list_network_egress_providers,
                ConsoleOperation("network_egress_providers.list".to_string()),
            )
            .post(
                create_network_egress_provider,
                ConsoleOperation("network_egress_providers.create".to_string()),
            ),
        )
        .route(
            "/settings/network-center/providers/:id",
            console_patch(
                update_network_egress_provider_lifecycle,
                ConsoleOperation("network_egress_providers.lifecycle.update".to_string()),
            ),
        )
        .route(
            "/settings/network-center/providers/:id/sync",
            console_post(
                sync_network_egress_provider,
                ConsoleOperation("network_egress_providers.sync".to_string()),
            ),
        )
        .merge(pools::route_assembly())
}

fn service(
    state: &ApiState,
) -> NetworkEgressProviderService<storage_durable::MainDurableStore, ApiProviderRuntime> {
    NetworkEgressProviderService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.api_node_id.clone(),
    )
}

fn parse_uuid(value: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn lifecycle(value: &str) -> Result<domain::NetworkEgressProviderLifecycle, ApiError> {
    match value {
        "draft" => Ok(domain::NetworkEgressProviderLifecycle::Draft),
        "active" => Ok(domain::NetworkEgressProviderLifecycle::Active),
        "disabled" => Ok(domain::NetworkEgressProviderLifecycle::Disabled),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("lifecycle").into()),
    }
}

fn format_time(value: time::OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("RFC3339 formatting is infallible")
}

fn response(view: NetworkEgressProviderView) -> NetworkEgressProviderResponse {
    NetworkEgressProviderResponse {
        id: view.provider.id.to_string(),
        installation_id: view.provider.installation_id.to_string(),
        provider_code: view.provider.provider_code,
        display_name: view.provider.display_name,
        lifecycle: view.provider.lifecycle.as_str().to_string(),
        health_status: view.provider.health_status.as_str().to_string(),
        secret_configured: !view.provider.secret_ref.is_empty(),
        last_sync_error: view.provider.last_sync_error,
        last_synced_at: view.provider.last_synced_at.map(format_time),
        egresses: view
            .egresses
            .into_iter()
            .map(|egress| NetworkEgressProjectionResponse {
                provider_egress_key: egress.provider_egress_key,
                display_name: egress.display_name,
                region: egress.region,
                tags: egress.tags,
                availability: egress.availability,
                synced_at: format_time(egress.synced_at),
            })
            .collect(),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/settings/network-center/providers",
    operation_id = "network_egress_providers_list",
    responses((status = 200, body = [NetworkEgressProviderResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_network_egress_providers(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<NetworkEgressProviderResponse>>>, ApiError> {
    require_session(&state, &headers).await?;
    let providers = service(&state).list().await?;
    Ok(Json(ApiSuccess::new(
        providers.into_iter().map(response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/network-center/providers",
    operation_id = "network_egress_providers_create",
    request_body = CreateNetworkEgressProviderBody,
    responses((status = 201, body = NetworkEgressProviderResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_network_egress_provider(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateNetworkEgressProviderBody>,
) -> Result<(StatusCode, Json<ApiSuccess<NetworkEgressProviderResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let provider = service(&state)
        .create(CreateNetworkEgressProviderCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&body.installation_id, "installation_id")?,
            display_name: body.display_name,
            secret_ref: body.secret_ref,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(response(provider))),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/network-center/providers/{id}",
    operation_id = "network_egress_providers_update_lifecycle",
    params(("id" = String, Path, description = "Network egress provider id")),
    request_body = UpdateNetworkEgressProviderLifecycleBody,
    responses((status = 200, body = NetworkEgressProviderResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_network_egress_provider_lifecycle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateNetworkEgressProviderLifecycleBody>,
) -> Result<Json<ApiSuccess<NetworkEgressProviderResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let provider = service(&state)
        .update_lifecycle(UpdateNetworkEgressProviderLifecycleCommand {
            actor_user_id: context.user.id,
            provider_id: parse_uuid(&id, "provider_id")?,
            lifecycle: lifecycle(&body.lifecycle)?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(response(provider))))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/network-center/providers/{id}/sync",
    operation_id = "network_egress_providers_sync",
    params(("id" = String, Path, description = "Network egress provider id")),
    responses((status = 200, body = NetworkEgressProviderResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn sync_network_egress_provider(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<NetworkEgressProviderResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let provider = service(&state)
        .sync(context.user.id, parse_uuid(&id, "provider_id")?)
        .await?;
    Ok(Json(ApiSuccess::new(response(provider))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_003_secret_reference_is_not_serialized_by_provider_projection() {
        let provider = domain::NetworkEgressProviderRecord {
            id: Uuid::now_v7(),
            installation_id: Uuid::now_v7(),
            provider_code: "fixture".to_string(),
            display_name: "Fixture".to_string(),
            secret_ref: "secret://system/network-egress/fixture".to_string(),
            lifecycle: domain::NetworkEgressProviderLifecycle::Draft,
            health_status: domain::NetworkEgressHealthStatus::Unknown,
            last_sync_error: None,
            last_synced_at: None,
            created_by: Uuid::now_v7(),
            updated_by: Uuid::now_v7(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let serialized = serde_json::to_value(response(NetworkEgressProviderView {
            provider,
            egresses: Vec::new(),
        }))
        .expect("response should serialize");

        assert_eq!(serialized["secret_configured"], true);
        assert!(serialized.get("secret_ref").is_none());
        assert!(!serialized.to_string().contains("secret://"));
    }
}
