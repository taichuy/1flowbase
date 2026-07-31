use std::{
    error::Error,
    fmt, fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rquickjs::{Context, Ctx, Runtime};
use serde::{Deserialize, Serialize};
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

const INVALID_OUTPUT_SENTINEL: &str = "__1flowbase_invalid_output__";

#[derive(Debug, Clone, PartialEq)]
pub struct CodeInvocationOutput {
    pub output_payload: Value,
    pub console_logs: Vec<ConsoleLogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsoleLogEntry {
    pub level: String,
    pub message: String,
    pub args: Vec<Value>,
}

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

        tokio::task::spawn_blocking(move || run_quickjs_code(request))
            .await
            .map_err(|_| CodeRunnerError::runtime_error())?
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
    ModuleImportDenied,
    DynamicImportDenied,
    DependencyArtifactMissing,
    DependencyIntegrityMismatch,
    DependencyArtifactInvalid,
    DependencyAliasInvalid,
    SyntaxError,
    MainMissing,
    InvalidOutput,
    RuntimeError,
    Timeout,
}

#[derive(Debug, Clone, PartialEq)]
struct CodeRunnerError {
    kind: CodeRunnerErrorKind,
    console_logs: Vec<ConsoleLogEntry>,
}

impl CodeRunnerError {
    fn new(kind: CodeRunnerErrorKind) -> Self {
        Self {
            kind,
            console_logs: Vec::new(),
        }
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

    fn module_import_denied() -> Self {
        Self::new(CodeRunnerErrorKind::ModuleImportDenied)
    }

    fn dynamic_import_denied() -> Self {
        Self::new(CodeRunnerErrorKind::DynamicImportDenied)
    }

    fn dependency_artifact_missing() -> Self {
        Self::new(CodeRunnerErrorKind::DependencyArtifactMissing)
    }

    fn dependency_integrity_mismatch() -> Self {
        Self::new(CodeRunnerErrorKind::DependencyIntegrityMismatch)
    }

    fn dependency_artifact_invalid() -> Self {
        Self::new(CodeRunnerErrorKind::DependencyArtifactInvalid)
    }

    fn dependency_alias_invalid() -> Self {
        Self::new(CodeRunnerErrorKind::DependencyAliasInvalid)
    }

    fn syntax_error() -> Self {
        Self::new(CodeRunnerErrorKind::SyntaxError)
    }

    fn main_missing() -> Self {
        Self::new(CodeRunnerErrorKind::MainMissing)
    }

    fn invalid_output() -> Self {
        Self::new(CodeRunnerErrorKind::InvalidOutput)
    }

    fn runtime_error() -> Self {
        Self::new(CodeRunnerErrorKind::RuntimeError)
    }

    fn timeout() -> Self {
        Self::new(CodeRunnerErrorKind::Timeout)
    }

    fn with_console_logs(mut self, console_logs: Vec<ConsoleLogEntry>) -> Self {
        self.console_logs = console_logs;
        self
    }

    fn public_code(&self) -> &'static str {
        match self.kind {
            CodeRunnerErrorKind::UnsupportedLanguage => "unsupported_language",
            CodeRunnerErrorKind::SourceMissing => "source_missing",
            CodeRunnerErrorKind::EntrypointUnsupported => "entrypoint_unsupported",
            CodeRunnerErrorKind::ModuleImportDenied => "module_import_denied",
            CodeRunnerErrorKind::DynamicImportDenied => "module_import_denied",
            CodeRunnerErrorKind::DependencyArtifactMissing => "dependency_artifact_missing",
            CodeRunnerErrorKind::DependencyIntegrityMismatch => "dependency_integrity_mismatch",
            CodeRunnerErrorKind::DependencyArtifactInvalid => "dependency_artifact_invalid",
            CodeRunnerErrorKind::DependencyAliasInvalid => "dependency_alias_invalid",
            CodeRunnerErrorKind::SyntaxError => "syntax_error",
            CodeRunnerErrorKind::MainMissing => "main_missing",
            CodeRunnerErrorKind::InvalidOutput => "invalid_output",
            CodeRunnerErrorKind::RuntimeError => "runtime_error",
            CodeRunnerErrorKind::Timeout => "timeout",
        }
    }

    fn public_message(&self) -> &'static str {
        match self.kind {
            CodeRunnerErrorKind::UnsupportedLanguage => "only JavaScript code runtime is supported",
            CodeRunnerErrorKind::SourceMissing => "code source is required",
            CodeRunnerErrorKind::EntrypointUnsupported => "only main entrypoint is supported",
            CodeRunnerErrorKind::ModuleImportDenied => "import and export syntax is not supported",
            CodeRunnerErrorKind::DynamicImportDenied => "dynamic import is not supported",
            CodeRunnerErrorKind::DependencyArtifactMissing => "dependency artifact is missing",
            CodeRunnerErrorKind::DependencyIntegrityMismatch => {
                "dependency artifact integrity mismatch"
            }
            CodeRunnerErrorKind::DependencyArtifactInvalid => "dependency artifact is invalid",
            CodeRunnerErrorKind::DependencyAliasInvalid => "dependency alias is invalid",
            CodeRunnerErrorKind::SyntaxError => "code source is not valid JavaScript",
            CodeRunnerErrorKind::MainMissing => "main function is required",
            CodeRunnerErrorKind::InvalidOutput => "main must return a JSON object",
            CodeRunnerErrorKind::RuntimeError => "code execution failed",
            CodeRunnerErrorKind::Timeout => "code execution timed out",
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

struct LoadedCodeDependency {
    alias: String,
    source: String,
}

fn load_dependency_artifacts(
    dependencies: &[CompiledCodeDependency],
) -> Result<Vec<LoadedCodeDependency>> {
    dependencies
        .iter()
        .map(|dependency| {
            if !is_js_identifier(dependency.alias.as_str()) {
                return Err(CodeRunnerError::dependency_alias_invalid().into());
            }
            let source = fs::read_to_string(&dependency.artifact_path)
                .map_err(|_| CodeRunnerError::dependency_artifact_missing())?;
            verify_dependency_integrity(&source, dependency)?;
            reject_module_syntax(&source)
                .map_err(|_| CodeRunnerError::dependency_artifact_invalid())?;
            Ok(LoadedCodeDependency {
                alias: dependency.alias.clone(),
                source,
            })
        })
        .collect()
}

fn verify_dependency_integrity(source: &str, dependency: &CompiledCodeDependency) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(source.as_bytes()));
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

fn run_quickjs_code(request: QuickJsInvocationRequest) -> Result<CodeInvocationOutput> {
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
    reject_module_syntax(source)?;
    let dependencies = load_dependency_artifacts(&request.dependencies)?;

    let deadline = std::time::Instant::now() + request.timeout;
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupt_state = Arc::clone(&interrupted);
    let runtime = Runtime::new().map_err(|_| CodeRunnerError::runtime_error())?;
    runtime.set_memory_limit(request.memory_limit_bytes);
    runtime.set_max_stack_size(request.stack_size_bytes);
    runtime.set_interrupt_handler(Some(Box::new(move || {
        let expired = std::time::Instant::now() >= deadline;
        if expired {
            interrupt_state.store(true, Ordering::Relaxed);
        }
        expired
    })));

    let context = Context::full(&runtime).map_err(|_| CodeRunnerError::runtime_error())?;
    context.with(|ctx| {
        install_console_capture(&ctx).map_err(|_| CodeRunnerError::runtime_error())?;
        ctx.eval::<(), _>("globalThis.__dependencies = globalThis.__dependencies || {};")
            .map_err(|_| CodeRunnerError::runtime_error())?;
        for dependency in &dependencies {
            ctx.eval::<(), _>(dependency.source.as_str())
                .map_err(|_| CodeRunnerError::dependency_artifact_invalid())?;
            let alias_literal = serde_json::to_string(&dependency.alias)
                .map_err(|_| CodeRunnerError::runtime_error())?;
            let dependency_check = format!(
                r#"Object.prototype.hasOwnProperty.call(globalThis.__dependencies, {alias_literal})"#
            );
            let registered = ctx
                .eval::<bool, _>(dependency_check)
                .map_err(|_| CodeRunnerError::dependency_artifact_invalid())?;
            if !registered {
                return Err(CodeRunnerError::dependency_artifact_invalid().into());
            }
            let facade_script = format!(
                "var {} = globalThis.__dependencies[{}];",
                dependency.alias, alias_literal
            );
            ctx.eval::<(), _>(facade_script)
                .map_err(|_| CodeRunnerError::dependency_artifact_invalid())?;
        }
        if ctx.eval::<(), _>(source).is_err() {
            return Err(code_error_for_elapsed(
                request.timeout,
                deadline,
                &interrupted,
                CodeRunnerError::syntax_error(),
            )
            .with_console_logs(read_console_logs(&ctx))
            .into());
        }
        let main_type = match ctx.eval::<String, _>("typeof main") {
            Ok(main_type) => main_type,
            Err(_) => {
                return Err(code_error_for_elapsed(
                request.timeout,
                deadline,
                &interrupted,
                CodeRunnerError::runtime_error(),
            )
                .with_console_logs(read_console_logs(&ctx))
                .into());
            }
        };
        if main_type != "function" {
            return Err(CodeRunnerError::main_missing()
                .with_console_logs(read_console_logs(&ctx))
                .into());
        }

        let input_literal = serde_json::to_string(&request.input_payload)
            .map_err(|_| CodeRunnerError::runtime_error())?;
        let invocation_script = build_invocation_script(&input_literal);
        let output_json = match ctx.eval::<String, _>(invocation_script) {
            Ok(output_json) => output_json,
            Err(_) => {
                return Err(code_error_for_elapsed(
                request.timeout,
                deadline,
                &interrupted,
                CodeRunnerError::runtime_error(),
            )
                .with_console_logs(read_console_logs(&ctx))
                .into());
            }
        };
        let console_logs = read_console_logs(&ctx);
        if output_json == INVALID_OUTPUT_SENTINEL {
            return Err(CodeRunnerError::invalid_output()
                .with_console_logs(console_logs)
                .into());
        }
        let output_payload = serde_json::from_str::<Value>(&output_json)
            .map_err(|_| CodeRunnerError::invalid_output().with_console_logs(console_logs.clone()))?;
        if !matches!(output_payload, Value::Object(_)) {
            return Err(CodeRunnerError::invalid_output()
                .with_console_logs(console_logs)
                .into());
        }

        Ok(CodeInvocationOutput {
            output_payload,
            console_logs,
        })
    })
}

fn install_console_capture(ctx: &Ctx<'_>) -> Result<()> {
    ctx.eval::<(), _>(
        r#"
(function () {
  const logs = [];

  function normalizeConsoleArg(value, seen) {
    if (value === undefined) {
      return { type: "undefined" };
    }
    if (value === null || typeof value === "string" || typeof value === "boolean") {
      return value;
    }
    if (typeof value === "number") {
      return Number.isFinite(value) ? value : String(value);
    }
    if (typeof value === "bigint" || typeof value === "symbol") {
      return String(value);
    }
    if (typeof value === "function") {
      return value.name ? "[Function " + value.name + "]" : "[Function]";
    }
    if (seen.indexOf(value) >= 0) {
      return "[Circular]";
    }

    const nextSeen = seen.concat([value]);
    if (Array.isArray(value)) {
      return value.map(function (item) {
        try {
          return normalizeConsoleArg(item, nextSeen);
        } catch (_error) {
          return "[Unserializable]";
        }
      });
    }

    if (value instanceof Error) {
      return { name: value.name, message: value.message };
    }

    const output = {};
    Object.keys(value).forEach(function (key) {
      try {
        output[key] = normalizeConsoleArg(value[key], nextSeen);
      } catch (_error) {
        output[key] = "[Unserializable]";
      }
    });
    return output;
  }

  function formatConsoleArg(value) {
    if (typeof value === "string") {
      return value;
    }
    if (value && typeof value === "object" && value.type === "undefined") {
      return "undefined";
    }
    if (value && typeof value === "object") {
      try {
        const json = JSON.stringify(value);
        return json === undefined ? String(value) : json;
      } catch (_error) {
        return "[Unserializable]";
      }
    }
    return String(value);
  }

  function captureConsole(level, args) {
    const normalizedArgs = Array.prototype.map.call(args, function (arg) {
      return normalizeConsoleArg(arg, []);
    });
    logs.push({
      level: level,
      message: normalizedArgs.map(formatConsoleArg).join(" "),
      args: normalizedArgs
    });
  }

  Object.defineProperty(globalThis, "__oneflowbase_console_logs", {
    value: logs,
    configurable: false,
    enumerable: false,
    writable: false
  });
  Object.defineProperty(globalThis, "console", {
    value: Object.freeze({
      log: function () { captureConsole("info", arguments); },
      info: function () { captureConsole("info", arguments); },
      warn: function () { captureConsole("warn", arguments); },
      error: function () { captureConsole("error", arguments); }
    }),
    configurable: true,
    enumerable: false,
    writable: true
  });
})();
"#,
    )?;
    Ok(())
}

fn read_console_logs(ctx: &Ctx<'_>) -> Vec<ConsoleLogEntry> {
    let Ok(logs_json) =
        ctx.eval::<String, _>(r#"JSON.stringify(globalThis.__oneflowbase_console_logs || [])"#)
    else {
        return Vec::new();
    };
    serde_json::from_str(&logs_json).unwrap_or_default()
}

fn code_error_for_elapsed(
    timeout: Duration,
    deadline: std::time::Instant,
    interrupted: &AtomicBool,
    fallback: CodeRunnerError,
) -> CodeRunnerError {
    if !timeout.is_zero()
        && (interrupted.load(Ordering::Relaxed) || std::time::Instant::now() >= deadline)
    {
        CodeRunnerError::timeout()
    } else {
        fallback
    }
}

fn build_invocation_script(input_literal: &str) -> String {
    format!(
        r#"
const __oneflowbase_inputs = {input_literal};
const __oneflowbase_output = main(__oneflowbase_inputs);
if (
  __oneflowbase_output === null ||
  Array.isArray(__oneflowbase_output) ||
  typeof __oneflowbase_output !== "object" ||
  Object.prototype.toString.call(__oneflowbase_output) !== "[object Object]"
) {{
  "{INVALID_OUTPUT_SENTINEL}";
}} else {{
  JSON.stringify(__oneflowbase_output);
}}
"#
    )
}

fn reject_module_syntax(source: &str) -> Result<()> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                index = skip_quoted_literal(bytes, index, bytes[index]);
                continue;
            }
            b'`' => {
                index = skip_quoted_literal(bytes, index, b'`');
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index = skip_line_comment(bytes, index + 2);
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index + 2);
                continue;
            }
            _ => {}
        }
        if keyword_at(bytes, index, b"import") {
            let next = skip_ascii_whitespace(bytes, index + b"import".len());
            if next < bytes.len() && bytes[next] == b'(' {
                return Err(CodeRunnerError::dynamic_import_denied().into());
            }
            return Err(CodeRunnerError::module_import_denied().into());
        }
        if keyword_at(bytes, index, b"export") {
            return Err(CodeRunnerError::module_import_denied().into());
        }
        index += 1;
    }

    Ok(())
}

fn keyword_at(source: &[u8], index: usize, keyword: &[u8]) -> bool {
    source
        .get(index..index + keyword.len())
        .is_some_and(|candidate| candidate == keyword)
        && index
            .checked_sub(1)
            .and_then(|previous| source.get(previous))
            .is_none_or(|byte| !is_js_identifier_byte(*byte))
        && source
            .get(index + keyword.len())
            .is_none_or(|byte| !is_js_identifier_byte(*byte))
}

fn skip_ascii_whitespace(source: &[u8], mut index: usize) -> usize {
    while source
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn skip_quoted_literal(source: &[u8], mut index: usize, quote: u8) -> usize {
    index += 1;
    while index < source.len() {
        if source[index] == b'\\' {
            index = index.saturating_add(2);
            continue;
        }
        if source[index] == quote {
            return index + 1;
        }
        index += 1;
    }
    source.len()
}

fn skip_line_comment(source: &[u8], mut index: usize) -> usize {
    while index < source.len() && source[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(source: &[u8], mut index: usize) -> usize {
    while index + 1 < source.len() {
        if source[index] == b'*' && source[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    source.len()
}

fn is_js_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn is_js_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_' || first == b'$')
        && bytes.all(is_js_identifier_byte)
}

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
        .downcast_ref::<CodeRunnerError>()
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
            output.value_type == plugin_framework::provider_contract::PROTOCOL_CONTEXT_VALUE_TYPE
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
