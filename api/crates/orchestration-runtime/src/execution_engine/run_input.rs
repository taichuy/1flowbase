use super::*;
use domain::AiNativeOperation;
use plugin_framework::provider_contract::{
    NativeModelPromptContext, NativeModelRequestContext, ProtocolContextEnvelope,
    ProviderInvocationCapability, CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
    NATIVE_MODEL_PROMPT_CONTEXT_PAYLOAD_KEY, NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY,
};
use std::sync::Arc;

const RUNTIME_TOOL_REGISTRATIONS_KEY: &str = "tool_registrations";

#[derive(Clone, Debug)]
struct FrozenRuntimeInternalToolRegistration {
    provider_name: String,
}

#[derive(Clone, Default)]
pub struct ExecutionRuntimeContext {
    operation: AiNativeOperation,
    pub(super) tools: Vec<Value>,
    frozen_runtime_internal_tools: BTreeMap<String, FrozenRuntimeInternalToolRegistration>,
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
        let frozen_runtime_internal_tools =
            frozen_runtime_internal_tool_registrations(plan, variable_pool);
        let mut tools = run_level_provider_tools(plan, variable_pool);
        let frozen_provider_names = frozen_runtime_internal_tools
            .values()
            .map(|registration| registration.provider_name.as_str())
            .collect::<BTreeSet<_>>();
        tools.retain(|tool| {
            runtime_provider_tool_name(tool)
                .as_deref()
                .is_none_or(|name| !frozen_provider_names.contains(name))
        });
        Ok(Self {
            operation: ai_native_operation_from_variable_pool(plan, variable_pool)?,
            tools,
            frozen_runtime_internal_tools,
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
            if let Some(frozen) = self
                .frozen_runtime_internal_tools
                .get(&registration.registration_id)
            {
                registration.provider_name = frozen.provider_name.clone();
                set_runtime_provider_tool_name(
                    &mut registration.provider_tool,
                    &registration.provider_name,
                );
                continue;
            }
            if occupied.insert(registration.provider_name.clone()) {
                continue;
            }
            let qualified = qualified_runtime_internal_tool_name(
                &registration.provider_name,
                &registration.registration_id,
            );
            registration.provider_name = qualified.clone();
            set_runtime_provider_tool_name(&mut registration.provider_tool, &qualified);
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

fn set_runtime_provider_tool_name(tool: &mut Value, provider_name: &str) {
    if let Some(function) = tool.get_mut("function").and_then(Value::as_object_mut) {
        function.insert("name".to_string(), Value::String(provider_name.to_string()));
    } else if let Some(object) = tool.as_object_mut() {
        object.insert("name".to_string(), Value::String(provider_name.to_string()));
    }
}

fn frozen_runtime_internal_tool_registrations(
    plan: &CompiledPlan,
    variable_pool: &Map<String, Value>,
) -> BTreeMap<String, FrozenRuntimeInternalToolRegistration> {
    plan.topological_order
        .iter()
        .filter_map(|node_id| plan.nodes.get(node_id))
        .filter(|node| matches!(node.node_type.as_str(), "start" | "workflow_start"))
        .filter_map(|node| variable_pool.get(&node.node_id))
        .filter_map(|payload| payload.get(RUNTIME_TOOL_REGISTRATIONS_KEY))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|snapshot| {
            let registration_id = snapshot.get("registration_id")?.as_str()?.to_string();
            let provider_name = snapshot.get("provider_name")?.as_str()?.to_string();
            Some((
                registration_id,
                FrozenRuntimeInternalToolRegistration { provider_name },
            ))
        })
        .collect()
}

pub(super) fn frozen_runtime_internal_provider_names(
    plan: &CompiledPlan,
    variable_pool: &Map<String, Value>,
) -> BTreeSet<String> {
    frozen_runtime_internal_tool_registrations(plan, variable_pool)
        .into_values()
        .map(|registration| registration.provider_name)
        .collect()
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
    materialize_run_level_internal_tool_registrations(plan, variable_pool, runtime_context);
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

fn materialize_run_level_internal_tool_registrations(
    plan: &CompiledPlan,
    variable_pool: &mut Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
) {
    let already_frozen = plan
        .topological_order
        .iter()
        .filter_map(|node_id| plan.nodes.get(node_id))
        .filter(|node| matches!(node.node_type.as_str(), "start" | "workflow_start"))
        .filter_map(|node| variable_pool.get(&node.node_id))
        .filter_map(|payload| payload.get(RUNTIME_TOOL_REGISTRATIONS_KEY))
        .any(|snapshots| snapshots.as_array().is_some_and(|items| !items.is_empty()));
    if already_frozen {
        return;
    }

    let mut registrations =
        BTreeMap::<String, (RuntimeInternalToolRegistration, BTreeSet<String>)>::new();
    for node in plan
        .topological_order
        .iter()
        .filter_map(|node_id| plan.nodes.get(node_id))
        .filter(|node| node.node_type == "llm")
    {
        for registration in runtime_context.runtime_internal_tool_registrations(node) {
            let is_run_level = registration
                .owner
                .get("source")
                .and_then(|source| source.get("kind"))
                .and_then(Value::as_str)
                == Some("run");
            if !is_run_level {
                continue;
            }
            registrations
                .entry(registration.registration_id.clone())
                .and_modify(|(_, node_ids)| {
                    node_ids.insert(node.node_id.clone());
                })
                .or_insert_with(|| (registration, BTreeSet::from([node.node_id.clone()])));
        }
    }
    if registrations.is_empty() {
        return;
    }

    let provider_tools = registrations
        .values()
        .map(|(registration, _)| registration.provider_tool.clone())
        .collect::<Vec<_>>();
    let snapshots = registrations
        .into_values()
        .map(|(registration, node_ids)| {
            json!({
                "registration_id": registration.registration_id,
                "provider_name": registration.provider_name,
                "execution_kind": "host_internal",
                "owner": registration.owner,
                "node_ids": node_ids,
            })
        })
        .collect::<Vec<_>>();

    for node in plan
        .topological_order
        .iter()
        .filter_map(|node_id| plan.nodes.get(node_id))
        .filter(|node| matches!(node.node_type.as_str(), "start" | "workflow_start"))
    {
        let Some(start_payload) = variable_pool
            .entry(node.node_id.clone())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
        else {
            continue;
        };
        let tools = start_payload
            .entry("tools".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(tools) = tools.as_array_mut() {
            let mut occupied = tools
                .iter()
                .filter_map(runtime_provider_tool_name)
                .collect::<BTreeSet<_>>();
            for provider_tool in &provider_tools {
                let provider_name = runtime_provider_tool_name(provider_tool);
                if provider_name
                    .as_ref()
                    .is_some_and(|name| !occupied.insert(name.clone()))
                {
                    continue;
                }
                tools.push(provider_tool.clone());
            }
        }
        start_payload.insert(
            RUNTIME_TOOL_REGISTRATIONS_KEY.to_string(),
            Value::Array(snapshots.clone()),
        );
    }
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
