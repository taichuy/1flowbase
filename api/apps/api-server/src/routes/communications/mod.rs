//! Communication management and scoped history use the canonical console invocation boundary.
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
pub(crate) mod interface;
pub(crate) mod provisioning;
pub(crate) mod scope;
use interface::{Input, Output};

#[derive(Default, Deserialize)]
pub(crate) struct PageQuery {
    pub cursor: Option<Uuid>,
    pub limit: Option<u32>,
}
#[derive(Default, Deserialize)]
pub(crate) struct OptionsQuery {
    pub application_id: Option<Uuid>,
}
#[derive(Default, Deserialize)]
pub(crate) struct FileQuery {
    pub offset: Option<u64>,
    pub max_bytes: Option<u32>,
}

pub(crate) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation as Op;
    ConsoleRouteAssembly::new()
        .route(
            "/communications/conversations",
            console_get(
                management_conversations,
                Op("communications.management.conversations.list".into()),
            ),
        )
        .route(
            "/communications/messages",
            console_get(
                management_messages,
                Op("communications.management.messages.list".into()),
            ),
        )
        .route(
            "/communications/messages/:message_id",
            console_get(
                management_message,
                Op("communications.management.messages.get".into()),
            ),
        )
        .route(
            "/communications/options",
            console_get(options, Op("communications.options.get".into())),
        )
        .route(
            "/communications/instances/:instance_id/binding",
            console_get(binding, Op("communications.binding.get".into()))
                .put(save_binding, Op("communications.binding.update".into())),
        )
        .route(
            "/communications/instances/:instance_id/conversations",
            console_get(
                conversations,
                Op("communications.conversations.list".into()),
            ),
        )
        .route(
            "/communications/instances/:instance_id/conversations/:conversation_id/messages",
            console_get(messages, Op("communications.messages.list".into())),
        )
        .route(
            "/communications/instances/:instance_id/conversations/:conversation_id/files",
            console_get(files, Op("communications.files.list".into())),
        )
        .route(
            "/communications/instances/:instance_id/files/:attachment_id",
            console_get(file, Op("communications.files.get".into())),
        )
        .route(
            "/communications/instances/:instance_id/messages/:message_id/retry",
            console_post(retry, Op("communications.messages.retry".into())),
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
    let Output(value) = console_interface::invoke(
        state,
        &format!("http.console.{operation}.v1"),
        credential,
        input,
    )
    .await?;
    Ok(Json(ApiSuccess::new(value)))
}
#[utoipa::path(get,path="/api/console/communications/options",tag="Communications",operation_id="communications_options",params(("application_id"=Option<Uuid>,Query,description="application id")),responses((status=200,body=Value))) ]
pub(crate) async fn options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<OptionsQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.options.get",
        Input::Options(q.application_id),
        false,
    )
    .await
}
#[utoipa::path(get,path="/api/console/communications/instances/{instance_id}/binding",tag="Communications",operation_id="communications_binding",params(("instance_id"=Uuid,Path,description="instance id")),responses((status=200,body=Value))) ]
pub(crate) async fn binding(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.binding.get",
        Input::Binding(id),
        false,
    )
    .await
}
#[utoipa::path(put,path="/api/console/communications/instances/{instance_id}/binding",tag="Communications",operation_id="communications_save_binding",request_body=Value,params(("instance_id"=Uuid,Path,description="instance id")),responses((status=200,body=Value))) ]
pub(crate) async fn save_binding(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<domain::CommunicationBindingInput>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.binding.update",
        Input::SaveBinding(id, input),
        true,
    )
    .await
}
#[utoipa::path(get,path="/api/console/communications/instances/{instance_id}/conversations",tag="Communications",operation_id="communications_conversations",params(("instance_id"=Uuid,Path,description="instance id"),("cursor"=Option<Uuid>,Query,description="cursor"),("limit"=Option<u32>,Query,description="limit")),responses((status=200,body=Value))) ]
pub(crate) async fn conversations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.conversations.list",
        Input::Conversations(id, q),
        false,
    )
    .await
}
#[utoipa::path(get,path="/api/console/communications/instances/{instance_id}/conversations/{conversation_id}/messages",tag="Communications",operation_id="communications_messages",params(("instance_id"=Uuid,Path,description="instance id"),("conversation_id"=Uuid,Path,description="conversation id"),("cursor"=Option<Uuid>,Query,description="cursor"),("limit"=Option<u32>,Query,description="limit")),responses((status=200,body=Value))) ]
pub(crate) async fn messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, conversation)): Path<(Uuid, Uuid)>,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.messages.list",
        Input::Messages(id, conversation, q),
        false,
    )
    .await
}
#[utoipa::path(get,path="/api/console/communications/instances/{instance_id}/conversations/{conversation_id}/files",tag="Communications",operation_id="communications_files",params(("instance_id"=Uuid,Path,description="instance id"),("conversation_id"=Uuid,Path,description="conversation id"),("cursor"=Option<Uuid>,Query,description="cursor"),("limit"=Option<u32>,Query,description="limit")),responses((status=200,body=Value))) ]
pub(crate) async fn files(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, conversation)): Path<(Uuid, Uuid)>,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.files.list",
        Input::Files(id, conversation, q),
        false,
    )
    .await
}
#[utoipa::path(get,path="/api/console/communications/instances/{instance_id}/files/{attachment_id}",tag="Communications",operation_id="communications_file",params(("instance_id"=Uuid,Path,description="instance id"),("attachment_id"=Uuid,Path,description="attachment id"),("offset"=Option<u64>,Query,description="offset"),("max_bytes"=Option<u32>,Query,description="max bytes")),responses((status=200,body=Value))) ]
pub(crate) async fn file(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, attachment)): Path<(Uuid, Uuid)>,
    Query(q): Query<FileQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.files.get",
        Input::File(id, attachment, q),
        false,
    )
    .await
}
#[utoipa::path(post,path="/api/console/communications/instances/{instance_id}/messages/{message_id}/retry",tag="Communications",operation_id="communications_retry",params(("instance_id"=Uuid,Path,description="instance id"),("message_id"=Uuid,Path,description="message id")),responses((status=200,body=Value))) ]
pub(crate) async fn retry(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, message)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.messages.retry",
        Input::Retry(id, message),
        true,
    )
    .await
}

#[utoipa::path(get,path="/api/console/communications/conversations",tag="Communications",operation_id="communications_management_conversations",params(("instance_id"=Option<Uuid>,Query,description="Channel filter"),("conversation_id"=Option<Uuid>,Query,description="Conversation filter"),("page"=Option<u32>,Query,description="Page number, defaults to 1"),("page_size"=Option<u32>,Query,description="Page size, defaults to 20, maximum 100")),responses((status=200,body=Value)))]
pub(crate) async fn management_conversations(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<domain::CommunicationManagementQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.management.conversations.list",
        Input::ManagementConversations(q),
        false,
    )
    .await
}
#[utoipa::path(get,path="/api/console/communications/messages",tag="Communications",operation_id="communications_management_messages",params(("instance_id"=Option<Uuid>,Query,description="Channel filter"),("conversation_id"=Option<Uuid>,Query,description="Conversation filter"),("processing_status"=Option<String>,Query,description="Processing status filter"),("send_status"=Option<String>,Query,description="Send status filter"),("page"=Option<u32>,Query,description="Page number, defaults to 1"),("page_size"=Option<u32>,Query,description="Page size, defaults to 20, maximum 100")),responses((status=200,body=Value)))]
pub(crate) async fn management_messages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<domain::CommunicationManagementQuery>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.management.messages.list",
        Input::ManagementMessages(q),
        false,
    )
    .await
}
#[utoipa::path(get,path="/api/console/communications/messages/{message_id}",tag="Communications",operation_id="communications_management_message",params(("message_id"=Uuid,Path,description="Received message ID")),responses((status=200,body=Value),(status=404,description="Message not found in this workspace")))]
pub(crate) async fn management_message(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    invoke(
        state,
        headers,
        "communications.management.messages.get",
        Input::ManagementMessage(id),
        false,
    )
    .await
}
