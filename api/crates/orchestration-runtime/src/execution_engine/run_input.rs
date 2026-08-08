use super::*;
use domain::AiNativeOperation;
use plugin_framework::provider_contract::{
    NativeModelPromptContext, NativeModelRequestContext, ProtocolContextEnvelope,
    ProviderInvocationCapability, CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
    NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY, NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY,
};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ExecutionRuntimeContext {
    operation: AiNativeOperation,
    pub(super) tools: Vec<Value>,
    protocol_context: RuntimeProtocolContext,
    pub(super) native_model_prompt_context: NativeModelPromptContext,
    pub(super) native_model_request_context: NativeModelRequestContext,
    pub(super) llm_routing_counter_store: Option<Arc<dyn LlmRoutingCounterStore>>,
    pub(super) http_response_file_persister: Option<Arc<dyn HttpResponseFilePersister>>,
    pub(super) provider_invocation_capabilities: BTreeSet<ProviderInvocationCapability>,
    pub(super) runtime_internal_tool_invoker: Option<Arc<dyn RuntimeInternalToolInvoker>>,
}

#[derive(Clone, Default)]
enum RuntimeProtocolContext {
    #[default]
    Absent,
    Available {
        envelope: ProtocolContextEnvelope,
        locator: Option<Value>,
    },
    Unavailable {
        locator: Value,
        reason: String,
    },
}

impl ExecutionRuntimeContext {
    pub fn from_plan_input(
        plan: &CompiledPlan,
        variable_pool: &Map<String, Value>,
    ) -> Result<Self> {
        Ok(Self {
            operation: ai_native_operation_from_variable_pool(plan, variable_pool)?,
            tools: run_level_provider_tools(plan, variable_pool),
            protocol_context: RuntimeProtocolContext::Absent,
            native_model_prompt_context: native_model_prompt_context_from_variable_pool(
                variable_pool,
            )?,
            native_model_request_context: native_model_request_context_from_variable_pool(
                variable_pool,
            )?,
            llm_routing_counter_store: None,
            http_response_file_persister: None,
            provider_invocation_capabilities: BTreeSet::new(),
            runtime_internal_tool_invoker: None,
        })
    }

    pub fn operation(&self) -> AiNativeOperation {
        self.operation
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

    pub fn with_provider_invocation_capability(
        mut self,
        capability: ProviderInvocationCapability,
    ) -> Self {
        self.provider_invocation_capabilities.insert(capability);
        self
    }

    pub fn with_runtime_internal_tool_invoker(
        mut self,
        invoker: Arc<dyn RuntimeInternalToolInvoker>,
    ) -> Self {
        self.runtime_internal_tool_invoker = Some(invoker);
        self
    }

    pub(super) fn runtime_internal_tool_registrations(
        &self,
        node: &CompiledNode,
    ) -> Vec<RuntimeInternalToolRegistration> {
        let mut registrations = self
            .runtime_internal_tool_invoker
            .as_ref()
            .map(|invoker| invoker.registrations_for_node(node))
            .unwrap_or_default();
        let mut occupied = self
            .tools
            .iter()
            .filter_map(runtime_provider_tool_name)
            .collect::<BTreeSet<_>>();
        occupied.extend(
            node.config
                .get("tools")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(runtime_provider_tool_name),
        );
        for registration in &mut registrations {
            if occupied.insert(registration.provider_name.clone()) {
                continue;
            }
            let qualified = qualified_runtime_internal_tool_name(
                &registration.provider_name,
                &registration.registration_id,
            );
            registration.provider_name = qualified.clone();
            if let Some(function) = registration
                .provider_tool
                .get_mut("function")
                .and_then(Value::as_object_mut)
            {
                function.insert("name".to_string(), Value::String(qualified.clone()));
            }
            occupied.insert(qualified);
        }
        registrations
    }

    pub fn with_protocol_context(mut self, protocol_context: ProtocolContextEnvelope) -> Self {
        self.protocol_context = RuntimeProtocolContext::Available {
            envelope: protocol_context,
            locator: None,
        };
        self
    }

    pub fn with_ephemeral_protocol_context(
        mut self,
        locator: Value,
        protocol_context: ProtocolContextEnvelope,
    ) -> Self {
        self.protocol_context = RuntimeProtocolContext::Available {
            envelope: protocol_context,
            locator: Some(locator),
        };
        self
    }

    pub fn with_unavailable_ephemeral_protocol_context(
        mut self,
        locator: Value,
        reason: impl Into<String>,
    ) -> Self {
        self.protocol_context = RuntimeProtocolContext::Unavailable {
            locator,
            reason: reason.into(),
        };
        self
    }

    pub(super) fn resolved_protocol_context(
        &self,
    ) -> std::result::Result<Option<ProtocolContextEnvelope>, &str> {
        match &self.protocol_context {
            RuntimeProtocolContext::Absent => Ok(None),
            RuntimeProtocolContext::Available { envelope, .. } => Ok(Some(envelope.clone())),
            RuntimeProtocolContext::Unavailable { reason, .. } => Err(reason),
        }
    }

    fn protocol_context_locator(&self) -> Option<&Value> {
        match &self.protocol_context {
            RuntimeProtocolContext::Available { locator, .. } => locator.as_ref(),
            RuntimeProtocolContext::Unavailable { locator, .. } => Some(locator),
            RuntimeProtocolContext::Absent => None,
        }
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

fn runtime_provider_tool_name(tool: &Value) -> Option<String> {
    tool.get("function")
        .and_then(|function| function.get("name"))
        .or_else(|| tool.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn qualified_runtime_internal_tool_name(base: &str, registration_id: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    registration_id.hash(&mut hasher);
    let suffix = format!("_{:010x}", hasher.finish() & 0xffffffffff);
    let keep = 64usize.saturating_sub(suffix.len());
    format!("{}{suffix}", &base[..base.len().min(keep)])
}

fn ai_native_operation_from_variable_pool(
    plan: &CompiledPlan,
    variable_pool: &Map<String, Value>,
) -> Result<AiNativeOperation> {
    let Some(start) = plan.nodes.values().find(|node| node.node_type == "start") else {
        return Ok(AiNativeOperation::default());
    };
    let Some(operation) = variable_pool
        .get(&start.node_id)
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("operation"))
    else {
        return Ok(AiNativeOperation::default());
    };

    serde_json::from_value(operation.clone()).map_err(|error| {
        anyhow!(
            "invalid AI Native operation at {}.operation: {error}",
            start.node_id
        )
    })
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

pub fn normalize_plan_variable_pool(plan: &CompiledPlan, variable_pool: &mut Map<String, Value>) {
    variable_pool.remove(CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY);
    let [namespace, field] = crate::compiled_plan::SYSTEM_PROTOCOL_CONTEXT_SELECTOR;
    if let Some(sys) = variable_pool
        .get_mut(namespace)
        .and_then(Value::as_object_mut)
    {
        sys.remove(field);
    }

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

    let start_protocol_context = runtime_context
        .resolved_protocol_context()
        .ok()
        .flatten()
        .map(|envelope| {
            serde_json::to_value(envelope)
                .expect("the canonical ProtocolContextEnvelope must serialize")
        })
        .unwrap_or(Value::Null);
    for node in plan.nodes.values().filter(|node| node.node_type == "start") {
        if let Some(start_payload) = variable_pool
            .get_mut(&node.node_id)
            .and_then(Value::as_object_mut)
        {
            start_payload.insert(
                "protocol_context".to_string(),
                start_protocol_context.clone(),
            );
        }
    }

    if let Some(protocol_context_locator) = runtime_context.protocol_context_locator() {
        let sys = variable_pool
            .entry("sys".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(sys) = sys.as_object_mut() {
            sys.insert(
                "protocol_context".to_string(),
                protocol_context_locator.clone(),
            );
        }
    }

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
        .entry("operation".to_string())
        .or_insert_with(|| {
            serde_json::to_value(AiNativeOperation::default())
                .expect("the canonical AI Native operation must serialize")
        });
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
    start_payload
        .entry("protocol_context".to_string())
        .or_insert(Value::Null);
}
