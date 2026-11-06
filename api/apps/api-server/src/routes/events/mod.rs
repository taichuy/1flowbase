//! Event configuration and process-local observation through canonical console interfaces.
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use domain::event_center::{CreateEventDefinitionInput, UpdateEventDefinitionInput};
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
    summary="List registered event definitions", description="Lists custom definitions and visible registered event contracts in the current workspace, with their supported trigger phases.",
    responses((status=200,body=Value)))]
pub(crate) async fn catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "events.catalog.list", Input::Catalog, false).await
}

#[utoipa::path(post, path="/api/console/events", tag="Events", operation_id="events.definitions.create",
    summary="Create a custom event definition", description="Creates a workspace-scoped custom event contract. Only configuration is persisted; this operation does not publish an event.",
    request_body=Value, responses((status=200,body=Value)))]
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
    summary="Update a custom event definition", description="Updates the name, schema or enabled state of a custom event in the current workspace. The event identifier remains unchanged.",
    params(("event_definition_id"=Uuid,Path,description="Custom event definition identifier")), request_body=Value, responses((status=200,body=Value)))]
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
