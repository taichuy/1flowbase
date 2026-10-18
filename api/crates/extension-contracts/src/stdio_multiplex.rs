//! Versioned, newline-delimited JSON transport for one long-lived RuntimeExtension worker.
//! `call_id` is a canonical positive decimal u64 assigned in increasing dispatch order
//! by the Host for one worker incarnation; the Host fences replies to that process.
//! A worker frame never carries a trusted installation, workspace, or actor identity.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RUNTIME_WORKER_RETENTION_V1: &str = "runtime_worker_retention/v1";

pub const STDIO_JSON_MULTIPLEX_V1: &str = "stdio_json_multiplex_v1";
/// Cooperative output write window, not a message-size or accumulated-output limit.
pub const MULTIPLEX_OUTPUT_BUDGET_BYTES: usize = 64 * 1024 * 1024;
/// Acknowledged event window. A larger event occupies the window until consumed.
pub const MULTIPLEX_CALL_EVENT_BUDGET_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiplexEnvelope<T> {
    pub protocol: String,
    #[serde(flatten)]
    pub message: T,
}

impl<T> MultiplexEnvelope<T> {
    pub fn new(message: T) -> Self {
        Self {
            protocol: STDIO_JSON_MULTIPLEX_V1.to_owned(),
            message,
        }
    }

    pub fn is_supported(&self) -> bool {
        self.protocol == STDIO_JSON_MULTIPLEX_V1
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MultiplexHostMessage {
    /// Sent only when the installed binding requires these supported worker controls.
    WorkerCapabilities {
        capabilities: Vec<String>,
    },
    Call {
        call_id: String,
        request: Value,
    },
    Input {
        call_id: String,
        input: Value,
    },
    EventAck {
        call_id: String,
        event_id: String,
    },
    Cancel {
        call_id: String,
    },
    CallbackResult {
        call_id: String,
        callback_id: String,
        response: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiplexHostService {
    PluginDataV1,
    PluginCredentialV1,
    FileImportV1,
    FileReadV1,
    ApplicationLogsV1,
    HttpEgressV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MultiplexWorkerMessage {
    /// Process-local resource ownership; never carries a trusted execution identity.
    ResourceRetention {
        sequence: u64,
        retained_resources: u64,
    },
    Event {
        call_id: String,
        event: Value,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        await_ack: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        event_id: Option<String>,
    },
    Response {
        call_id: String,
        response: Value,
    },
    Cancelled {
        call_id: String,
    },
    Callback {
        call_id: String,
        callback_id: String,
        service: MultiplexHostService,
        request: Value,
    },
}
