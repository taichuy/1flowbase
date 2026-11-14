//! Event configuration and process-local observation through canonical console interfaces.
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use domain::event_center::{
    CreateEventDefinitionInput, EventCatalogQuery, UpdateEventDefinitionInput,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub(crate) mod interface;
use interface::{Input, Output};

pub(crate) fn route_assembly(
) -> crate::routes::console_route_assembly::ConsoleRouteAssembly<Arc<ApiState>> {
    use crate::routes::console_route_assembly::{console_get, console_put, ConsoleRouteAssembly};
    use access_control::ConsoleRouteOwnership::ConsoleOperation as Op;
    ConsoleRouteAssembly::new()
        .route(
            "/events",
            console_get(catalog, Op("events.catalog.list".into()))
                .post(create, Op("events.definitions.create".into())),
        )
        .route(
            "/events/api-operations",
            console_get(api_operations, Op("events.api_operations.list".into())),
        )
        .route(
            "/events/api-operations/:operation_id",
            console_get(api_operation, Op("events.api_operations.get".into())),
        )
        .route(
            "/events/:event_definition_id",
            console_put(update, Op("events.definitions.update".into())),
        )
        .route(
            "/events/runtime",
            console_get(runtime, Op("events.runtime.get".into())),
        )
}

async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    operation: &str,
    input: Input,
    mutating: bool,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let credential = if mutating {
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
    let Output(value) = crate::routes::console_interface::invoke(
        state,
        &format!("http.console.{operation}.v1"),
        credential,
        input,
    )
    .await?;
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path="/api/console/events", tag="Events", operation_id="events.catalog.list",
    summary="List registered event definitions", description="Lists custom definitions and visible registered event contracts in the current workspace, with supported trigger phases and nullable creation/update timestamps. Defaults to most recently updated first; entries without timestamps follow dated entries in either direction.",
    params(EventCatalogQuery),
    responses((status=200,body=Value)))]
pub(crate) async fn catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<EventCatalogQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "events.catalog.list",
        Input::Catalog(query),
        false,
    )
    .await
}

#[utoipa::path(post, path="/api/console/events", tag="Events", operation_id="events.definitions.create",
    summary="Create an interface event rule", description="Creates a workspace-scoped rule bound to a declared canonical interface and lifecycle phase. Schema is inherited from the interface owner; this operation does not publish an event.",
    request_body=CreateEventDefinitionInput, responses((status=200,body=Value)))]
pub(crate) async fn create(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(input): Json<CreateEventDefinitionInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "events.definitions.create",
        Input::Create(input),
        true,
    )
    .await
}

#[utoipa::path(put, path="/api/console/events/{event_definition_id}", tag="Events", operation_id="events.definitions.update",
    summary="Update an interface event rule", description="Updates an interface event rule and enabled state in the current workspace. Schema is derived from the selected interface, and the rule identifier remains unchanged.",
    params(("event_definition_id"=Uuid,Path,description="Custom event definition identifier")), request_body=UpdateEventDefinitionInput, responses((status=200,body=Value)))]
pub(crate) async fn update(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateEventDefinitionInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "events.definitions.update",
        Input::Update(id, input),
        true,
    )
    .await
}

#[utoipa::path(get, path="/api/console/events/runtime", tag="Events", operation_id="events.runtime.get",
    summary="Inspect current event queues and activity", description="Returns the current host instance's workspace-scoped ephemeral queues and recent metadata. Reading this snapshot never consumes messages; observations are lost on restart.",
    responses((status=200,body=Value)))]
pub(crate) async fn runtime(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "events.runtime.get", Input::Runtime, false).await
}

#[utoipa::path(get, path="/api/console/events/api-operations", tag="Events", operation_id="events.api_operations.list",
    summary="List API event operations", description="Lists the complete shared API documentation catalog with independent Before and Completion availability. Schemas are loaded by operation identifier.",
    responses((status=200,body=Vec<domain::event_center::ApiEventOperationSummary>)))]
pub(crate) async fn api_operations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "events.api_operations.list",
        Input::ApiOperations,
        false,
    )
    .await
}
#[utoipa::path(get, path="/api/console/events/api-operations/{operation_id}", tag="Events", operation_id="events.api_operations.get",
    summary="Read API event operation details", description="Returns the selected documented operation's owner-filtered request, result and lifecycle payload schemas from the current compiled generation.",
    params(("operation_id"=String,Path,description="URL-encoded documented operation identifier")),
    responses((status=200,body=domain::event_center::ApiEventOperationDetails)))]
pub(crate) async fn api_operation(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(operation_id): Path<String>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "events.api_operations.get",
        Input::ApiOperation(operation_id),
        false,
    )
    .await
}
