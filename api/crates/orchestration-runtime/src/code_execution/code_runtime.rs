use std::{error::Error, fmt, fs, time::Duration};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::{
    code_executor_capability::select_code_executor,
    compiled_plan::{CompiledCodeDependency, CompiledCodeRuntime, CompiledNode, CompiledPlan},
    output_schema::{validate_output_value, ProtocolContextContractViolation},
    payload_builder::{
        is_reserved_payload_key, BuiltNodePayloads, PublicOutputContract, RawNodeExecutionResult,
    },
};

use sync_js_runtime::{CancellationToken, Limits, ScriptDependency};
pub use sync_js_runtime::{ConsoleLogEntry, ExecutionOutput as CodeInvocationOutput};

#[async_trait]
pub trait CodeInvoker: Send + Sync {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CodeInvocationOutput>;

    /// Replaces protocol-context values selected from Code output with durable-safe locators
    /// after in-memory contract validation and before node tracing or checkpoint creation.
    async fn protect_protocol_context_output(
        &self,
        output: &mut CodeInvocationOutput,
        selected_output_paths: &[Vec<String>],
    ) -> Result<()> {
        if selected_output_paths.iter().any(|path| {
            read_value_path(&output.output_payload, path).is_some_and(|value| !value.is_null())
        }) {
            return Err(anyhow!("ephemeral_protocol_context_store_unavailable"));
        }
        Ok(())
    }

    /// Scrubs protocol-context values from Code console logs produced on an error path where no
    /// output exists to protect.
    async fn protect_protocol_context_logs(
        &self,
        _console_logs: &mut Vec<ConsoleLogEntry>,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct QuickJsCodeInvoker {
    timeout_override: Option<Duration>,
    #[cfg(test)]
    lifecycle_probe: Option<std::sync::Arc<ExecutionProbe>>,
}

impl QuickJsCodeInvoker {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_override = Some(timeout);
        self
    }
}

#[async_trait]
impl CodeInvoker for QuickJsCodeInvoker {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        _config_payload: Value,
        input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        select_code_executor(
            &runtime.isolation_profile,
            &runtime.language,
            &runtime.dependencies,
            &[crate::compiled_plan::CodeExecutorCapability::quickjs_local()],
        )?;
        let request = QuickJsInvocationRequest {
            language: runtime.language.clone(),
            source: runtime.source.clone(),
            entrypoint: runtime.entrypoint.clone(),
            dependencies: runtime.dependencies.clone(),
            input_payload,
            timeout: self
                .timeout_override
                .unwrap_or_else(|| Duration::from_millis(runtime.isolation_profile.timeout_ms)),
            memory_limit_bytes: runtime.isolation_profile.memory_mb as usize * 1024 * 1024,
            stack_size_bytes: runtime.isolation_profile.stack_kb as usize * 1024,
        };

        let task = CodeExecutionTask::spawn(request);
        #[cfg(test)]
        let task = task.with_probe(self.lifecycle_probe.clone());
        task.wait().await
    }
}

struct QuickJsInvocationRequest {
    language: String,
    source: Option<String>,
    entrypoint: String,
    dependencies: Vec<CompiledCodeDependency>,
    input_payload: Value,
    timeout: Duration,
    memory_limit_bytes: usize,
    stack_size_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeRunnerErrorKind {
    UnsupportedLanguage,
    SourceMissing,
    EntrypointUnsupported,
    DependencyArtifactMissing,
    DependencyIntegrityMismatch,
    RuntimeError,
}

#[derive(Debug, Clone, PartialEq)]
struct CodeRunnerError {
    kind: CodeRunnerErrorKind,
}

impl CodeRunnerError {
    fn new(kind: CodeRunnerErrorKind) -> Self {
        Self { kind }
    }

    fn unsupported_language() -> Self {
        Self::new(CodeRunnerErrorKind::UnsupportedLanguage)
    }

    fn source_missing() -> Self {
        Self::new(CodeRunnerErrorKind::SourceMissing)
    }

    fn entrypoint_unsupported() -> Self {
        Self::new(CodeRunnerErrorKind::EntrypointUnsupported)
    }

    fn dependency_artifact_missing() -> Self {
        Self::new(CodeRunnerErrorKind::DependencyArtifactMissing)
    }

    fn dependency_integrity_mismatch() -> Self {
        Self::new(CodeRunnerErrorKind::DependencyIntegrityMismatch)
    }

    fn runtime_error() -> Self {
        Self::new(CodeRunnerErrorKind::RuntimeError)
    }

    fn public_code(&self) -> &'static str {
        match self.kind {
            CodeRunnerErrorKind::UnsupportedLanguage => "unsupported_language",
            CodeRunnerErrorKind::SourceMissing => "source_missing",
            CodeRunnerErrorKind::EntrypointUnsupported => "entrypoint_unsupported",
            CodeRunnerErrorKind::DependencyArtifactMissing => "dependency_artifact_missing",
            CodeRunnerErrorKind::DependencyIntegrityMismatch => "dependency_integrity_mismatch",
            CodeRunnerErrorKind::RuntimeError => "runtime_error",
        }
    }

    fn public_message(&self) -> &'static str {
        match self.kind {
            CodeRunnerErrorKind::UnsupportedLanguage => "only JavaScript code runtime is supported",
            CodeRunnerErrorKind::SourceMissing => "code source is required",
            CodeRunnerErrorKind::EntrypointUnsupported => "only main entrypoint is supported",
            CodeRunnerErrorKind::DependencyArtifactMissing => "dependency artifact is missing",
            CodeRunnerErrorKind::DependencyIntegrityMismatch => {
                "dependency artifact integrity mismatch"
            }
            CodeRunnerErrorKind::RuntimeError => "code execution failed",
        }
    }
}

impl fmt::Display for CodeRunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.public_code(),
            self.public_message()
        )
    }
}

impl Error for CodeRunnerError {}

fn load_dependency_artifacts(
    dependencies: &[CompiledCodeDependency],
    cancel: &CancellationToken,
) -> Result<Vec<ScriptDependency>> {
    dependencies
        .iter()
        .map(|dependency| {
            check_code_cancellation(cancel)?;
            let mut bytes = dependency.alias.bytes();
            if !bytes
                .next()
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_' || b == b'$')
                || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
            {
                return Err(sync_js_runtime::ExecutionError::from(
                    sync_js_runtime::ErrorKind::DependencyAliasInvalid,
                )
                .into());
            }
            let source = fs::read_to_string(&dependency.artifact_path)
                .map_err(|_| CodeRunnerError::dependency_artifact_missing())?;
            check_code_cancellation(cancel)?;
            verify_dependency_integrity(&source, dependency)?;
            Ok(ScriptDependency {
                alias: dependency.alias.clone(),
                source,
            })
        })
        .collect()
}

fn verify_dependency_integrity(source: &str, dependency: &CompiledCodeDependency) -> Result<()> {
    let actual = format!(
        "{:x}",
        base16ct::HexDisplay(&Sha256::digest(source.as_bytes()))
    );
    for expected in [&dependency.artifact_hash, &dependency.integrity] {
        let normalized = normalize_sha256(expected);
        if normalized.as_deref() != Some(actual.as_str()) {
            return Err(CodeRunnerError::dependency_integrity_mismatch().into());
        }
    }
    Ok(())
}

fn normalize_sha256(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .strip_prefix("sha256:")
        .or_else(|| trimmed.strip_prefix("sha256-"))
        .or(Some(trimmed))
        .map(str::to_ascii_lowercase)
}

/// Holds the only carrier join owner. Dropping a caller requests stop immediately;
/// a cooperative-interrupt cleanup task retains and awaits the blocking JoinHandle.
struct CodeExecutionTask {
    cancel: CancellationToken,
    join: Option<tokio::task::JoinHandle<Result<CodeInvocationOutput>>>,
    #[cfg(test)]
    probe: Option<std::sync::Arc<ExecutionProbe>>,
}
impl CodeExecutionTask {
    fn spawn(request: QuickJsInvocationRequest) -> Self {
        let cancel = CancellationToken::default();
        let stop = cancel.clone();
        let join = tokio::task::spawn_blocking(move || run_quickjs_code(request, stop));
        Self {
            cancel,
            join: Some(join),
            #[cfg(test)]
            probe: None,
        }
    }
    #[cfg(test)]
    fn with_probe(mut self, probe: Option<std::sync::Arc<ExecutionProbe>>) -> Self {
        if let Some(probe) = &probe {
            *probe.cancel.lock().unwrap() = Some(self.cancel.clone());
        }
        self.probe = probe;
        self
    }
    async fn wait(mut self) -> Result<CodeInvocationOutput> {
        let result = self.join.as_mut().expect("execution join owner").await;
        self.join.take();
        #[cfg(test)]
        if let Some(probe) = &self.probe {
            probe.joined(&self.cancel);
        }
        result.map_err(|_| CodeRunnerError::runtime_error())?
    }
}
impl Drop for CodeExecutionTask {
    fn drop(&mut self) {
        let Some(join) = self.join.take() else {
            return;
        };
        self.cancel.cancel();
        #[cfg(test)]
        if let Some(probe) = &self.probe {
            probe.events.lock().unwrap().push("stop");
        }
        #[cfg(test)]
        let observation = (self.probe.take(), self.cancel.clone());
        // This is an owned cleanup continuation, not an aborted/detached JS carrier.
        // Tokio shutdown also waits for the blocking carrier; it has already been stopped.
        tokio::spawn(async move {
            let _ = join.await;
            #[cfg(test)]
            if let (Some(probe), cancel) = observation {
                probe.joined(&cancel);
            }
        });
    }
}

fn check_code_cancellation(cancel: &CancellationToken) -> Result<()> {
    if cancel.is_cancelled() {
        return Err(
            sync_js_runtime::ExecutionError::from(sync_js_runtime::ErrorKind::Cancelled).into(),
        );
    }
    Ok(())
}

fn run_quickjs_code(
    request: QuickJsInvocationRequest,
    cancel: CancellationToken,
) -> Result<CodeInvocationOutput> {
    check_code_cancellation(&cancel)?;
    if !request.language.eq_ignore_ascii_case("javascript") {
        return Err(CodeRunnerError::unsupported_language().into());
    }
    if request.entrypoint != "main" {
        return Err(CodeRunnerError::entrypoint_unsupported().into());
    }
    let source = request
        .source
        .as_deref()
        .filter(|source| !source.trim().is_empty())
        .ok_or_else(CodeRunnerError::source_missing)?;
    sync_js_runtime::validate_legacy_source(source)?;
    check_code_cancellation(&cancel)?;
    let dependencies = load_dependency_artifacts(&request.dependencies, &cancel)?;
    check_code_cancellation(&cancel)?;
    sync_js_runtime::run_legacy_script_cancellable(
        source,
        &request.input_payload,
        Limits {
            timeout: request.timeout,
            memory_limit_bytes: request.memory_limit_bytes,
            stack_size_bytes: request.stack_size_bytes,
            max_output_bytes: request.memory_limit_bytes,
        },
        &dependencies,
        cancel,
    )
    .map_err(Into::into)
}

#[cfg(test)]
#[derive(Debug, Default)]
struct ExecutionProbe {
    cancel: std::sync::Mutex<Option<CancellationToken>>,
    events: std::sync::Mutex<Vec<&'static str>>,
}
#[cfg(test)]
impl ExecutionProbe {
    fn joined(&self, cancel: &CancellationToken) {
        let exited = if cancel.has_started() {
            assert!(cancel.has_exited());
            "js_exited"
        } else {
            "not_started"
        };
        self.events.lock().unwrap().extend([exited, "joined"]);
    }
}

#[cfg(test)]
#[path = "_tests/shared_runtime.rs"]
mod shared_runtime_tests;

#[derive(Debug, Clone, PartialEq)]
pub struct CodeNodeExecution {
    pub output_payload: Value,
    pub error_payload: Option<Value>,
    pub metrics_payload: Value,
    pub debug_payload: Value,
}

pub async fn execute_code_node<I>(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    invoker: &I,
) -> Result<CodeNodeExecution>
where
    I: CodeInvoker + ?Sized,
{
    let runtime = node.code_runtime.as_ref().ok_or_else(|| {
        anyhow!(
            "compiled code node is missing runtime metadata: {}",
            node.node_id
        )
    })?;
    let config_payload = node.config.clone();
    let mut input_object = resolved_inputs.clone();

    if let Some(Value::Object(named_bindings)) = resolved_inputs.get("named_bindings") {
        for (key, value) in named_bindings {
            input_object.insert(key.clone(), value.clone());
        }
    }

    let input_payload = Value::Object(input_object);

    match invoker
        .invoke_code_node(runtime, config_payload, input_payload)
        .await
    {
        Ok(mut output) => {
            let raw_executor_output = object_from_value(output.output_payload.clone())?;
            let output_contract_error =
                validate_code_output_contract(node, &raw_executor_output).err();
            let protocol_context_output_paths = protocol_context_output_paths(plan, &node.node_id)?;
            if let Err(error) = invoker
                .protect_protocol_context_output(&mut output, &protocol_context_output_paths)
                .await
            {
                return failed_code_runtime_execution(node, runtime, &error, invoker).await;
            }
            let debug_facts = code_runtime_debug_facts(&output.console_logs)?;
            let executor_output = object_from_value(output.output_payload)?;
            if let Some(error) = output_contract_error {
                let contract_violation = error.downcast_ref::<ProtocolContextContractViolation>();
                let error_facts = if let Some(violation) = contract_violation {
                    object_from_value(json!({
                        "error_code": "protocol_context_contract_violation",
                        "message": "protocol context output contract validation failed",
                        "node_id": node.node_id,
                        "selector": format!("{}.{}", node.node_id, violation.selector),
                        "instance_path": violation.instance_path,
                        "reason": violation.reason,
                    }))?
                } else {
                    object_from_value(json!({
                        "error_code": "code_output_contract_error",
                        "message": "code output contract validation failed",
                        "runtime_message": error.to_string(),
                    }))?
                };
                let raw = RawNodeExecutionResult {
                    executor_output: Map::new(),
                    metrics_facts: code_runtime_metrics(runtime, true)?,
                    error_facts,
                    debug_facts,
                    provider_events: Vec::new(),
                };
                let built = build_code_node_payloads(node, raw)?;

                return Ok(CodeNodeExecution {
                    output_payload: built.output_payload,
                    error_payload: Some(built.error_payload),
                    metrics_payload: built.metrics_payload,
                    debug_payload: built.debug_payload,
                });
            }
            let raw = RawNodeExecutionResult {
                executor_output,
                metrics_facts: code_runtime_metrics(runtime, false)?,
                error_facts: Map::new(),
                debug_facts,
                provider_events: Vec::new(),
            };
            let built = build_code_node_payloads(node, raw)?;

            Ok(CodeNodeExecution {
                output_payload: built.output_payload,
                error_payload: None,
                metrics_payload: built.metrics_payload,
                debug_payload: built.debug_payload,
            })
        }
        Err(error) => failed_code_runtime_execution(node, runtime, &error, invoker).await,
    }
}

async fn failed_code_runtime_execution<I>(
    node: &CompiledNode,
    runtime: &CompiledCodeRuntime,
    error: &anyhow::Error,
    invoker: &I,
) -> Result<CodeNodeExecution>
where
    I: CodeInvoker + ?Sized,
{
    let mut console_logs = error
        .downcast_ref::<sync_js_runtime::ExecutionError>()
        .map(|error| error.console_logs.clone())
        .unwrap_or_default();
    let runtime_message = match invoker
        .protect_protocol_context_logs(&mut console_logs)
        .await
    {
        Ok(()) => error.to_string(),
        Err(protection_error) => {
            console_logs.clear();
            protection_error.to_string()
        }
    };
    let raw = RawNodeExecutionResult {
        executor_output: Map::new(),
        metrics_facts: code_runtime_metrics(runtime, true)?,
        error_facts: object_from_value(json!({
            "error_code": "code_runtime_error",
            "message": "code execution failed",
            "runtime_message": runtime_message,
        }))?,
        debug_facts: code_runtime_debug_facts(&console_logs)?,
        provider_events: Vec::new(),
    };
    let built = build_code_node_payloads(node, raw)?;

    Ok(CodeNodeExecution {
        output_payload: built.output_payload,
        error_payload: Some(built.error_payload),
        metrics_payload: built.metrics_payload,
        debug_payload: built.debug_payload,
    })
}

fn protocol_context_output_paths(
    plan: &CompiledPlan,
    code_node_id: &str,
) -> Result<Vec<Vec<String>>> {
    let mut paths = Vec::new();
    for node in plan.nodes.values().filter(|node| node.node_type == "llm") {
        let Some(reference) = node.protocol_context_reference()? else {
            continue;
        };
        if reference.is_system_protocol_context() {
            continue;
        }
        let selector = reference.selector_path();
        let Some((source_node_id, output_path)) = selector.split_first() else {
            continue;
        };
        if source_node_id != code_node_id {
            continue;
        }
        let output_path = if output_path
            .first()
            .is_some_and(|segment| segment == "result")
        {
            &output_path[1..]
        } else {
            output_path
        };
        let output_path = output_path.to_vec();
        let selected_output = plan.nodes.get(code_node_id).and_then(|source| {
            source.outputs.iter().find(|output| {
                let declared_path = output
                    .selector
                    .strip_prefix(&["result".to_string()])
                    .unwrap_or(&output.selector);
                declared_path == output_path
                    || (declared_path.is_empty()
                        && output_path.last().is_some_and(|key| key == &output.key))
            })
        });
        if selected_output.is_some_and(|output| {
            output.value_type == extension_contracts::provider_contract::PROTOCOL_CONTEXT_VALUE_TYPE
        }) {
            continue;
        }
        paths.push(output_path);
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn read_value_path<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.as_object()?.get(segment)?;
    }
    Some(current)
}

fn validate_code_output_contract(
    node: &CompiledNode,
    executor_output: &Map<String, Value>,
) -> Result<()> {
    let mut envelope = Map::new();
    envelope.insert("result".to_string(), Value::Object(executor_output.clone()));
    envelope.insert("error".to_string(), Value::Null);

    for output in &node.outputs {
        let selector = if output.selector.is_empty() {
            vec!["result".to_string(), output.key.clone()]
        } else {
            output.selector.clone()
        };
        if let Some(value) = read_output_selector(&envelope, &selector) {
            validate_output_value(output, value)?;
        }
    }

    Ok(())
}

fn read_output_selector<'a>(
    output_payload: &'a Map<String, Value>,
    selector: &[String],
) -> Option<&'a Value> {
    let (first, rest) = selector.split_first()?;
    let mut current = output_payload.get(first)?;

    for segment in rest {
        current = current.as_object()?.get(segment)?;
    }

    Some(current)
}

fn code_runtime_debug_facts(console_logs: &[ConsoleLogEntry]) -> Result<Map<String, Value>> {
    let mut debug_facts = Map::new();
    if !console_logs.is_empty() {
        debug_facts.insert(
            "console_logs".to_string(),
            serde_json::to_value(console_logs)?,
        );
    }
    Ok(debug_facts)
}

fn code_runtime_metrics(runtime: &CompiledCodeRuntime, error: bool) -> Result<Map<String, Value>> {
    object_from_value(json!({
        "language": runtime.language,
        "entrypoint": runtime.entrypoint,
        "imports": runtime.imports,
        "dependency_count": runtime.dependencies.len(),
        "executor_id": runtime.isolation_profile.executor_id,
        "isolation_mode": runtime.isolation_profile.mode,
        "timeout_ms": runtime.isolation_profile.timeout_ms,
        "memory_mb": runtime.isolation_profile.memory_mb,
        "stack_kb": runtime.isolation_profile.stack_kb,
        "error": error,
    }))
}

fn build_code_node_payloads(
    node: &CompiledNode,
    raw: RawNodeExecutionResult,
) -> Result<BuiltNodePayloads> {
    for key in raw.executor_output.keys() {
        if is_reserved_payload_key(key) {
            return Err(anyhow!(
                "reserved code output key `{key}` cannot be returned by code node executor"
            ));
        }
    }

    let mut debug_payload = raw.debug_facts.clone();
    if !raw.provider_events.is_empty() {
        debug_payload.insert(
            "provider_events".to_string(),
            serde_json::to_value(raw.provider_events)?,
        );
    }

    let error_payload = Value::Object(raw.error_facts.clone());
    let result_payload = if raw.error_facts.is_empty() {
        Value::Object(raw.executor_output)
    } else {
        Value::Null
    };
    let output_payload = json!({
        "result": result_payload,
        "error": if raw.error_facts.is_empty() {
            Value::Null
        } else {
            error_payload.clone()
        },
    });

    PublicOutputContract::from_compiled_outputs(&node.outputs)?;
    Ok(BuiltNodePayloads {
        output_payload,
        metrics_payload: Value::Object(raw.metrics_facts),
        error_payload,
        debug_payload: Value::Object(debug_payload),
    })
}

fn object_from_value(value: Value) -> Result<Map<String, Value>> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("payload bucket facts must be an object"))
}
