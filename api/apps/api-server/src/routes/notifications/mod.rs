use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::{
        console_interface,
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
    },
};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;
pub mod events;
pub(crate) mod interface;
use interface::{NotificationInput as Input, NotificationOutput};

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation as Op;
    ConsoleRouteAssembly::new()
        .route(
            "/notifications/channels",
            console_get(channels, Op("notifications.channels.list".into())),
        )
        .route(
            "/notifications/channels/:instance_id/read-all",
            console_post(read_channel, Op("notifications.channels.read_all".into())),
        )
        .route(
            "/notifications/messages/:notification_id/unread",
            console_post(mark_unread, Op("notifications.messages.unread".into())),
        )
        .route(
            "/communications/logs",
            console_get(logs, Op("notifications.logs.list".into())),
        )
        .route(
            "/communications/logs/:log_id",
            console_get(log, Op("notifications.logs.get".into())),
        )
        .route(
            "/communications/types",
            console_get(types, Op("notifications.types.list".into())),
        )
        .route(
            "/communications/instances",
            console_get(list, Op("notifications.instances.list".into()))
                .post(create, Op("notifications.instances.create".into())),
        )
        .route(
            "/communications/instances/:instance_id",
            console_get(get, Op("notifications.instances.get".into()))
                .patch(patch, Op("notifications.instances.update".into()))
                .delete(delete, Op("notifications.instances.delete".into())),
        )
        .route(
            "/communications/instances/:instance_id/authorization",
            console_post(
                authorize,
                Op("notifications.instances.authorization".into()),
            ),
        )
        .route(
            "/communications/instances/:instance_id/send",
            console_post(send, Op("notifications.instances.send".into())),
        )
        .route(
            "/notifications/messages",
            console_get(messages, Op("notifications.messages.list".into())),
        )
        .route(
            "/notifications/messages/unread-count",
            console_get(unread, Op("notifications.messages.unread_count".into())),
        )
        .route(
            "/notifications/messages/:notification_id/read",
            console_post(read, Op("notifications.messages.read".into())),
        )
        .route(
            "/notifications/messages/read-all",
            console_post(read_all, Op("notifications.messages.read_all".into())),
        )
        .route(
            "/notifications/events",
            console_get(events::events, Op("notifications.events.subscribe".into())),
        )
}
async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    operation: &str,
    input: Input,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = match &input {
        Input::Types
        | Input::List(_)
        | Input::Get(_)
        | Input::Messages(_)
        | Input::UnreadCount
        | Input::Logs(_)
        | Input::Log(_)
        | Input::Channels(_) => {
            crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
        }
        Input::Create(_)
        | Input::Patch(..)
        | Input::Delete(_)
        | Input::Authorize(..)
        | Input::Send(..)
        | Input::Read(_)
        | Input::ReadAll
        | Input::ReadChannel(_)
        | Input::MarkUnread(_) => {
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state,
                headers,
            }
        }
    };
    let NotificationOutput(value) = console_interface::invoke(
        snapshot_state,
        &format!("http.console.notifications.{operation}.v1"),
        credential,
        input,
    )
    .await?;
    Ok(Json(ApiSuccess::new(value)))
}
pub async fn types(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "types.list", Input::Types).await
}
pub async fn list(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<control_plane::notifications::NotificationInstanceListInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.list", Input::List(query)).await
}
pub async fn unread(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.unread_count", Input::UnreadCount).await
}
pub async fn read_all(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.read_all", Input::ReadAll).await
}
pub async fn get(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.get", Input::Get(id)).await
}
pub async fn delete(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.delete", Input::Delete(id)).await
}
pub async fn read(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.read", Input::Read(id)).await
}

pub async fn create(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<control_plane::notifications::CreateNotificationInstance>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.create", Input::Create(body)).await
}
pub async fn patch(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<control_plane::notifications::PatchNotificationInstance>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.update", Input::Patch(id, body)).await
}
pub async fn authorize(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<extension_contracts::NotificationAuthorizationOperation>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "instances.authorization",
        Input::Authorize(id, body),
    )
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendBody {
    pub input: Value,
}
pub async fn send(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SendBody>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "instances.send",
        Input::Send(id, body.input),
    )
    .await
}
#[derive(Deserialize, Default)]
pub struct MessageQuery {
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
    pub unread_only: Option<bool>,
    pub instance_id: Option<Uuid>,
    pub read_status: Option<domain::NotificationReadStatus>,
}
pub async fn messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "messages.list",
        Input::Messages(control_plane::ports::NotificationMessageListInput {
            cursor: query.cursor,
            page_size: query.page_size,
            unread_only: query.unread_only.unwrap_or(false),
            instance_id: query.instance_id,
            read_status: query.read_status,
        }),
    )
    .await
}

pub async fn logs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<control_plane::ports::NotificationLogListInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "logs.list", Input::Logs(query)).await
}
pub async fn log(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "logs.get", Input::Log(id)).await
}

pub async fn channels(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<control_plane::ports::NotificationChannelListInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "channels.list", Input::Channels(query)).await
}
pub async fn read_channel(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "channels.read_all", Input::ReadChannel(id)).await
}
pub async fn mark_unread(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.unread", Input::MarkUnread(id)).await
}
