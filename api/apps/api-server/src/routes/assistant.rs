#[cfg(test)]
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
#[cfg(test)]
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::sse::{KeepAlive, Sse},
    Json,
};
#[cfg(test)]
use control_plane::{
    application::{ApplicationNonCrudConsoleOperation, ApplicationService},
    application_public_api::{
        model_catalog::extract_agent_model_catalog_from_start_node,
        native::{NativeExecution, NativeObject, NativeRequestMetadata, NativeRunRequest},
        publications::{ApplicationPublicationService, LoadActiveApplicationPublicationCommand},
        run_service::{
            assistant_conversation_native_history_to_values, ApplicationPublishedFlowRunRepository,
            ApplicationPublishedRunService, AssistantPageReference,
            CreateAssistantConversationInput, CreateAssistantRunCommand,
            ASSISTANT_PAGE_REFERENCE_MAX_COUNT, ASSISTANT_PAGE_REFERENCE_MAX_TOTAL_BYTES,
        },
    },
    mcp_management::McpManagementService,
    orchestration_runtime::{
        debug_stream_events, project_runtime_event_stream_terminal,
        start_runtime_debug_event_persister, wait_for_runtime_debug_event_persister,
        OrchestrationRuntimeService, StartPublishedFlowRunCommand,
    },
    ports::{
        CacheStore, OrchestrationRuntimeRepository, RuntimeEventCloseReason, RuntimeEventStream,
        RuntimeEventStreamPolicy, TaskQueue,
    },
};
#[cfg(test)]
use domain::mcp_management::McpInstanceStatus;
#[cfg(test)]
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::{json, Value};
#[cfg(test)]
use storage_durable_postgres::MainDurableStore;
#[cfg(test)]
use tokio::sync::{mpsc, oneshot};
#[cfg(test)]
use utoipa::ToSchema;
#[cfg(test)]
use uuid::Uuid;

#[cfg(test)]
use self::conversation_events::{AssistantConversationEventKind, AssistantConversationEventScope};
#[cfg(test)]
use crate::routes::mcp_protocol::virtual_ui::VirtualMcpScope;
#[cfg(test)]
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::RequestContext,
    response::ApiSuccess,
    routes::{
        console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
        debug_run_stream,
        mcp_protocol::virtual_ui,
    },
};
#[cfg(test)]
use control_plane::profile::ASSISTANT_META_KEY;
#[cfg(test)]
use api_host_kernel::assistant_execution::abort_registered_assistant_execution;
pub use api_console_interaction::routes::assistant::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "assistant/_tests/route_contracts.rs"]
mod tests;
