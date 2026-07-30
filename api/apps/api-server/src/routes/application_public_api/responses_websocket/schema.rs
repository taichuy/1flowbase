use axum::extract::ws::Message;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use utoipa::ToSchema;

/// A client request accepted by the Responses WebSocket protocol.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "type")]
pub(crate) enum ResponsesWebSocketClientRequest {
    #[serde(rename = "response.create")]
    Create {
        /// The regular OpenAI Responses request body decoded from the flat WebSocket envelope.
        response: Value,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ResponsesWebSocketClientMessageError {
    #[error("binary client messages are not supported")]
    BinaryNotSupported,
    #[error("invalid response.create JSON envelope")]
    InvalidEnvelope,
    #[error("unknown Responses WebSocket request type")]
    UnknownRequestType,
}

impl ResponsesWebSocketClientMessageError {
    /// RFC 6455 close code WP-07 should send for this rejected client message.
    pub(crate) fn close_code(&self) -> u16 {
        match self {
            Self::BinaryNotSupported => 1003,
            Self::InvalidEnvelope => 1007,
            Self::UnknownRequestType => 1008,
        }
    }

    pub(crate) fn close_reason(&self) -> &'static str {
        match self {
            Self::BinaryNotSupported => "binary messages are not supported",
            Self::InvalidEnvelope => "invalid response.create envelope",
            Self::UnknownRequestType => "unknown request type",
        }
    }
}

/// Decodes a client data message. Ping, pong, and close remain transport controls.
pub(crate) fn decode_client_message(
    message: Message,
) -> Result<Option<ResponsesWebSocketClientRequest>, ResponsesWebSocketClientMessageError> {
    match message {
        Message::Text(text) => decode_text_message(text.as_str()).map(Some),
        Message::Binary(_) => Err(ResponsesWebSocketClientMessageError::BinaryNotSupported),
        Message::Ping(_) | Message::Pong(_) | Message::Close(_) => Ok(None),
    }
}

fn decode_text_message(
    text: &str,
) -> Result<ResponsesWebSocketClientRequest, ResponsesWebSocketClientMessageError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| ResponsesWebSocketClientMessageError::InvalidEnvelope)?;
    let request_type = value
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .ok_or(ResponsesWebSocketClientMessageError::InvalidEnvelope)?;
    if request_type != "response.create" {
        return Err(ResponsesWebSocketClientMessageError::UnknownRequestType);
    }

    let mut response = value
        .as_object()
        .cloned()
        .ok_or(ResponsesWebSocketClientMessageError::InvalidEnvelope)?;
    response.remove("type");
    if response.is_empty() || response.contains_key("response") {
        return Err(ResponsesWebSocketClientMessageError::InvalidEnvelope);
    }
    Ok(ResponsesWebSocketClientRequest::Create {
        response: Value::Object(response),
    })
}
