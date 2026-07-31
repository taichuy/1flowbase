use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use plugin_framework::{
    error::PluginFrameworkError,
    provider_contract::{
        ProtocolAuthenticationPresentation, ProtocolContextEnvelope, ProviderCompactProfile,
        ProviderCompactResult, ProviderCountTokensInput, ProviderCountTokensResult,
        ProviderFinishReason, ProviderInvocationInput, ProviderInvocationResult, ProviderMcpCall,
        ProviderMessageRole, ProviderRuntimeError, ProviderRuntimeErrorKind, ProviderStreamEvent,
        ProviderToolCall, ProviderUsage, ProviderWireOperation, SourceProtocolRequest,
        PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY,
    },
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    compiled_plan::{
        CodeIsolationProfile, CompiledBinding, CompiledCodeRuntime, CompiledEdge,
        CompiledLlmRouteTarget, CompiledLlmRouting, CompiledLlmRuntime, CompiledNode,
        CompiledOutput, CompiledPlan, CompiledPluginRuntime, LlmDistributionRule, LlmRoutingMode,
    },
    execution_engine::{
        resume_flow_debug_run, start_flow_debug_run, start_flow_debug_run_with_runtime_context,
        CapabilityInvocationOutput, CapabilityInvoker, CodeInvocationOutput, CodeInvoker,
        ExecutionRuntimeContext, LlmRoutingCounterStore, NativeSqlInvocationOutput,
        ProviderInvocationOutput, ProviderInvoker,
    },
    execution_state::{
        compact_operation_receipt_from_traces, count_tokens_receipt_from_traces,
        ExecutionStopReason,
    },
};

struct StubProviderInvoker {
    fail: bool,
    captured_input: Arc<Mutex<Option<ProviderInvocationInput>>>,
    final_content: String,
}

#[async_trait]
impl ProviderInvoker for StubProviderInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        *self
            .captured_input
            .lock()
            .expect("captured input mutex poisoned") = Some(input);

        if self.fail {
            return Ok(ProviderInvocationOutput {
                events: vec![ProviderStreamEvent::Error {
                    error: ProviderRuntimeError {
                        kind: ProviderRuntimeErrorKind::AuthFailed,
                        message: "invalid api_key".to_string(),
                        provider_summary: Some("Authorization: Bearer sk-secret-value".to_string()),
                        provider_details: None,
                    },
                }],
                result: ProviderInvocationResult {
                    finish_reason: Some(ProviderFinishReason::Error),
                    ..ProviderInvocationResult::default()
                },
                first_token_at: None,
                time_to_first_token_ms: None,
            });
        }

        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: self.final_content.clone(),
                },
                ProviderStreamEvent::UsageSnapshot {
                    usage: ProviderUsage {
                        input_tokens: Some(5),
                        output_tokens: Some(7),
                        total_tokens: Some(12),
                        ..ProviderUsage::default()
                    },
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some(self.final_content.clone()),
                usage: ProviderUsage {
                    input_tokens: Some(5),
                    output_tokens: Some(7),
                    total_tokens: Some(12),
                    ..ProviderUsage::default()
                },
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for StubProviderInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        Ok(CapabilityInvocationOutput {
            output_payload: json!({
                "answer": input_payload["query"].clone(),
            }),
        })
    }

    async fn invoke_native_sql_node(
        &self,
        _node: &CompiledNode,
        sql: &str,
    ) -> Result<NativeSqlInvocationOutput> {
        Ok(NativeSqlInvocationOutput {
            output_payload: json!({
                "results": [{ "kind": "completion", "affected_rows": 1 }]
            }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({ "data_source_instance_id": "main", "sql": sql }),
        })
    }
}

struct UnknownCapabilityOutputInvoker;
struct ReservedCapabilityOutputInvoker;

#[async_trait]
impl ProviderInvoker for UnknownCapabilityOutputInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        unreachable!("plugin output contract test does not execute llm nodes")
    }
}

#[async_trait]
impl CapabilityInvoker for UnknownCapabilityOutputInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        Ok(CapabilityInvocationOutput {
            output_payload: json!({
                "answer": "ok",
                "unexpected": true
            }),
        })
    }
}

#[async_trait]
impl ProviderInvoker for ReservedCapabilityOutputInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        unreachable!("plugin output contract test does not execute llm nodes")
    }
}

#[async_trait]
impl CapabilityInvoker for ReservedCapabilityOutputInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        Ok(CapabilityInvocationOutput {
            output_payload: json!({
                "answer": "ok",
                "metadata": { "secret": "x" }
            }),
        })
    }
}

macro_rules! impl_noop_code_invoker {
    ($($ty:ty),+ $(,)?) => {
        $(
            #[async_trait]
            impl CodeInvoker for $ty {
                async fn invoke_code_node(
                    &self,
                    _runtime: &CompiledCodeRuntime,
                    _config_payload: Value,
                    _input_payload: Value,
                ) -> Result<CodeInvocationOutput> {
                    unreachable!("this test invoker does not execute code nodes")
                }
            }
        )+
    };
}

impl_noop_code_invoker!(
    StubProviderInvoker,
    UnknownCapabilityOutputInvoker,
    ReservedCapabilityOutputInvoker,
    RuntimeContractErrorInvoker,
    InvalidProviderContractInvoker,
    ProviderUpstreamErrorInvoker,
    FailsAfterFirstTokenInvoker,
    InputCacheUsageSnapshotInvoker,
    ToolMcpMetadataInvoker,
    FailFirstFailoverInvoker,
    FailAfterTokenFinishErrorFailoverInvoker,
    SequentialLlmToolCallInvoker,
    RecordingSuccessInvoker,
    FailFirstAttemptsInvoker,
);

struct RuntimeContractErrorInvoker;

#[async_trait]
impl ProviderInvoker for RuntimeContractErrorInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Err(PluginFrameworkError::runtime(ProviderRuntimeError {
            kind: ProviderRuntimeErrorKind::ProviderInvalidResponse,
            message: "401 401 Unauthorized: Incorrect API key provided".to_string(),
            provider_summary: None,
            provider_details: None,
        })
        .into())
    }
}

#[async_trait]
impl CapabilityInvoker for RuntimeContractErrorInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("base plan does not execute capability nodes")
    }
}

struct InvalidProviderContractInvoker;

const INVALID_PROVIDER_CONTRACT_DISPLAY: &str =
    "invalid provider contract: provider stream invocation must declare operation=generate";

#[async_trait]
impl ProviderInvoker for InvalidProviderContractInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Err(PluginFrameworkError::invalid_provider_contract(
            "provider stream invocation must declare operation=generate",
        )
        .into())
    }
}

#[async_trait]
impl CapabilityInvoker for InvalidProviderContractInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("base plan does not execute capability nodes")
    }
}

struct ProviderUpstreamErrorInvoker;

const PROVIDER_UPSTREAM_ERROR_BODY: &str =
    " {\"future_error\":{\"shape\":\"unknown\"},\"message\":\"keep complete body\"}\n ";

#[async_trait]
impl ProviderInvoker for ProviderUpstreamErrorInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Ok(ProviderInvocationOutput {
            events: vec![ProviderStreamEvent::Error {
                error: ProviderRuntimeError {
                    kind: ProviderRuntimeErrorKind::ProviderUpstreamError,
                    message: PROVIDER_UPSTREAM_ERROR_BODY.to_string(),
                    provider_summary: Some(PROVIDER_UPSTREAM_ERROR_BODY.to_string()),
                    provider_details: Some(json!({
                        "status": 400,
                        "request_id": "req_123"
                    })),
                },
            }],
            result: ProviderInvocationResult {
                finish_reason: Some(ProviderFinishReason::Error),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for ProviderUpstreamErrorInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("base plan does not execute capability nodes")
    }
}

struct FailsAfterFirstTokenInvoker;

#[async_trait]
impl ProviderInvoker for FailsAfterFirstTokenInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "partial answer".to_string(),
                },
                ProviderStreamEvent::Error {
                    error: ProviderRuntimeError {
                        kind: ProviderRuntimeErrorKind::ProviderInvalidResponse,
                        message: "stream failed".to_string(),
                        provider_summary: None,
                        provider_details: None,
                    },
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("partial answer".to_string()),
                finish_reason: Some(ProviderFinishReason::Error),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for FailsAfterFirstTokenInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("base plan does not execute capability nodes")
    }
}

struct InputCacheUsageSnapshotInvoker;

#[async_trait]
impl ProviderInvoker for InputCacheUsageSnapshotInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "cache-aware response".to_string(),
                },
                ProviderStreamEvent::UsageSnapshot {
                    usage: ProviderUsage {
                        input_tokens: Some(100),
                        input_cache_hit_tokens: Some(40),
                        input_cache_miss_tokens: Some(60),
                        output_tokens: Some(12),
                        total_tokens: Some(112),
                        ..ProviderUsage::default()
                    },
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("cache-aware response".to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for InputCacheUsageSnapshotInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("base plan does not execute capability nodes")
    }
}

struct ToolMcpMetadataInvoker;

#[async_trait]
impl ProviderInvoker for ToolMcpMetadataInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "tool-aware response".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::ToolCall,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("tool-aware response".to_string()),
                tool_calls: vec![ProviderToolCall {
                    id: "tool-call-1".to_string(),
                    name: "lookup_order".to_string(),
                    arguments: json!({ "order_id": "order_123" }),
                    provider_metadata: json!({}),
                }],
                mcp_calls: vec![ProviderMcpCall {
                    id: "mcp-call-1".to_string(),
                    server: "orders".to_string(),
                    method: "get_order".to_string(),
                    arguments: json!({ "id": "order_123" }),
                }],
                finish_reason: Some(ProviderFinishReason::ToolCall),
                provider_metadata: json!({
                    "raw_id": "provider-response-1",
                    (PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY): {
                        "decisions": ["omitted_protocol_context_profile_mismatch"]
                    }
                }),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for ToolMcpMetadataInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("base plan does not execute capability nodes")
    }
}

struct SequentialLlmToolCallInvoker {
    responses: Arc<Mutex<Vec<ProviderInvocationOutput>>>,
    captured_inputs: Arc<Mutex<Vec<ProviderInvocationInput>>>,
}

#[async_trait]
impl ProviderInvoker for SequentialLlmToolCallInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.captured_inputs
            .lock()
            .expect("captured inputs mutex poisoned")
            .push(input);
        let output = self
            .responses
            .lock()
            .expect("responses mutex poisoned")
            .pop()
            .expect("provider response should exist");

        Ok(output)
    }
}

#[async_trait]
impl CapabilityInvoker for SequentialLlmToolCallInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("llm tool callback tests do not execute capability nodes")
    }
}

struct FailFirstFailoverInvoker {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ProviderInvoker for FailFirstFailoverInvoker {
    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.calls
            .lock()
            .expect("calls mutex poisoned")
            .push(runtime.provider_instance_id.clone());
        if matches!(
            runtime.provider_instance_id.as_str(),
            "provider-primary" | "provider-a"
        ) {
            anyhow::bail!("primary provider unavailable");
        }

        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: format!("winner:{}", runtime.model),
                },
                ProviderStreamEvent::UsageSnapshot {
                    usage: ProviderUsage {
                        input_tokens: Some(3),
                        output_tokens: Some(4),
                        total_tokens: Some(7),
                        ..ProviderUsage::default()
                    },
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some(format!("winner:{}", runtime.model)),
                usage: ProviderUsage {
                    input_tokens: Some(3),
                    output_tokens: Some(4),
                    total_tokens: Some(7),
                    ..ProviderUsage::default()
                },
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for FailFirstFailoverInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("failover plan does not execute capability nodes")
    }
}

#[derive(Default)]
struct RecordingRoutingCounterStore {
    values: Mutex<BTreeMap<String, i64>>,
    keys: Mutex<Vec<String>>,
}

#[async_trait]
impl LlmRoutingCounterStore for RecordingRoutingCounterStore {
    async fn increment_counter(
        &self,
        key: &str,
        amount: i64,
        _ttl: Option<time::Duration>,
    ) -> Result<i64> {
        self.keys
            .lock()
            .expect("counter keys mutex poisoned")
            .push(key.to_string());
        let mut values = self.values.lock().expect("counter values mutex poisoned");
        let next = values.get(key).copied().unwrap_or(0) + amount;
        values.insert(key.to_string(), next);
        Ok(next)
    }
}

struct RecordingSuccessInvoker {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ProviderInvoker for RecordingSuccessInvoker {
    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.calls
            .lock()
            .expect("calls mutex poisoned")
            .push(runtime.provider_instance_id.clone());
        Ok(final_provider_output(format!(
            "winner:{}",
            runtime.provider_instance_id
        )))
    }
}

#[async_trait]
impl CapabilityInvoker for RecordingSuccessInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("round-robin plan does not execute capability nodes")
    }
}

struct FailFirstAttemptsInvoker {
    calls: Arc<Mutex<Vec<String>>>,
    remaining_failures: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl ProviderInvoker for FailFirstAttemptsInvoker {
    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.calls
            .lock()
            .expect("calls mutex poisoned")
            .push(runtime.provider_instance_id.clone());

        if self
            .remaining_failures
            .fetch_update(
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
                |remaining| remaining.checked_sub(1),
            )
            .is_ok()
        {
            anyhow::bail!("provider unavailable before first token");
        }

        Ok(final_provider_output(format!(
            "winner:{}",
            runtime.provider_instance_id
        )))
    }
}

#[async_trait]
impl CapabilityInvoker for FailFirstAttemptsInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("retry round-robin plan does not execute capability nodes")
    }
}

fn final_provider_output(content: String) -> ProviderInvocationOutput {
    ProviderInvocationOutput {
        events: vec![ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        }],
        result: ProviderInvocationResult {
            final_content: Some(content),
            finish_reason: Some(ProviderFinishReason::Stop),
            ..ProviderInvocationResult::default()
        },
        first_token_at: None,
        time_to_first_token_ms: None,
    }
}

struct FailAfterTokenFinishErrorFailoverInvoker {
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ProviderInvoker for FailAfterTokenFinishErrorFailoverInvoker {
    async fn invoke_llm(
        &self,
        runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        self.calls
            .lock()
            .expect("calls mutex poisoned")
            .push(runtime.provider_instance_id.clone());

        Ok(ProviderInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "partial answer".to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Error,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("partial answer".to_string()),
                finish_reason: Some(ProviderFinishReason::Error),
                ..ProviderInvocationResult::default()
            },
            first_token_at: None,
            time_to_first_token_ms: None,
        })
    }
}

#[async_trait]
impl CapabilityInvoker for FailAfterTokenFinishErrorFailoverInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: serde_json::Value,
        _input_payload: serde_json::Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("failover plan does not execute capability nodes")
    }
}

fn successful_invoker() -> StubProviderInvoker {
    StubProviderInvoker {
        fail: false,
        captured_input: Arc::new(Mutex::new(None)),
        final_content: "echo:gpt-5.4-mini".to_string(),
    }
}

async fn run_llm_trace_with_fixture_provider() -> crate::execution_state::NodeExecutionTrace {
    let outcome = start_flow_debug_run(
        &base_plan(),
        &json!({
            "node-start": {
                "query": "hello"
            }
        }),
        &successful_invoker(),
    )
    .await
    .unwrap();

    outcome
        .node_traces
        .into_iter()
        .find(|trace| trace.node_id == "node-llm")
        .expect("llm trace should exist")
}

fn assert_no_pending_observability_ref(value: &Value) {
    match value {
        Value::String(text) => {
            assert!(
                !text.starts_with("pending_attempt_id:")
                    && !text.starts_with("pending_projection_id:"),
                "debug payload kept unresolved observability ref: {text}"
            );
        }
        Value::Array(items) => {
            for item in items {
                assert_no_pending_observability_ref(item);
            }
        }
        Value::Object(object) => {
            for item in object.values() {
                assert_no_pending_observability_ref(item);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn base_plan() -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-llm".to_string()],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "query".to_string(),
                title: "用户输入".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-llm".to_string(),
        CompiledNode {
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            alias: "LLM".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec!["node-human".to_string()],
            bindings: BTreeMap::from([(
                "prompt_messages".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "prompt_messages".to_string(),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
                    raw_value: json!([
                        {
                            "id": "user-1",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "{{node-start.query}}"
                            }
                        }
                    ]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "模型输出".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({
                "provider_instance_id": "provider-ready",
                "model": "gpt-5.4-mini"
            }),
            plugin_runtime: None,
            llm_runtime: Some(CompiledLlmRuntime {
                provider_instance_id: "provider-ready".to_string(),
                provider_instance_display_name: String::new(),
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                model: "gpt-5.4-mini".to_string(),
                routing: None,
            }),
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-human".to_string(),
        CompiledNode {
            node_id: "node-human".to_string(),
            node_type: "human_input".to_string(),
            alias: "Human Input".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-llm".to_string()],
            downstream_node_ids: vec!["node-answer".to_string()],
            bindings: BTreeMap::from([(
                "prompt".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "templated_text".to_string(),
                    selector_paths: vec![vec!["node-llm".to_string(), "text".to_string()]],
                    raw_value: json!("请审核：{{ node-llm.text }}"),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "input".to_string(),
                title: "人工输入".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );
    nodes.insert(
        "node-answer".to_string(),
        CompiledNode {
            node_id: "node-answer".to_string(),
            node_type: "answer".to_string(),
            alias: "Answer".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-human".to_string()],
            downstream_node_ids: vec![],
            bindings: BTreeMap::from([(
                "answer_template".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "selector".to_string(),
                    selector_paths: vec![vec!["node-human".to_string(), "input".to_string()]],
                    raw_value: json!(["node-human", "input"]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "answer".to_string(),
                title: "对话输出".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        },
    );

    CompiledPlan {
        flow_id: Uuid::nil(),
        source_draft_id: "draft-1".to_string(),
        schema_version: "1flowbase.flow/v2".to_string(),
        topological_order: vec![
            "node-start".to_string(),
            "node-llm".to_string(),
            "node-human".to_string(),
            "node-answer".to_string(),
        ],
        edges: Vec::new(),
        nodes,
        compile_issues: Vec::new(),
    }
}

fn llm_answer_plan() -> CompiledPlan {
    let mut plan = base_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-answer".to_string(),
    ];
    plan.nodes.remove("node-human");
    let llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("llm node should exist");
    llm.downstream_node_ids = vec!["node-answer".to_string()];
    let answer = plan
        .nodes
        .get_mut("node-answer")
        .expect("answer node should exist");
    answer.dependency_node_ids = vec!["node-llm".to_string()];
    answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "selector".to_string(),
            selector_paths: vec![vec!["node-llm".to_string(), "text".to_string()]],
            raw_value: json!(["node-llm", "text"]),
        },
    )]);
    plan
}

fn multi_llm_answer_plan() -> CompiledPlan {
    let mut plan = llm_answer_plan();
    plan.topological_order = vec![
        "node-start".to_string(),
        "node-llm".to_string(),
        "node-llm-2".to_string(),
        "node-answer".to_string(),
    ];

    let first_llm = plan
        .nodes
        .get_mut("node-llm")
        .expect("first llm node should exist");
    first_llm.downstream_node_ids = vec!["node-llm-2".to_string()];

    plan.nodes.insert(
        "node-llm-2".to_string(),
        CompiledNode {
            node_id: "node-llm-2".to_string(),
            node_type: "llm".to_string(),
            alias: "LLM2".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-llm".to_string()],
            downstream_node_ids: vec!["node-answer".to_string()],
            bindings: BTreeMap::from([(
                "prompt_messages".to_string(),
                CompiledBinding {
                    i18n_text_ref: None,
                    kind: "prompt_messages".to_string(),
                    selector_paths: vec![vec!["node-llm".to_string(), "text".to_string()]],
                    raw_value: json!([
                        {
                            "id": "user-2",
                            "role": "user",
                            "content": {
                                "kind": "templated_text",
                                "value": "Continue from {{ node-llm.text }}"
                            }
                        }
                    ]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "text".to_string(),
                title: "模型输出".to_string(),
                value_type: "string".to_string(),
                selector: Vec::new(),
                json_schema: None,
            }],
            config: json!({
                "provider_instance_id": "provider-ready",
                "model": "gpt-5.4-mini"
            }),
            plugin_runtime: None,
            llm_runtime: Some(CompiledLlmRuntime {
                provider_instance_id: "provider-ready".to_string(),
                provider_instance_display_name: String::new(),
                provider_code: "fixture_provider".to_string(),
                protocol: "openai_compatible".to_string(),
                model: "gpt-5.4-mini".to_string(),
                routing: None,
            }),
            code_runtime: None,
        },
    );

    let answer = plan
        .nodes
        .get_mut("node-answer")
        .expect("answer node should exist");
    answer.dependency_node_ids = vec!["node-llm-2".to_string()];
    answer.bindings = BTreeMap::from([(
        "answer_template".to_string(),
        CompiledBinding {
            i18n_text_ref: None,
            kind: "selector".to_string(),
            selector_paths: vec![vec!["node-llm-2".to_string(), "text".to_string()]],
            raw_value: json!(["node-llm-2", "text"]),
        },
    )]);

    plan
}

fn tool_call_response(tool_calls: Vec<ProviderToolCall>) -> ProviderInvocationResult {
    ProviderInvocationResult {
        final_content: Some("need tools".to_string()),
        tool_calls,
        finish_reason: Some(ProviderFinishReason::ToolCall),
        usage: ProviderUsage {
            input_tokens: Some(11),
            input_cache_hit_tokens: Some(5),
            output_tokens: Some(3),
            total_tokens: Some(14),
            ..ProviderUsage::default()
        },
        ..ProviderInvocationResult::default()
    }
}

fn final_llm_response(text: &str) -> ProviderInvocationResult {
    ProviderInvocationResult {
        final_content: Some(text.to_string()),
        finish_reason: Some(ProviderFinishReason::Stop),
        usage: ProviderUsage {
            input_tokens: Some(20),
            input_cache_hit_tokens: Some(8),
            output_tokens: Some(4),
            total_tokens: Some(24),
            ..ProviderUsage::default()
        },
        ..ProviderInvocationResult::default()
    }
}

fn sequential_tool_invoker(
    responses_in_call_order: Vec<ProviderInvocationResult>,
) -> (
    SequentialLlmToolCallInvoker,
    Arc<Mutex<Vec<ProviderInvocationInput>>>,
) {
    sequential_tool_output_invoker(
        responses_in_call_order
            .into_iter()
            .map(provider_output)
            .collect(),
    )
}

fn sequential_tool_output_invoker(
    responses_in_call_order: Vec<ProviderInvocationOutput>,
) -> (
    SequentialLlmToolCallInvoker,
    Arc<Mutex<Vec<ProviderInvocationInput>>>,
) {
    let captured_inputs = Arc::new(Mutex::new(Vec::new()));
    let invoker = SequentialLlmToolCallInvoker {
        responses: Arc::new(Mutex::new(
            responses_in_call_order.into_iter().rev().collect(),
        )),
        captured_inputs: captured_inputs.clone(),
    };
    (invoker, captured_inputs)
}

fn provider_output(result: ProviderInvocationResult) -> ProviderInvocationOutput {
    ProviderInvocationOutput {
        events: result
            .finish_reason
            .clone()
            .map(|reason| ProviderStreamEvent::Finish { reason })
            .into_iter()
            .collect(),
        result,
        first_token_at: None,
        time_to_first_token_ms: None,
    }
}

mod answer_and_failover;
mod branches;
mod failures_and_parameters;
mod http_request;
mod human_and_tool_resume;
mod llm_context;
mod llm_output;
mod plugin_nodes;
mod sql;
mod variable_updates;
mod workflow;
