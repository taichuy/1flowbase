use thiserror::Error;
use uuid::Uuid;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativeWebSocketCommandError {
    #[error("binary client messages are not supported")]
    BinaryNotSupported,
    #[error("invalid Native WebSocket command")]
    InvalidEnvelope,
}
impl NativeWebSocketCommandError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::BinaryNotSupported => "binary_not_supported",
            Self::InvalidEnvelope => "invalid_command",
        }
    }
}
pub fn sequence_from_event_id(
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
