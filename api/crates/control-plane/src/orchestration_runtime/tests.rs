use super::*;
use crate::{errors::ControlPlaneError, ports::ModelProviderRepository};
use orchestration_runtime::compiled_plan::CompiledLlmRuntime;
use orchestration_runtime::execution_state::{
    ExecutionStopReason, FlowDebugExecutionOutcome, NodeExecutionTrace,
};
use plugin_framework::provider_contract::{
    ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensInput,
    ProviderCountTokensResult, ProviderFinishReason, ProviderInvocationCapability,
    ProviderInvocationInput, ProviderInvocationResult, ProviderMessage, ProviderMessageRole,
    ProviderModelDescriptor, ProviderRuntimeErrorKind, ProviderStreamEvent, ProviderToolCall,
    ProviderWireOperation,
};
use serde_json::Map;

fn compiled_llm_runtime(
    provider_instance_id: impl Into<String>,
    provider_code: &str,
) -> CompiledLlmRuntime {
    CompiledLlmRuntime {
        provider_instance_id: provider_instance_id.into(),
        provider_instance_display_name: String::new(),
        provider_code: provider_code.to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        routing: None,
    }
}

fn assert_control_plane_error(error: anyhow::Error, expected: ControlPlaneError) {
    assert_eq!(error.downcast_ref::<ControlPlaneError>(), Some(&expected));
}

fn provider_user_input(provider_instance_id: Uuid) -> ProviderInvocationInput {
    ProviderInvocationInput {
        provider_instance_id: provider_instance_id.to_string(),
        provider_code: "fixture_provider".to_string(),
        protocol: "openai_compatible".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "pinned invocation".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderInvocationInput::default()
    }
}

mod instance_resolution;
mod internal_tool_events;
mod live_tool_events;
mod media_compatibility;
mod native_transport;
mod provider_commands;
mod route_resolution;
mod terminal_projection;
