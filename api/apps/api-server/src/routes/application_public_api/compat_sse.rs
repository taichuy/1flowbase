#[cfg(test)]
use api_public_gateway::routes::application_public_api::openai::test_support::native_response_id;
#[cfg(test)]
use std::{convert::Infallible, sync::Arc, time::Duration};

#[cfg(test)]
use control_plane::ports::OrchestrationRuntimeRepository;

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[cfg(test)]
use std::collections::HashSet;

#[cfg(test)]
use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse, Response,
};
#[cfg(test)]
use control_plane::application_public_api::{
    callback_resume::{
        ApplicationPublishedCallbackResumeService, PendingNativeWaitingReceipt,
        PreparedPublishedCallbackResume, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    native::NativeRunStatus,
};
#[cfg(test)]
use control_plane::{
    application_public_api::native::{NativeRunResult, NativeUsage},
    orchestration_runtime::{
        debug_stream_events, start_runtime_debug_event_persister,
        wait_for_runtime_debug_event_persister, OrchestrationRuntimeService,
        StartPublishedFlowRunCommand,
    },
    ports::{
        RuntimeEventDeliveryClaim, RuntimeEventDeliveryScope, RuntimeEventEnvelope,
        RuntimeEventPayload,
    },
};
#[cfg(test)]
use serde_json::{json, Value};
#[cfg(test)]
use tokio::sync::mpsc;

#[cfg(test)]
use crate::routes::application_public_api::delivery_receipt::RuntimeEventDeliveryReceipt;
#[cfg(test)]
use tracing::warn;

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[cfg(test)]
use tracing::{debug, info};

#[cfg(test)]
use crate::routes::application_public_api::tool_callback_ids::{
    encode_anthropic_callback_tool_use_id, encode_openai_callback_tool_call_id,
};
#[cfg(test)]
use crate::{
    app_state::ApiState,
    routes::application_public_api::{
        compatibility_interface::CompatibilityExecutionDependencies,
        native::{self, service_error, NativeApiError},
        stream_terminal_fallback::{
            cold_runtime_event_subscription,
            durable_canonical_partial_runtime_events_from_native_run,
            durable_native_run_matches_terminal,
            load_durable_native_run_for_terminal_projection_with_dependencies,
            recover_missing_stream_terminal_winner_with_dependencies,
            terminal_runtime_event_from_native_run, NativeRunTerminalDependencies,
        },
    },
};

#[cfg(test)]
pub(crate) use api_public_gateway::routes::application_public_api::compat_sse::test_support::*;
pub use api_public_gateway::routes::application_public_api::compat_sse::*;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
mod tests;
