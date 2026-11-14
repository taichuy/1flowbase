use axum::extract::ws::Message;
use control_plane::application_public_api::native::NativeStreamOptions;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum NativeWebSocketClientCommand {
    #[serde(rename = "run.create")]
    Create { request_id: String, request: Value },
    #[serde(rename = "run.cancel")]
    Cancel { request_id: String, run_id: Uuid },
    #[serde(rename = "run.resume")]
    Resume {
        request_id: String,
        run_id: Uuid,
        callback_task_id: Uuid,
        #[serde(default)]
        response_payload: Value,
        #[serde(default)]
        #[schema(value_type = Object)]
        stream_options: NativeStreamOptions,
    },
    #[serde(rename = "run.attach")]
    Attach {
        request_id: String,
        run_id: Uuid,
        #[serde(default)]
        after_event_id: Option<String>,
        #[serde(default)]
        #[schema(value_type = Object)]
        stream_options: NativeStreamOptions,
    },
}

impl NativeWebSocketClientCommand {
    pub(crate) fn request_id(&self) -> &str {
        match self {
            Self::Create { request_id, .. }
            | Self::Cancel { request_id, .. }
            | Self::Resume { request_id, .. }
            | Self::Attach { request_id, .. } => request_id,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum NativeWebSocketCommandError {
    #[error("binary client messages are not supported")]
    BinaryNotSupported,
    #[error("invalid Native WebSocket command")]
    InvalidEnvelope,
}

impl NativeWebSocketCommandError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::BinaryNotSupported => "binary_not_supported",
            Self::InvalidEnvelope => "invalid_command",
        }
    }
}

pub(crate) fn decode_client_message(
    message: Message,
) -> Result<Option<NativeWebSocketClientCommand>, NativeWebSocketCommandError> {
    match message {
        Message::Text(text) => serde_json::from_str(text.as_str())
            .map(Some)
            .map_err(|_| NativeWebSocketCommandError::InvalidEnvelope),
        Message::Binary(_) => Err(NativeWebSocketCommandError::BinaryNotSupported),
        Message::Ping(_) | Message::Pong(_) | Message::Close(_) => Ok(None),
    }
}

pub(crate) fn sequence_from_event_id(
    run_id: Uuid,
    event_id: Option<&str>,
) -> Result<Option<i64>, NativeWebSocketCommandError> {
    let Some(event_id) = event_id else {
        return Ok(None);
    };
    let (event_run_id, sequence) = event_id
        .rsplit_once(':')
        .ok_or(NativeWebSocketCommandError::InvalidEnvelope)?;
    if event_run_id != run_id.to_string() {
        return Err(NativeWebSocketCommandError::InvalidEnvelope);
    }
    sequence
        .parse::<i64>()
        .ok()
        .filter(|sequence| *sequence >= 0)
        .map(Some)
        .ok_or(NativeWebSocketCommandError::InvalidEnvelope)
}
