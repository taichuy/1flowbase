use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use control_plane::network_egress::{
    CreateNetworkEgressProviderCommand, NetworkEgressProviderService,
    NetworkEgressProviderTypeView, NetworkEgressProviderView,
    UpdateNetworkEgressProviderLifecycleCommand,
};
use control_plane::network_egress_route::{
    CreateNetworkEgressRouteCommand, NetworkEgressRouteService, UpdateNetworkEgressRouteCommand,
};
use control_plane::network_egress_secret::ProviderRegistryNetworkEgressSecretResolver;
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

pub mod plugins;
pub mod pools;
pub mod routes;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNetworkEgressProviderBody {
    pub installation_id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    /// Plugin-defined configuration. It is encrypted before persistence and never returned.
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNetworkEgressProviderLifecycleBody {
    pub lifecycle: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNetworkEgressRouteBody {
    pub consumer_kind: String,
    pub consumer_reference: Option<String>,
    pub pool_member_ids: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNetworkEgressRouteBody {
    pub pool_member_ids: Vec<String>,
    pub enabled: bool,
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
    pub extension_category: Option<String>,
    pub extension_organization: Option<String>,
    pub extension_artifact_id: Option<String>,
    pub provider_code: String,
    pub display_name: String,
    pub description: String,
    pub lifecycle: String,
    pub health_status: String,
    pub secret_configured: bool,
    pub last_sync_error: Option<String>,
    pub last_synced_at: Option<String>,
    pub egresses: Vec<NetworkEgressProjectionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkEgressProviderTypeResponse {
    pub installation_id: Option<String>,
    pub provider_code: String,
    pub display_name: String,
    pub form_schema: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NetworkEgressRouteResponse {
    pub id: String,
    pub consumer_kind: String,
    pub consumer_reference: Option<String>,
    pub pool_member_ids: Vec<String>,
    pub enabled: bool,
    pub failure_policy: String,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    route_assembly_with_plugin_upload_max_bytes(crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES)
}

pub(crate) fn route_assembly_with_plugin_upload_max_bytes(
    plugin_upload_max_bytes: usize,
) -> ConsoleRouteAssembly<Arc<ApiState>> {
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
            "/settings/network-center/providers/types",
            console_get(
                list_network_egress_provider_types,
                ConsoleOperation("network_egress_proxy_types.list".to_string()),
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
        .merge(plugins::route_assembly_with_plugin_upload_max_bytes(
            plugin_upload_max_bytes,
        ))
        .merge(pools::route_assembly())
        .merge(routes::route_assembly())
}

fn service(
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

fn route_service(state: &ApiState) -> NetworkEgressRouteService<storage_durable::MainDurableStore> {
    NetworkEgressRouteService::new(state.store.clone())
}

fn parse_uuid(value: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn parse_uuids(values: Vec<String>, field: &'static str) -> Result<Vec<Uuid>, ApiError> {
    values
        .into_iter()
        .map(|value| parse_uuid(&value, field))
        .collect()
}

fn lifecycle(value: &str) -> Result<domain::NetworkEgressProviderLifecycle, ApiError> {
    match value {
        "draft" => Ok(domain::NetworkEgressProviderLifecycle::Draft),
        "active" => Ok(domain::NetworkEgressProviderLifecycle::Active),
        "disabled" => Ok(domain::NetworkEgressProviderLifecycle::Disabled),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("lifecycle").into()),
    }
}

fn consumer_selector(
    consumer_kind: String,
    consumer_reference: Option<String>,
) -> Result<domain::NetworkEgressConsumerSelector, ApiError> {
    let consumer_reference = consumer_reference
        .as_deref()
        .map(|value| parse_uuid(value, "consumer_reference"))
        .transpose()?;
    domain::NetworkEgressConsumerSelector::from_storage(&consumer_kind, consumer_reference).map_err(
        |_| control_plane::errors::ControlPlaneError::InvalidInput("consumer_selector").into(),
    )
}

fn format_time(value: time::OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("RFC3339 formatting is infallible")
}

pub(super) fn response(view: NetworkEgressProviderView) -> NetworkEgressProviderResponse {
    let extension_family = view.provider.extension_family.as_ref();
    NetworkEgressProviderResponse {
        id: view.provider.id.to_string(),
        extension_category: extension_family.map(|family| family.category().as_str().to_string()),
        extension_organization: extension_family.map(|family| family.organization().to_string()),
        extension_artifact_id: extension_family.map(|family| family.artifact_id().to_string()),
        provider_code: view.provider.provider_code,
        display_name: view.provider.display_name,
        description: view.provider.description,
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

fn type_response(view: NetworkEgressProviderTypeView) -> NetworkEgressProviderTypeResponse {
    NetworkEgressProviderTypeResponse {
        installation_id: view.installation_id.map(|id| id.to_string()),
        provider_code: view.provider_code,
        display_name: view.display_name,
        form_schema: serde_json::to_value(view.form_schema)
            .expect("network egress provider form schema serializes"),
    }
}

fn route_response(route: domain::NetworkEgressRoute) -> NetworkEgressRouteResponse {
    NetworkEgressRouteResponse {
        id: route.id.to_string(),
        consumer_kind: route.selector.consumer_kind().to_string(),
        consumer_reference: route
            .selector
            .consumer_reference()
            .map(|value| value.to_string()),
        pool_member_ids: route
            .pool_member_ids
            .into_iter()
            .map(|value| value.to_string())
            .collect(),
        enabled: route.enabled,
        failure_policy: "block".to_string(),
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
    get,
    path = "/api/console/settings/network-center/providers/types",
    operation_id = "network_egress_provider_types_list",
    responses((status = 200, body = [NetworkEgressProviderTypeResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_network_egress_provider_types(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<NetworkEgressProviderTypeResponse>>>, ApiError> {
    require_session(&state, &headers).await?;
    let types = service(&state).list_types().await?;
    Ok(Json(ApiSuccess::new(
        types.into_iter().map(type_response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/network-center/providers",
    operation_id = "network_egress_providers_create",
    request_body = CreateNetworkEgressProviderBody,
    responses((status = 201, body = NetworkEgressProviderResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
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
            description: body.description,
            secret_json: body.config,
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

#[utoipa::path(
    get,
    path = "/api/console/network-center/routes",
    operation_id = "network_egress_routes_list",
    responses((status = 200, body = [NetworkEgressRouteResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_network_egress_routes(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<NetworkEgressRouteResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let routes = route_service(&state)
        .list(context.actor.current_workspace_id)
        .await?;
    Ok(Json(ApiSuccess::new(
        routes.into_iter().map(route_response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/network-center/routes",
    operation_id = "network_egress_routes_create",
    request_body = CreateNetworkEgressRouteBody,
    responses((status = 201, body = NetworkEgressRouteResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn create_network_egress_route(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateNetworkEgressRouteBody>,
) -> Result<(StatusCode, Json<ApiSuccess<NetworkEgressRouteResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let route = route_service(&state)
        .create(CreateNetworkEgressRouteCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            selector: consumer_selector(body.consumer_kind, body.consumer_reference)?,
            pool_member_ids: parse_uuids(body.pool_member_ids, "pool_member_ids")?,
            enabled: body.enabled,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(route_response(route))),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/console/network-center/routes/{route_id}",
    operation_id = "network_egress_routes_update",
    params(("route_id" = String, Path, description = "Network egress route id")),
    request_body = UpdateNetworkEgressRouteBody,
    responses((status = 200, body = NetworkEgressRouteResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_network_egress_route(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(route_id): Path<String>,
    Json(body): Json<UpdateNetworkEgressRouteBody>,
) -> Result<Json<ApiSuccess<NetworkEgressRouteResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let route = route_service(&state)
        .update(UpdateNetworkEgressRouteCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            route_id: parse_uuid(&route_id, "route_id")?,
            pool_member_ids: parse_uuids(body.pool_member_ids, "pool_member_ids")?,
            enabled: body.enabled,
        })
        .await?;
    Ok(Json(ApiSuccess::new(route_response(route))))
}

#[utoipa::path(
    delete,
    path = "/api/console/network-center/routes/{route_id}",
    operation_id = "network_egress_routes_delete",
    params(("route_id" = String, Path, description = "Network egress route id")),
    responses((status = 204), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_network_egress_route(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(route_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    route_service(&state)
        .delete(
            context.user.id,
            context.actor.current_workspace_id,
            parse_uuid(&route_id, "route_id")?,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_001_ac_002_route_contract_uses_explicit_pool_member_mapping() {
        let first_member_id = Uuid::now_v7();
        let second_member_id = Uuid::now_v7();
        let body: CreateNetworkEgressRouteBody = serde_json::from_value(serde_json::json!({
            "consumer_kind": "github",
            "consumer_reference": null,
            "pool_member_ids": [first_member_id, second_member_id],
            "enabled": true
        }))
        .expect("route request should accept the backend-owned mapping field");
        assert_eq!(
            body.pool_member_ids,
            vec![first_member_id.to_string(), second_member_id.to_string()]
        );

        let now = time::OffsetDateTime::now_utc();
        let response = route_response(domain::NetworkEgressRoute {
            id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            selector: domain::NetworkEgressConsumerSelector::GithubOfficialSources,
            pool_id: Uuid::now_v7(),
            pool_member_ids: vec![first_member_id, second_member_id],
            enabled: true,
            created_by: Uuid::now_v7(),
            updated_by: Uuid::now_v7(),
            created_at: now,
            updated_at: now,
        });
        let json = serde_json::to_value(response).expect("route response should serialize");
        assert_eq!(
            json["pool_member_ids"],
            serde_json::json!([first_member_id, second_member_id])
        );
        assert!(json.get("pool_id").is_none());
    }

    #[test]
    fn ac_003_secret_reference_is_not_serialized_by_provider_projection() {
        let provider = domain::NetworkEgressProviderRecord {
            id: Uuid::now_v7(),
            extension_family: domain::ExtensionCatalogIdentity::new(
                domain::ExtensionCategory::RuntimeExtensions,
                "test",
                "fixture",
            ),
            provider_code: "fixture".to_string(),
            display_name: "Fixture".to_string(),
            description: "Fixture description".to_string(),
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
        assert!(serialized.get("installation_id").is_none());
        assert_eq!(serialized["extension_category"], "runtime-extensions");
        assert_eq!(serialized["extension_organization"], "test");
        assert_eq!(serialized["extension_artifact_id"], "fixture");
        assert!(serialized.get("secret_ref").is_none());
        assert!(!serialized.to_string().contains("secret://"));
    }
}
