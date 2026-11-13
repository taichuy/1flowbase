use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JobOwner {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub current_workspace_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartInput {
    pub request_id: Uuid,
    pub command: String,
    pub timeout_ms: u64,
    pub retention_ms: u64,
    pub max_output_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadInput {
    pub stream: OutputStream,
    pub offset: u64,
    pub max_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ReadOutput {
    pub text: String,
    pub next_offset: u64,
    pub eof: bool,
    pub encoding_lossy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct JobSnapshot {
    pub execution_id: Uuid,
    pub request_id: Uuid,
    pub state: JobState,
    pub cancel_requested: bool,
    pub exit_code: Option<i64>,
    pub signal: Option<i32>,
    pub duration_ms: u64,
    pub output_complete: bool,
    pub cleanup_complete: bool,
    pub truncated: bool,
    pub discarded_bytes: u64,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: Option<u64>,
    pub retention_ms: u64,
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug)]
pub enum JobError {
    InvalidInput(String),
    NotFound,
    Conflict,
    Io(std::io::Error),
}
impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "{message}"),
            Self::NotFound => write!(f, "execution not found or expired"),
            Self::Conflict => write!(f, "request_id already used with different parameters"),
            Self::Io(_) => write!(f, "local execution I/O failed"),
        }
    }
}
impl std::error::Error for JobError {}
impl From<std::io::Error> for JobError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
