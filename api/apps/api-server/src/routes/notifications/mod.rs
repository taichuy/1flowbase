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
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
pub mod events;
pub(crate) mod interface;
pub mod openapi;
use interface::{NotificationInput as Input, NotificationOutput};

#[derive(ToSchema)]
pub struct UnreadNotificationCount {
    pub unread_count: i64,
}
#[derive(ToSchema)]
pub struct UpdatedNotificationCount {
    pub updated_count: u64,
}

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
#[utoipa::path(
    get,
    path = "/api/console/communications/types",
    operation_id = "notifications.types.list",
    tag = "Notifications",
    summary = "List notification types",
    description = "Return canonical notification types and valid recipient form options available in the current workspace.",
    responses((status = 200, body = [openapi::NotificationTypeCatalogEntrySchema]))
)]
pub async fn types(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "types.list", Input::Types).await
}
#[utoipa::path(
    get,
    path = "/api/console/communications/instances",
    operation_id = "notifications.instances.list",
    tag = "Notifications",
    summary = "List notification instances",
    description = "List workspace notification instances with credential fields removed and explicit availability status.",
    params(openapi::NotificationInstanceQuery),
    responses((status = 200, body = [openapi::NotificationInstanceSchema]))
)]
pub async fn list(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<control_plane::notifications::NotificationInstanceListInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.list", Input::List(query)).await
}
#[utoipa::path(
    get,
    path = "/api/console/notifications/messages/unread-count",
    operation_id = "notifications.messages.unread_count",
    tag = "Notifications",
    summary = "Count my unread notifications",
    description = "Count only unread messages owned by the authenticated recipient in the current workspace.",
    responses((status = 200, body = UnreadNotificationCount))
)]
pub async fn unread(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.unread_count", Input::UnreadCount).await
}
#[utoipa::path(
    post,
    path = "/api/console/notifications/messages/read-all",
    operation_id = "notifications.messages.read_all",
    tag = "Notifications",
    summary = "Mark all my notifications read",
    description = "Mark all unread messages of the authenticated recipient in the current workspace read.",
    responses((status = 200, body = UpdatedNotificationCount))
)]
pub async fn read_all(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.read_all", Input::ReadAll).await
}
#[utoipa::path(
    get,
    path = "/api/console/communications/instances/{instance_id}",
    operation_id = "notifications.instances.get",
    tag = "Notifications",
    summary = "Get notification instance",
    description = "Read one instance in the current workspace with stored credentials omitted.",
    params(("instance_id" = Uuid, Path)),
    responses((status = 200, body = openapi::NotificationInstanceSchema))
)]
pub async fn get(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.get", Input::Get(id)).await
}
#[utoipa::path(
    delete,
    path = "/api/console/communications/instances/{instance_id}",
    operation_id = "notifications.instances.delete",
    tag = "Notifications",
    summary = "Delete notification instance",
    description = "Delete one workspace notification instance and preserve independently stored inbox history.",
    params(("instance_id" = Uuid, Path)),
    responses((status = 200, body = openapi::NotificationDeletedSchema))
)]
pub async fn delete(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.delete", Input::Delete(id)).await
}
#[utoipa::path(
    post,
    path = "/api/console/notifications/messages/{notification_id}/read",
    operation_id = "notifications.messages.read",
    tag = "Notifications",
    summary = "Mark my notification read",
    description = "Mark only the authenticated recipient message read, preserving the original read timestamp on repeated calls.",
    params(("notification_id" = Uuid, Path)),
    responses((status = 200, body = domain::SystemNotificationRecord))
)]
pub async fn read(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.read", Input::Read(id)).await
}

#[utoipa::path(
    post,
    path = "/api/console/communications/instances",
    operation_id = "notifications.instances.create",
    tag = "Notifications",
    summary = "Create notification instance",
    description = "Validate the type configuration and store one workspace instance with password fields encrypted.",
    request_body = openapi::CreateNotificationInstanceSchema,
    responses((status = 200, body = openapi::NotificationInstanceSchema))
)]
pub async fn create(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<control_plane::notifications::CreateNotificationInstance>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.create", Input::Create(body)).await
}
#[utoipa::path(
    patch,
    path = "/api/console/communications/instances/{instance_id}",
    operation_id = "notifications.instances.update",
    tag = "Notifications",
    summary = "Update notification instance",
    description = "Update instance name, status or configuration in its workspace while retaining omitted password fields.",
    request_body = openapi::PatchNotificationInstanceSchema,
    params(("instance_id" = Uuid, Path)),
    responses((status = 200, body = openapi::NotificationInstanceSchema))
)]
pub async fn patch(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<control_plane::notifications::PatchNotificationInstance>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "instances.update", Input::Patch(id, body)).await
}
#[utoipa::path(
    post,
    path = "/api/console/communications/instances/{instance_id}/authorization",
    operation_id = "notifications.instances.authorization",
    tag = "Notifications",
    summary = "Authorize notification instance",
    description = "Begin, poll, cancel or maintain provider authorization for one current-workspace notification instance; return only the public authorization view and persist managed credentials encrypted.",
    request_body = openapi::NotificationAuthorizationOperationSchema,
    params(("instance_id" = Uuid, Path)),
    responses((status = 200, body = openapi::NotificationAuthorizationViewSchema))
)]
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

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SendBody {
    /// Business input validated against the selected notification type input_schema.
    pub input: Value,
}
#[utoipa::path(
    post,
    path = "/api/console/communications/instances/{instance_id}/send",
    operation_id = "notifications.instances.send",
    tag = "Notifications",
    summary = "Send notification",
    description = "Send validated invocation input through an enabled workspace instance and return the committed or provider-confirmed result.",
    request_body = SendBody,
    params(("instance_id" = Uuid, Path)),
    responses((status = 200, body = domain::NotificationSendResult))
)]
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
#[derive(Deserialize, Default, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct MessageQuery {
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
    pub unread_only: Option<bool>,
    pub instance_id: Option<Uuid>,
    #[param(inline)]
    pub read_status: Option<domain::NotificationReadStatus>,
}
#[utoipa::path(
    get,
    path = "/api/console/notifications/messages",
    operation_id = "notifications.messages.list",
    tag = "Notifications",
    summary = "List my notification messages",
    description = "Page only the authenticated recipient inbox in the current workspace and return its unread count.",
    params(MessageQuery),
    responses((status = 200, body = domain::NotificationMessagePage))
)]
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

#[utoipa::path(
    get,
    path = "/api/console/communications/logs",
    operation_id = "notifications.logs.list",
    tag = "Notifications",
    summary = "List notification send logs",
    description = "Page safe send invocation facts in the current workspace using explicit log-read authorization.",
    params(openapi::NotificationLogQuery),
    responses((status = 200, body = domain::NotificationSendLogPage))
)]
pub async fn logs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<control_plane::ports::NotificationLogListInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "logs.list", Input::Logs(query)).await
}
#[utoipa::path(
    get,
    path = "/api/console/communications/logs/{log_id}",
    operation_id = "notifications.logs.get",
    tag = "Notifications",
    summary = "Get notification send log",
    description = "Read one safe send invocation fact in the current workspace using explicit log-detail authorization.",
    params(("log_id" = Uuid, Path)),
    responses((status = 200, body = domain::NotificationSendLog))
)]
pub async fn log(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "logs.get", Input::Log(id)).await
}

#[utoipa::path(
    get,
    path = "/api/console/notifications/channels",
    operation_id = "notifications.channels.list",
    tag = "Notifications",
    summary = "List my notification channels",
    description = "Page channels from the authenticated recipient inbox with filtered latest messages and independently aggregated unread counts.",
    params(openapi::NotificationChannelQuery),
    responses((status = 200, body = domain::NotificationChannelPage))
)]
pub async fn channels(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<control_plane::ports::NotificationChannelListInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "channels.list", Input::Channels(query)).await
}
#[utoipa::path(
    post,
    path = "/api/console/notifications/channels/{instance_id}/read-all",
    operation_id = "notifications.channels.read_all",
    tag = "Notifications",
    summary = "Mark my notification channel read",
    description = "Mark unread inbox messages read only in the requested channel, current workspace and authenticated recipient.",
    params(("instance_id" = Uuid, Path)),
    responses((status = 200, body = UpdatedNotificationCount))
)]
pub async fn read_channel(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "channels.read_all", Input::ReadChannel(id)).await
}
#[utoipa::path(
    post,
    path = "/api/console/notifications/messages/{notification_id}/unread",
    operation_id = "notifications.messages.unread",
    tag = "Notifications",
    summary = "Mark my notification unread",
    description = "Clear the read timestamp only for a message owned by the authenticated recipient in the current workspace.",
    params(("notification_id" = Uuid, Path)),
    responses((status = 200, body = domain::SystemNotificationRecord))
)]
pub async fn mark_unread(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(state, headers, "messages.unread", Input::MarkUnread(id)).await
}
