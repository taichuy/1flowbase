use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use control_plane::ports::{
    FrontstageExecutableCompilerFailure, FrontstageExecutableUpgradeCompiler,
    FrontstageExecutableUpgradeRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

const COMPILER_ROOT: &str = "/app/frontstage-executable-compiler";
const COMPILER_ENTRY_RELATIVE: &str = "packages/tailwindcss-catalog/bin/compiler-4.3.3.mjs";
const COMPILER_NODE_PATH: &str = "/usr/local/bin/node";
const COMPILER_ROOT_ENV: &str = "API_FRONTSTAGE_EXECUTABLE_COMPILER_ROOT";
const COMPILER_NODE_PATH_ENV: &str = "API_FRONTSTAGE_EXECUTABLE_NODE_PATH";
const COMPILER_ENTRY_SHA256: &str =
    "603eb3ed18b81b7de3ce3f0e1f6f599dc1c6d58e246b6f567bad59e2a4d0a704";
const COMPILER_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_COMPILER_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_COMPILER_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_COMPILER_STDERR_BYTES: usize = 64 * 1024;

pub fn target() -> domain::FrontstageExecutableUpgradeTarget {
    domain::FrontstageExecutableUpgradeTarget {
        marker: "frontstage-tailwind-4.3.3-block-preset-v1".into(),
        contract_identity: json!({
            "name": "@1flowbase/tailwindcss-catalog/compiler",
            "version": "4.3.3",
            "contract": "executable-compiler-v1",
            "preset": "default-utilities-standard-variants-v1",
            "preset_asset_sha256": "77c009cb4826b765d416513e3d9c83093482ecb69de9e361e4c25f5441240b36",
            "stylesheet_sha256": "41e1b1cefc721fa2889683134f896f1bafa9907d9057800343b2b7376f7d36a1",
            "tsx_validation": "sucrase@3.35.1/dependency-lock-imports-v2",
        }),
        compiler_identity: json!({
            "name": "@1flowbase/tailwindcss-catalog",
            "contract": "block-preset-v1",
            "tailwind_version": "4.3.3",
        }),
        toolchain_lock: json!({
            "package": "tailwindcss",
            "version": "4.3.3",
            "mode": "block-preset",
        }),
    }
}

pub async fn require_cutover(store: &storage_durable::MainDurableStore) -> Result<()> {
    store
        .require_frontstage_executable_cutover(&target())
        .await
        .context(
            "frontstage executable cutover is incomplete; run frontstage_executable_upgrade before starting this runtime",
        )
}

pub async fn run_upgrade(store: storage_durable::MainDurableStore) -> Result<()> {
    control_plane::frontstage_executable_upgrade::FrontstageExecutableUpgradeService::new(
        store,
        NodeFrontstageExecutableCompiler::from_env(),
    )
    .run(target())
    .await?;
    Ok(())
}

pub struct NodeFrontstageExecutableCompiler {
    process: CompilerProcess,
}

impl NodeFrontstageExecutableCompiler {
    fn from_env() -> Self {
        let root = std::env::var_os(COMPILER_ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(COMPILER_ROOT));
        let program = std::env::var_os(COMPILER_NODE_PATH_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(COMPILER_NODE_PATH));
        Self {
            process: CompilerProcess {
                program,
                entry: root.join(COMPILER_ENTRY_RELATIVE),
                current_dir: root,
                entry_sha256: COMPILER_ENTRY_SHA256.into(),
                timeout: COMPILER_TIMEOUT,
            },
        }
    }
}

#[derive(Serialize)]
struct CompilerRequest<'a> {
    source_code: &'a str,
    dependency_lock: &'a Value,
    compiler_identity: &'a Value,
    toolchain_lock: &'a Value,
}

#[derive(Deserialize)]
struct CompilerResponse {
    ok: bool,
    #[serde(default)]
    error: Option<CompilerResponseError>,
    #[serde(default)]
    generated_css: Option<String>,
    #[serde(default)]
    generated_css_sha256: Option<String>,
    #[serde(default)]
    source_sha256: Option<String>,
    #[serde(default)]
    dependency_lock: Option<Value>,
    #[serde(default)]
    compiler_identity: Option<Value>,
    #[serde(default)]
    toolchain_lock: Option<Value>,
    #[serde(default)]
    artifact_identity: Option<Value>,
    #[serde(default)]
    artifact_sha256: Option<String>,
}

#[derive(Deserialize)]
struct CompilerResponseError {
    code: String,
}

#[async_trait]
impl FrontstageExecutableUpgradeCompiler for NodeFrontstageExecutableCompiler {
    async fn compile_frontstage_executable(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        source: &domain::LegacyFrontstageExecutableSnapshotRow,
    ) -> Result<domain::CompiledFrontstageExecutable, FrontstageExecutableCompilerFailure> {
        let request = serde_json::to_vec(&CompilerRequest {
            source_code: &source.source_code,
            dependency_lock: &source.dependency_lock,
            compiler_identity: &target.compiler_identity,
            toolchain_lock: &target.toolchain_lock,
        })
        .map_err(|_| compiler_failure("request_encode_failed"))?;
        let output = self
            .process
            .run(&request)
            .await
            .map_err(|code| compiler_failure(code))?;
        let response: CompilerResponse =
            serde_json::from_slice(&output).map_err(|_| compiler_failure("malformed_output"))?;
        if !response.ok {
            return Err(compiler_failure(
                response
                    .error
                    .as_ref()
                    .map_or("compiler_rejected", |error| error.code.as_str()),
            ));
        }
        let artifact_digest_matches = response.artifact_sha256.as_deref()
            == Some("2005c459882fcaeb283ff36706b327efebf8783414ecaa111f92f628c4ba0af8");
        if response.artifact_identity.as_ref() != Some(&target.contract_identity)
            || !artifact_digest_matches
        {
            return Err(compiler_failure("artifact_identity_mismatch"));
        }
        Ok(domain::CompiledFrontstageExecutable {
            row_id: source.row_id,
            source_sha256: response
                .source_sha256
                .ok_or_else(|| compiler_failure("malformed_output"))?,
            dependency_lock: response
                .dependency_lock
                .ok_or_else(|| compiler_failure("malformed_output"))?,
            generated_css: response
                .generated_css
                .ok_or_else(|| compiler_failure("malformed_output"))?,
            generated_css_sha256: response
                .generated_css_sha256
                .ok_or_else(|| compiler_failure("malformed_output"))?,
            compiler_identity: response
                .compiler_identity
                .ok_or_else(|| compiler_failure("malformed_output"))?,
            toolchain_lock: response
                .toolchain_lock
                .ok_or_else(|| compiler_failure("malformed_output"))?,
            contract_identity: target.contract_identity.clone(),
        })
    }
}

fn compiler_failure(code: impl Into<String>) -> FrontstageExecutableCompilerFailure {
    FrontstageExecutableCompilerFailure {
        error_code: code.into(),
    }
}

struct CompilerProcess {
    program: std::path::PathBuf,
    entry: std::path::PathBuf,
    current_dir: std::path::PathBuf,
    entry_sha256: String,
    timeout: Duration,
}

impl CompilerProcess {
    async fn run(&self, input: &[u8]) -> std::result::Result<Vec<u8>, &'static str> {
        if input.len() > MAX_COMPILER_INPUT_BYTES {
            return Err("input_too_large");
        }
        let entry = tokio::fs::read(&self.entry)
            .await
            .map_err(|_| "artifact_unavailable")?;
        if format!("{:x}", Sha256::digest(&entry)) != self.entry_sha256 {
            return Err("artifact_digest_mismatch");
        }
        let mut child = Command::new(&self.program);
        child
            .arg(&self.entry)
            .current_dir(&self.current_dir)
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("LC_ALL", "C.UTF-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = child.spawn().map_err(|_| "process_spawn_failed")?;
        let mut stdin = child.stdin.take().ok_or("process_spawn_failed")?;
        let stdout = child.stdout.take().ok_or("process_spawn_failed")?;
        let stderr = child.stderr.take().ok_or("process_spawn_failed")?;
        let write = async move {
            stdin
                .write_all(input)
                .await
                .map_err(|_| "process_io_failed")?;
            stdin.shutdown().await.map_err(|_| "process_io_failed")
        };
        let execution = async {
            let (write_result, stdout, stderr, status) = tokio::join!(
                write,
                read_bounded(stdout, MAX_COMPILER_OUTPUT_BYTES),
                read_bounded(stderr, MAX_COMPILER_STDERR_BYTES),
                child.wait()
            );
            write_result?;
            let stdout = stdout?;
            stderr?;
            let status = status.map_err(|_| "process_io_failed")?;
            if !status.success() {
                return Err("process_nonzero");
            }
            if stdout.iter().filter(|byte| **byte == b'\n').count() != 1 || !stdout.ends_with(b"\n")
            {
                return Err("extra_output");
            }
            Ok(stdout)
        };
        tokio::time::timeout(self.timeout, execution)
            .await
            .map_err(|_| "process_timeout")?
    }
}

async fn read_bounded(
    reader: impl AsyncRead + Unpin,
    limit: usize,
) -> std::result::Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::with_capacity(limit.min(8192));
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "process_io_failed")?;
    if bytes.len() > limit {
        Err("process_output_too_large")
    } else {
        Ok(bytes)
    }
}

pub fn verify_release_artifact() -> Result<()> {
    let compiler = NodeFrontstageExecutableCompiler::from_env();
    let bytes = std::fs::read(&compiler.process.entry).context("read compiler entry")?;
    let digest = format!("{:x}", Sha256::digest(bytes));
    if digest != COMPILER_ENTRY_SHA256 {
        bail!("compiler entry digest mismatch")
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/frontstage_executable_upgrade.rs"]
mod tests;
