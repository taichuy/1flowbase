//! Per-invocation synchronous QuickJS execution, strict plugin JSON and packaged assets.
//! No plans, node payloads, protocol context, host services or live-runtime cache.
mod codec;
mod console;
mod error;
mod packaged;
mod syntax;

pub use error::{ErrorKind, ExecutionError};
pub use packaged::{load_packaged_script, PackagedScript};
use rquickjs::{Context, Function, Runtime, Value as JsValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub timeout: Duration,
    /// Zero preserves the engine default (does not call set_memory_limit).
    pub memory_limit_bytes: usize,
    /// Zero preserves the engine default (does not call set_max_stack_size).
    pub stack_size_bytes: usize,
    /// Combined JSON payload and captured console budget (UTF-8 bytes).
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsoleLogEntry {
    pub level: String,
    pub message: String,
    pub args: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionOutput {
    pub output_payload: Value,
    pub console_logs: Vec<ConsoleLogEntry>,
}

#[derive(Debug, Default)]
struct CancellationState {
    stop: AtomicBool,
    started: AtomicBool,
    exited: AtomicBool,
}

/// One invocation's stop signal and cleanup observations. Never reuse between calls.
/// `has_exited` means the JS Context and Runtime have been dropped, not carrier join.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<CancellationState>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.stop.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.stop.load(Ordering::Acquire)
    }
    /// Set by a host callback inside the invocation wrapper, before calling guest main.
    pub fn has_started(&self) -> bool {
        self.0.started.load(Ordering::Acquire)
    }
    pub fn has_exited(&self) -> bool {
        self.0.exited.load(Ordering::Acquire)
    }
}
struct ExitObservation(CancellationToken);
impl Drop for ExitObservation {
    fn drop(&mut self) {
        self.0 .0.exited.store(true, Ordering::Release);
    }
}

/// Already-read dependency source. Host path policy stays in the orchestration adapter.
/// Packaged plugin scripts do not use dependency facades.
pub struct ScriptDependency {
    pub alias: String,
    pub source: String,
}

pub fn run_script(
    source: &str,
    input: &Value,
    limits: Limits,
) -> Result<ExecutionOutput, ExecutionError> {
    run_script_cancellable(source, input, limits, &[], CancellationToken::default())
}

pub fn run_script_cancellable(
    source: &str,
    input: &Value,
    limits: Limits,
    dependencies: &[ScriptDependency],
    cancel: CancellationToken,
) -> Result<ExecutionOutput, ExecutionError> {
    execute(source, input, limits, dependencies, cancel, false)
}

/// Compatibility entry for existing Code nodes ONLY. Retains JSON.stringify omission,
/// non-finite-to-null, toJSON, and serialization-exception mapping. Never strict fallback.
pub fn run_legacy_script_cancellable(
    source: &str,
    input: &Value,
    limits: Limits,
    dependencies: &[ScriptDependency],
    cancel: CancellationToken,
) -> Result<ExecutionOutput, ExecutionError> {
    execute(source, input, limits, dependencies, cancel, true)
}

/// Existing Code's preflight order, before host-owned dependency artifact I/O.
pub fn validate_legacy_source(source: &str) -> Result<(), ExecutionError> {
    if source.trim().is_empty() {
        return Err(ErrorKind::SourceMissing.into());
    }
    syntax::reject_module_syntax(source, true)
}

fn check_stop(cancel: &CancellationToken, deadline: Instant) -> Result<(), ExecutionError> {
    if cancel.is_cancelled() {
        return Err(ErrorKind::Cancelled.into());
    }
    if Instant::now() >= deadline {
        return Err(ErrorKind::Timeout.into());
    }
    Ok(())
}

fn execute(
    source: &str,
    input: &Value,
    limits: Limits,
    dependencies: &[ScriptDependency],
    cancel: CancellationToken,
    legacy: bool,
) -> Result<ExecutionOutput, ExecutionError> {
    // Declared before Runtime: exit observation cannot precede heap destruction.
    let _exit = ExitObservation(cancel.clone());
    let deadline = Instant::now()
        .checked_add(limits.timeout)
        .ok_or(ErrorKind::InvalidLimits)?;
    if limits.max_output_bytes == 0 {
        return Err(ErrorKind::InvalidLimits.into());
    }
    check_stop(&cancel, deadline)?;
    if source.trim().is_empty() {
        return Err(ErrorKind::SourceMissing.into());
    }
    syntax::reject_module_syntax(source, legacy)?;
    for dependency in dependencies {
        if !syntax::is_js_identifier(&dependency.alias) {
            return Err(ErrorKind::DependencyAliasInvalid.into());
        }
        syntax::reject_module_syntax(&dependency.source, legacy)
            .map_err(|_| ExecutionError::from(ErrorKind::DependencyArtifactInvalid))?;
    }
    check_stop(&cancel, deadline)?;
    let runtime = Runtime::new().map_err(|_| ExecutionError::from(ErrorKind::RuntimeError))?;
    if limits.memory_limit_bytes != 0 {
        runtime.set_memory_limit(limits.memory_limit_bytes);
    }
    if limits.stack_size_bytes != 0 {
        runtime.set_max_stack_size(limits.stack_size_bytes);
    }
    let stop = cancel.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        stop.is_cancelled() || Instant::now() >= deadline
    })));
    let context =
        Context::full(&runtime).map_err(|_| ExecutionError::from(ErrorKind::RuntimeError))?;
    context.with(|ctx| {
        let codec = codec::StrictCodec::capture(&ctx).map_err(|_| ExecutionError::from(ErrorKind::RuntimeError))?;
        let console = console::ConsoleCapture::install(&ctx, limits.max_output_bytes, legacy, codec.clone(), cancel.clone(), deadline)?;
        let result: Result<Value, ExecutionError> = (|| {
            ctx.eval::<(), _>("globalThis.__dependencies = {};").map_err(|_| ErrorKind::RuntimeError)?;
            for dependency in dependencies {
                check_stop(&cancel, deadline)?;
                ctx.eval::<(), _>(dependency.source.as_str()).map_err(|_| ErrorKind::DependencyArtifactInvalid)?;
                let alias = serde_json::to_string(&dependency.alias).map_err(|_| ErrorKind::RuntimeError)?;
                let registered = ctx.eval::<bool, _>(format!("Object.prototype.hasOwnProperty.call(globalThis.__dependencies,{alias})"))
                    .map_err(|_| ErrorKind::DependencyArtifactInvalid)?;
                if !registered { return Err(ErrorKind::DependencyArtifactInvalid.into()); }
                ctx.eval::<(), _>(format!("var {} = globalThis.__dependencies[{alias}];", dependency.alias))
                    .map_err(|_| ErrorKind::DependencyArtifactInvalid)?;
            }
            // Capture the wrapper and host marker before evaluating guest source.
            let invoke: Function = ctx.eval("(function(main, input, started){ started(); return main(input); })")
                .map_err(|_| ErrorKind::RuntimeError)?;
            let marker = cancel.clone();
            let started = Function::new(ctx.clone(), move || marker.0.started.store(true, Ordering::Release))
                .map_err(|_| ErrorKind::RuntimeError)?;
            check_stop(&cancel, deadline)?;
            ctx.eval::<(), _>(source).map_err(|_| ErrorKind::SyntaxError)?;
            let main_type = ctx.eval::<String, _>("typeof main").map_err(|_| ErrorKind::RuntimeError)?;
            if main_type != "function" { return Err(ErrorKind::MainMissing.into()); }
            let main: Function = ctx.eval("main").map_err(|_| ErrorKind::RuntimeError)?;
            // Native parser treats __proto__ as data and cannot be replaced by guest JSON.parse.
            let input_json = serde_json::to_vec(input).map_err(|_| ErrorKind::RuntimeError)?;
            let js_input = if legacy {
                // Keep original Code's object-literal input construction semantics.
                let literal = String::from_utf8(input_json).map_err(|_| ErrorKind::RuntimeError)?;
                ctx.eval::<JsValue, _>(format!("({literal})")).map_err(|_| ErrorKind::RuntimeError)?
            } else { ctx.json_parse(input_json).map_err(|_| ErrorKind::RuntimeError)? };
            let output: JsValue = invoke.call((main, js_input, started)).map_err(|_| ErrorKind::RuntimeError)?;
            let output_payload = if legacy {
                // Keep the original Code root check and guest-observable stringify/error behavior.
                let encode: Function = ctx.eval(r#"(function(output) {
                    if (output === null || Array.isArray(output) || typeof output !== 'object' ||
                        Object.prototype.toString.call(output) !== '[object Object]') return '__1flowbase_invalid_output__';
                    return JSON.stringify(output);
                })"#).map_err(|_| ErrorKind::RuntimeError)?;
                let json: String = encode.call((output,)).map_err(|_| ErrorKind::RuntimeError)?;
                if json == "__1flowbase_invalid_output__" { return Err(ErrorKind::InvalidOutput.into()); }
                if json.len() > limits.max_output_bytes { return Err(ErrorKind::OutputLimit.into()); }
                let value: Value = serde_json::from_str(&json).map_err(|_| ErrorKind::InvalidOutput)?;
                if !value.is_object() { return Err(ErrorKind::InvalidOutput.into()); }
                value
            } else {
                codec.encode_root(output, limits.max_output_bytes, &cancel, deadline)?
            };
            check_stop(&cancel, deadline)?;
            Ok(output_payload)
        })();
        // Interrupt exceptions are sticky. Console capture must not execute guest JS on
        // strict paths; legacy retains the existing best-effort read after failures.
        let (console_logs, console_bytes, overflow) = console.read(&ctx);
        let result = match check_stop(&cancel, deadline) { Err(e) => Err(e), Ok(()) => result };
        let result = result.and_then(|output_payload| {
            let payload_bytes = serde_json::to_vec(&output_payload).map_err(|_| ErrorKind::InvalidOutput)?.len();
            if overflow || payload_bytes.saturating_add(console_bytes) > limits.max_output_bytes {
                return Err(ErrorKind::OutputLimit.into());
            }
            Ok(ExecutionOutput { output_payload, console_logs: console_logs.clone() })
        });
        result.map_err(|mut error| { error.console_logs = console_logs; error })
    })
}

#[cfg(test)]
#[path = "_tests/execution.rs"]
mod execution_tests;
