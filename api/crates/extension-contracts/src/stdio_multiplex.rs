//! Versioned, newline-delimited JSON transport for one long-lived RuntimeExtension worker.
//! `call_id` is a canonical positive decimal u64 assigned in increasing dispatch order
//! by the Host for one worker incarnation; the Host fences replies to that process.
//! A worker frame never carries a trusted installation, workspace, or actor identity.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const STDIO_JSON_MULTIPLEX_V1: &str = "stdio_json_multiplex_v1";
pub const MULTIPLEX_MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
/// Shared across queued worker output, pending terminal frames, and callback results.
pub const MULTIPLEX_OUTPUT_BUDGET_BYTES: usize = 64 * 1024 * 1024;
/// Host budget for events waiting on one call; independent of worker transport buffers.
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
    Call {
        call_id: String,
        request: Value,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MultiplexWorkerMessage {
    Event {
        call_id: String,
        event: Value,
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
