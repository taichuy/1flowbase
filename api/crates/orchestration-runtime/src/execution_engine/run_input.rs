use super::*;
use plugin_framework::provider_contract::{
    ClientProtocolEnvelope, NativeModelPromptContext, NativeModelRequestContext,
    CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY, NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY,
    NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY,
};
use std::sync::Arc;

use crate::execution_state::{ApplicationFlowExecutionIntent, CompactResponseIngress};

#[derive(Clone, Default)]
pub struct ExecutionRuntimeContext {
    pub(super) tools: Vec<Value>,
    pub(super) client_protocol_envelope: Option<ClientProtocolEnvelope>,
    pub(super) native_model_prompt_context: NativeModelPromptContext,
    pub(super) native_model_request_context: NativeModelRequestContext,
    pub(super) llm_routing_counter_store: Option<Arc<dyn LlmRoutingCounterStore>>,
    pub(super) http_response_file_persister: Option<Arc<dyn HttpResponseFilePersister>>,
    pub(super) compact_response_ingress: Option<CompactResponseIngress>,
}

impl ExecutionRuntimeContext {
    pub fn from_plan_input(
        plan: &CompiledPlan,
        variable_pool: &Map<String, Value>,
    ) -> Result<Self> {
        Ok(Self {
            tools: run_level_provider_tools(plan, variable_pool),
            client_protocol_envelope: client_protocol_envelope_from_variable_pool(variable_pool)?,
            native_model_prompt_context: native_model_prompt_context_from_variable_pool(
                variable_pool,
            )?,
            native_model_request_context: native_model_request_context_from_variable_pool(
                variable_pool,
            )?,
            llm_routing_counter_store: None,
            http_response_file_persister: None,
            compact_response_ingress: None,
        })
    }

    pub fn with_llm_routing_counter_store(
        mut self,
        store: Arc<dyn LlmRoutingCounterStore>,
    ) -> Self {
        self.llm_routing_counter_store = Some(store);
        self
    }

    pub fn with_http_response_file_persister(
        mut self,
        persister: Arc<dyn HttpResponseFilePersister>,
    ) -> Self {
        self.http_response_file_persister = Some(persister);
        self
    }

    /// Callers use this only after the published route has explicitly chosen
    /// application-flow dispatch and admitted one successful typed Compact
    /// result. Transparent Compact routing bypasses this runtime entirely.
    pub fn with_application_flow_compact_ingress(
        mut self,
        ingress: CompactResponseIngress,
    ) -> Self {
        self.compact_response_ingress = Some(ingress);
        self
    }

    pub fn execution_intent(&self) -> ApplicationFlowExecutionIntent {
        self.compact_response_ingress
            .as_ref()
            .map(CompactResponseIngress::intent)
            .unwrap_or(ApplicationFlowExecutionIntent::Ordinary)
    }

    pub(super) fn compact_response_ingress(&self) -> Option<&CompactResponseIngress> {
        self.compact_response_ingress.as_ref()
    }

    pub(super) async fn next_llm_routing_counter(
        &self,
        key: &str,
        ttl: Option<time::Duration>,
    ) -> Result<i64> {
        let store = self
            .llm_routing_counter_store
            .as_ref()
            .ok_or_else(|| anyhow!("llm routing counter store is not configured"))?;
        store.increment_counter(key, 1, ttl).await
    }
}

fn native_model_prompt_context_from_variable_pool(
    variable_pool: &Map<String, Value>,
) -> Result<NativeModelPromptContext> {
    let Some(value) = variable_pool.get(NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY) else {
        return Ok(NativeModelPromptContext::default());
    };

    serde_json::from_value(value.clone())
        .map_err(|error| anyhow!("invalid {NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY}: {error}"))
}

fn native_model_request_context_from_variable_pool(
    variable_pool: &Map<String, Value>,
) -> Result<NativeModelRequestContext> {
    let Some(value) = variable_pool.get(NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY) else {
        return Ok(NativeModelRequestContext::default());
    };

    serde_json::from_value(value.clone())
        .map_err(|error| anyhow!("invalid {NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY}: {error}"))
}

fn client_protocol_envelope_from_variable_pool(
    variable_pool: &Map<String, Value>,
) -> Result<Option<ClientProtocolEnvelope>> {
    variable_pool
        .get(CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY)
        .cloned()
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|error| anyhow!("invalid {CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY}: {error}"))
        })
        .transpose()
}

pub fn normalize_plan_variable_pool(plan: &CompiledPlan, variable_pool: &mut Map<String, Value>) {
    for (node_id, node) in &plan.nodes {
        if node.node_type != "start" {
            continue;
        }

        let start_payload = variable_pool
            .entry(node_id.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if !start_payload.is_object() {
            continue;
        }
        if let Some(start_payload) = start_payload.as_object_mut() {
            materialize_start_builtin_defaults(start_payload);
        }
    }
}

pub(super) fn synchronize_runtime_global_variables(
    plan: &CompiledPlan,
    variable_pool: &mut Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
) {
    let prior_user_turn_count = if runtime_context.native_model_prompt_context.is_empty() {
        legacy_start_history(plan, variable_pool)
            .map(prior_user_turn_count)
            .unwrap_or_default()
    } else {
        prior_user_turn_count(&runtime_context.native_model_prompt_context.messages)
    };

    let Some(sys) = variable_pool.get_mut("sys").and_then(Value::as_object_mut) else {
        return;
    };
    if !sys.contains_key("conversation_id") {
        return;
    }
    sys.insert("dialog_count".to_string(), json!(prior_user_turn_count));
}

fn legacy_start_history<'a>(
    plan: &CompiledPlan,
    variable_pool: &'a Map<String, Value>,
) -> Option<&'a [Value]> {
    plan.nodes
        .values()
        .find(|node| node.node_type == "start")
        .and_then(|node| variable_pool.get(&node.node_id))
        .and_then(|value| value.get("history"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
}

fn prior_user_turn_count(messages: &[Value]) -> usize {
    messages
        .iter()
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .count()
}

pub(super) fn start_node_execution_input(
    variable_pool: &Map<String, Value>,
    node_id: &str,
) -> Value {
    let mut payload = variable_pool
        .get(node_id)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for namespace in ["sys", "env", "conversation", "trigger"] {
        if let Some(value) = variable_pool.get(namespace) {
            payload.insert(namespace.to_string(), value.clone());
        }
    }

    Value::Object(payload)
}

pub(crate) fn materialize_start_builtin_defaults(start_payload: &mut Map<String, Value>) {
    start_payload
        .entry("system".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    start_payload
        .entry("model".to_string())
        .or_insert_with(|| Value::String(String::new()));
    start_payload
        .entry("reasoning_effort".to_string())
        .or_insert_with(|| Value::String(String::new()));
    start_payload
        .entry("max_output_tokens".to_string())
        .or_insert(Value::Null);
    start_payload
        .entry("history".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    start_payload
        .entry("files".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    start_payload
        .entry("tools".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    start_payload
        .entry("tool_choice".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
}
