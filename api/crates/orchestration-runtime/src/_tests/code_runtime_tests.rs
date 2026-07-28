use std::{collections::BTreeMap, fs, time::Duration};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use plugin_framework::provider_contract::ProviderInvocationInput;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    code_runtime::{execute_code_node, QuickJsCodeInvoker},
    compiled_plan::{
        CodeIsolationProfile, CompiledBinding, CompiledCodeDependency, CompiledCodeRuntime,
        CompiledLlmRuntime, CompiledNode, CompiledOutput, CompiledPlan, CompiledPluginRuntime,
    },
    execution_engine::{
        start_flow_debug_run, CapabilityInvocationOutput, CapabilityInvoker, CodeInvocationOutput,
        CodeInvoker, ProviderInvocationOutput, ProviderInvoker,
    },
    execution_state::ExecutionStopReason,
};

struct CodeFixtureInvoker {
    output_payload: Value,
    fail_message: Option<String>,
}

struct RealCodeFixtureInvoker {
    code: QuickJsCodeInvoker,
}

#[async_trait]
impl ProviderInvoker for CodeFixtureInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        unreachable!("code runtime tests do not execute llm nodes")
    }
}

#[async_trait]
impl CapabilityInvoker for CodeFixtureInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("code runtime tests do not execute capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for CodeFixtureInvoker {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        assert_eq!(runtime.language, "javascript");
        assert_eq!(runtime.entrypoint, "main");
        assert_eq!(config_payload["entrypoint"], json!("main"));
        assert_eq!(input_payload["query"], json!("hello"));

        if let Some(message) = &self.fail_message {
            return Err(anyhow!(message.clone()));
        }

        Ok(CodeInvocationOutput {
            output_payload: self.output_payload.clone(),
            console_logs: Vec::new(),
        })
    }
}

#[async_trait]
impl ProviderInvoker for RealCodeFixtureInvoker {
    async fn invoke_llm(
        &self,
        _runtime: &CompiledLlmRuntime,
        _input: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        unreachable!("real code runtime tests do not execute llm nodes")
    }
}

#[async_trait]
impl CapabilityInvoker for RealCodeFixtureInvoker {
    async fn invoke_capability_node(
        &self,
        _runtime: &CompiledPluginRuntime,
        _config_payload: Value,
        _input_payload: Value,
    ) -> Result<CapabilityInvocationOutput> {
        unreachable!("real code runtime tests do not execute capability nodes")
    }
}

#[async_trait]
impl CodeInvoker for RealCodeFixtureInvoker {
    async fn invoke_code_node(
        &self,
        runtime: &CompiledCodeRuntime,
        config_payload: Value,
        input_payload: Value,
    ) -> Result<CodeInvocationOutput> {
        self.code
            .invoke_code_node(runtime, config_payload, input_payload)
            .await
    }
}

fn code_runtime_plan() -> CompiledPlan {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "node-start".to_string(),
        CompiledNode {
            node_id: "node-start".to_string(),
            node_type: "start".to_string(),
            alias: "Start".to_string(),
            container_id: None,
            dependency_node_ids: vec![],
            downstream_node_ids: vec!["node-code".to_string()],
            bindings: BTreeMap::new(),
            outputs: vec![CompiledOutput {
                key: "query".to_string(),
                title: "Query".to_string(),
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
        "node-code".to_string(),
        CompiledNode {
            node_id: "node-code".to_string(),
            node_type: "code".to_string(),
            alias: "Code".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-start".to_string()],
            downstream_node_ids: vec!["node-answer".to_string()],
            bindings: BTreeMap::from([(
                "query".to_string(),
                CompiledBinding {
                    kind: "selector".to_string(),
                    selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
                    raw_value: json!(["node-start", "query"]),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "result".to_string(),
                title: "Result".to_string(),
                value_type: "string".to_string(),
                selector: vec!["result".to_string(), "result".to_string()],
                json_schema: None,
            }],
            config: json!({
                "language": "javascript",
                "source": "function main(input) { return { result: input.query }; }",
                "entrypoint": "main"
            }),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: Some(CompiledCodeRuntime {
                language: "javascript".to_string(),
                source: Some(
                    "function main(input) { return { result: input.query }; }".to_string(),
                ),
                source_ref: None,
                entrypoint: "main".to_string(),
                imports: Vec::new(),
                dependencies: Vec::new(),
                isolation_profile: CodeIsolationProfile::quickjs_default(),
            }),
        },
    );
    nodes.insert(
        "node-answer".to_string(),
        CompiledNode {
            node_id: "node-answer".to_string(),
            node_type: "answer".to_string(),
            alias: "Answer".to_string(),
            container_id: None,
            dependency_node_ids: vec!["node-code".to_string()],
            downstream_node_ids: vec![],
            bindings: BTreeMap::from([(
                "answer_template".to_string(),
                CompiledBinding {
                    kind: "templated_text".to_string(),
                    selector_paths: vec![vec![
                        "node-code".to_string(),
                        "result".to_string(),
                        "result".to_string(),
                    ]],
                    raw_value: json!("Code said: {{ node-code.result.result }}"),
                },
            )]),
            outputs: vec![CompiledOutput {
                key: "answer".to_string(),
                title: "Answer".to_string(),
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
            "node-code".to_string(),
            "node-answer".to_string(),
        ],
        edges: Vec::new(),
        nodes,
        compile_issues: Vec::new(),
    }
}

fn quickjs_runtime(source: &str) -> CompiledCodeRuntime {
    CompiledCodeRuntime {
        language: "javascript".to_string(),
        source: Some(source.to_string()),
        source_ref: None,
        entrypoint: "main".to_string(),
        imports: Vec::new(),
        dependencies: Vec::new(),
        isolation_profile: CodeIsolationProfile::quickjs_default(),
    }
}

fn quickjs_runtime_with_dependency(
    source: &str,
    alias: &str,
    artifact_source: &str,
) -> CompiledCodeRuntime {
    let artifact_path = write_temp_dependency_artifact(alias, artifact_source);
    let artifact_hash = format!("sha256:{:x}", Sha256::digest(artifact_source.as_bytes()));

    CompiledCodeRuntime {
        language: "javascript".to_string(),
        source: Some(source.to_string()),
        source_ref: None,
        entrypoint: "main".to_string(),
        imports: vec![alias.to_string()],
        dependencies: vec![CompiledCodeDependency {
            alias: alias.to_string(),
            target: "backend_code".to_string(),
            artifact_path,
            artifact_hash: artifact_hash.clone(),
            integrity: artifact_hash,
        }],
        isolation_profile: CodeIsolationProfile::quickjs_default(),
    }
}

fn code_node_with_runtime(runtime: CompiledCodeRuntime) -> CompiledNode {
    CompiledNode {
        node_id: "node-code".to_string(),
        node_type: "code".to_string(),
        alias: "Code".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: vec![CompiledOutput {
            key: "result".to_string(),
            title: "Result".to_string(),
            value_type: "json".to_string(),
            selector: vec!["result".to_string(), "result".to_string()],
            json_schema: None,
        }],
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: Some(runtime),
    }
}

fn write_temp_dependency_artifact(alias: &str, artifact_source: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "1flowbase-code-dependency-{alias}-{}.mjs",
        Uuid::now_v7()
    ));
    fs::write(&path, artifact_source).expect("test dependency artifact should be written");
    path.to_string_lossy().into_owned()
}

async fn invoke_quickjs(source: &str, input_payload: Value) -> Result<CodeInvocationOutput> {
    QuickJsCodeInvoker::default()
        .invoke_code_node(&quickjs_runtime(source), json!({}), input_payload)
        .await
}

async fn invoke_quickjs_with_dependency(
    source: &str,
    alias: &str,
    artifact_source: &str,
    input_payload: Value,
) -> Result<CodeInvocationOutput> {
    QuickJsCodeInvoker::default()
        .invoke_code_node(
            &quickjs_runtime_with_dependency(source, alias, artifact_source),
            json!({}),
            input_payload,
        )
        .await
}

#[tokio::test]
async fn code_runner_quickjs_success_transform_returns_json_object() {
    let output = invoke_quickjs(
        "function main(inputs) { return { result: inputs.query + ' world', count: inputs.count + 1 }; }",
        json!({ "query": "hello", "count": 1 }),
    )
    .await
    .unwrap();

    assert_eq!(
        output.output_payload,
        json!({ "result": "hello world", "count": 2 })
    );
}

#[tokio::test]
async fn code_dependency_quickjs_loads_zod_like_artifact_by_alias() {
    let artifact_source = r#"
globalThis.__dependencies = globalThis.__dependencies || {};
globalThis.__dependencies.zod = {
  object: function(shape) {
    return {
      parse: function(input) {
        const output = {};
        for (const key in shape) {
          output[key] = shape[key].parse(input[key]);
        }
        return output;
      }
    };
  },
  string: function() {
    return {
      parse: function(value) {
        if (typeof value !== "string") {
          throw new Error("expected string");
        }
        return value;
      }
    };
  }
};
"#;
    let output = invoke_quickjs_with_dependency(
        r#"
function main(inputs) {
  const parsed = zod.object({ query: zod.string() }).parse(inputs);
  return { result: parsed.query + " parsed" };
}
"#,
        "zod",
        artifact_source,
        json!({ "query": "hello" }),
    )
    .await
    .unwrap();

    assert_eq!(output.output_payload, json!({ "result": "hello parsed" }));
}

#[tokio::test]
async fn code_dependency_quickjs_missing_artifact_is_stable() {
    let runtime = CompiledCodeRuntime {
        language: "javascript".to_string(),
        source: Some("function main(inputs) { return { result: zod }; }".to_string()),
        source_ref: None,
        entrypoint: "main".to_string(),
        imports: vec!["zod".to_string()],
        dependencies: vec![CompiledCodeDependency {
            alias: "zod".to_string(),
            target: "backend_code".to_string(),
            artifact_path: "/tmp/1flowbase-missing-zod-artifact.mjs".to_string(),
            artifact_hash: "sha256:missing".to_string(),
            integrity: "sha256:missing".to_string(),
        }],
        isolation_profile: CodeIsolationProfile::quickjs_default(),
    };

    let error = QuickJsCodeInvoker::default()
        .invoke_code_node(&runtime, json!({}), json!({}))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "dependency_artifact_missing: dependency artifact is missing"
    );
}

#[tokio::test]
async fn code_dependency_quickjs_hash_mismatch_is_stable() {
    let mut runtime = quickjs_runtime_with_dependency(
        "function main(inputs) { return { result: zod.value }; }",
        "zod",
        "globalThis.__dependencies = { zod: { value: 'ok' } };",
    );
    runtime.dependencies[0].artifact_hash = "sha256:mismatch".to_string();
    runtime.dependencies[0].integrity = "sha256:mismatch".to_string();

    let error = QuickJsCodeInvoker::default()
        .invoke_code_node(&runtime, json!({}), json!({}))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "dependency_integrity_mismatch: dependency artifact integrity mismatch"
    );
}

#[tokio::test]
async fn code_runner_quickjs_syntax_error_is_stable() {
    let error = invoke_quickjs("function main(inputs) { return {", json!({}))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "syntax_error: code source is not valid JavaScript"
    );
}

#[tokio::test]
async fn code_runner_quickjs_runtime_throw_is_stable() {
    let error = invoke_quickjs(
        "function main(inputs) { throw new Error('secret host stack should not leak'); }",
        json!({}),
    )
    .await
    .unwrap_err();

    assert_eq!(error.to_string(), "runtime_error: code execution failed");
}

#[tokio::test]
async fn code_runner_quickjs_missing_main_is_stable() {
    let error = invoke_quickjs("function helper(inputs) { return {}; }", json!({}))
        .await
        .unwrap_err();

    assert_eq!(error.to_string(), "main_missing: main function is required");
}

#[tokio::test]
async fn code_runner_quickjs_non_object_output_is_stable() {
    let error = invoke_quickjs("function main(inputs) { return 'not-object'; }", json!({}))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "invalid_output: main must return a JSON object"
    );
}

#[tokio::test]
async fn code_runner_quickjs_promise_output_is_not_json_object() {
    let error = invoke_quickjs(
        "function main(inputs) { return Promise.resolve({ result: 'async' }); }",
        json!({}),
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "invalid_output: main must return a JSON object"
    );
}

#[tokio::test]
async fn code_runner_quickjs_static_import_is_denied() {
    let error = invoke_quickjs(
        "import value from 'pkg'; function main(inputs) { return { value }; }",
        json!({}),
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "module_import_denied: import and export syntax is not supported"
    );
}

#[tokio::test]
async fn code_runner_quickjs_allows_import_text_inside_strings_and_comments() {
    let output = invoke_quickjs(
        r#"
// import text in a comment is not a module dependency
function main(inputs) {
  const label = "import text";
  return { result: label + " " + inputs.query };
}
"#,
        json!({ "query": "ok" }),
    )
    .await
    .unwrap();

    assert_eq!(output.output_payload, json!({ "result": "import text ok" }));
}

#[tokio::test]
async fn code_runner_quickjs_dynamic_import_is_denied() {
    let error = invoke_quickjs("function main(inputs) { return import('pkg'); }", json!({}))
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "module_import_denied: dynamic import is not supported"
    );
}

#[tokio::test]
async fn code_runner_quickjs_timeout_is_stable() {
    let error = QuickJsCodeInvoker::default()
        .with_timeout(Duration::from_millis(50))
        .invoke_code_node(
            &quickjs_runtime("function main(inputs) { while (true) {} }"),
            json!({}),
            json!({}),
        )
        .await
        .unwrap_err();

    assert_eq!(error.to_string(), "timeout: code execution timed out");
}

#[tokio::test]
async fn code_isolation_profile_timeout_is_enforced_by_quickjs_runner() {
    let mut runtime = quickjs_runtime("function main(inputs) { while (true) {} }");
    runtime.isolation_profile.timeout_ms = 50;

    let error = QuickJsCodeInvoker::default()
        .invoke_code_node(&runtime, json!({}), json!({}))
        .await
        .expect_err("profile timeout should stop user code");

    assert_eq!(error.to_string(), "timeout: code execution timed out");
}

#[tokio::test]
async fn code_isolation_profile_executor_id_is_enforced_by_quickjs_runner() {
    let mut runtime = quickjs_runtime("function main() { return { result: 'ok' }; }");
    runtime.isolation_profile.executor_id = "container-js".to_string();

    let error = QuickJsCodeInvoker::default()
        .invoke_code_node(&runtime, json!({}), json!({}))
        .await
        .expect_err("unsupported executor should not fall back to quickjs");

    assert_eq!(
        error.to_string(),
        "executor_not_found: code executor `container-js` is not available"
    );
}

#[tokio::test]
async fn code_isolation_profile_is_included_in_code_metrics() {
    let mut runtime = quickjs_runtime("function main() { return { result: 'ok' }; }");
    runtime.isolation_profile.timeout_ms = 250;
    runtime.isolation_profile.memory_mb = 16;
    runtime.isolation_profile.stack_kb = 512;
    let node = code_node_with_runtime(runtime);
    let plan = code_runtime_plan();

    let execution = execute_code_node(&plan, &node, &Map::new(), &QuickJsCodeInvoker::default())
        .await
        .expect("code node should execute");

    assert_eq!(execution.metrics_payload["executor_id"], "quickjs-local");
    assert_eq!(execution.metrics_payload["isolation_mode"], "vm_limited");
    assert_eq!(execution.metrics_payload["timeout_ms"], 250);
    assert_eq!(execution.metrics_payload["memory_mb"], 16);
    assert_eq!(execution.metrics_payload["stack_kb"], 512);
}

#[tokio::test]
async fn code_runtime_quickjs_console_logs_are_debug_payload_only() {
    let runtime = quickjs_runtime(
        r#"
function main(inputs) {
  console.log("hello", inputs.query, { nested: true, count: inputs.count });
  console.info("plain info");
  console.warn("heads", ["a", 2]);
  console.error("bad", false);
  return { result: { value: inputs.query } };
}
"#,
    );
    let node = code_node_with_runtime(runtime);
    let plan = code_runtime_plan();
    let resolved_inputs = json!({ "query": "hello", "count": 1 })
        .as_object()
        .cloned()
        .unwrap();

    let execution = execute_code_node(
        &plan,
        &node,
        &resolved_inputs,
        &QuickJsCodeInvoker::default(),
    )
    .await
    .expect("code node should execute");

    assert_eq!(
        execution.output_payload,
        json!({ "result": { "result": { "value": "hello" } }, "error": null })
    );
    assert!(execution.output_payload.get("console_logs").is_none());
    assert_eq!(
        execution.debug_payload["console_logs"],
        json!([
            {
                "level": "info",
                "message": "hello hello {\"nested\":true,\"count\":1}",
                "args": ["hello", "hello", { "nested": true, "count": 1 }]
            },
            {
                "level": "info",
                "message": "plain info",
                "args": ["plain info"]
            },
            {
                "level": "warn",
                "message": "heads [\"a\",2]",
                "args": ["heads", ["a", 2]]
            },
            {
                "level": "error",
                "message": "bad false",
                "args": ["bad", false]
            }
        ])
    );
}

#[tokio::test]
async fn code_runtime_quickjs_console_logs_survive_sanitized_runtime_error() {
    let runtime = quickjs_runtime(
        r#"
function main() {
  console.error("before throw", { detail: "visible debug fact" });
  throw new Error("secret host stack should not leak");
}
"#,
    );
    let node = code_node_with_runtime(runtime);
    let plan = code_runtime_plan();

    let execution = execute_code_node(&plan, &node, &Map::new(), &QuickJsCodeInvoker::default())
        .await
        .expect("code node errors should be converted to execution payloads");

    let error_payload = execution
        .error_payload
        .expect("runtime throw should produce an error payload");
    assert_eq!(error_payload["error_code"], json!("code_runtime_error"));
    assert!(error_payload.get("error_kind").is_none());
    assert_eq!(error_payload["message"], json!("code execution failed"));
    assert_eq!(
        error_payload["runtime_message"],
        json!("runtime_error: code execution failed")
    );
    assert!(!error_payload["runtime_message"]
        .as_str()
        .unwrap()
        .contains("secret host stack"));
    assert_eq!(
        execution.debug_payload["console_logs"],
        json!([
            {
                "level": "error",
                "message": "before throw {\"detail\":\"visible debug fact\"}",
                "args": ["before throw", { "detail": "visible debug fact" }]
            }
        ])
    );
}

#[tokio::test]
async fn code_runtime_invoker_success_projects_output_for_downstream_template_node() {
    let outcome = start_flow_debug_run(
        &code_runtime_plan(),
        &json!({ "node-start": { "query": "hello" } }),
        &CodeFixtureInvoker {
            output_payload: json!({ "result": "from-code" }),
            fail_message: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        outcome.variable_pool["node-code"],
        json!({ "result": { "result": "from-code" }, "error": null })
    );
    assert_eq!(
        outcome.variable_pool["node-answer"],
        json!({ "answer": "Code said: from-code" })
    );
    assert_eq!(
        outcome.node_traces[1].output_payload,
        json!({ "result": { "result": "from-code" }, "error": null })
    );
    assert_eq!(
        outcome.node_traces[1].metrics_payload["language"],
        json!("javascript")
    );
    assert!(outcome.node_traces[1].error_payload.is_none());
}

#[tokio::test]
async fn code_runtime_resolves_templated_named_bindings_as_top_level_args() {
    let mut plan = code_runtime_plan();
    let code_node = plan
        .nodes
        .get_mut("node-code")
        .expect("expected code node in fixture plan");
    code_node.bindings = BTreeMap::from([(
        "named_bindings".to_string(),
        CompiledBinding {
            kind: "named_bindings".to_string(),
            selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
            raw_value: json!([
                {
                    "name": "arg1",
                    "content": {
                        "kind": "templated_text",
                        "value": "Question: {{ node-start.query }}"
                    }
                }
            ]),
        },
    )]);
    code_node.config["source"] = json!("function main({arg1}) { return { result: arg1 }; }");
    code_node.code_runtime.as_mut().unwrap().source =
        Some("function main({arg1}) { return { result: arg1 }; }".to_string());

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &RealCodeFixtureInvoker {
            code: QuickJsCodeInvoker::default(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        outcome.variable_pool["node-code"],
        json!({ "result": { "result": "Question: hello" }, "error": null })
    );
}

#[tokio::test]
async fn code_runtime_resolves_constant_named_bindings_with_json_types() {
    let mut plan = code_runtime_plan();
    let code_node = plan
        .nodes
        .get_mut("node-code")
        .expect("expected code node in fixture plan");
    code_node.bindings = BTreeMap::from([(
        "named_bindings".to_string(),
        CompiledBinding {
            kind: "named_bindings".to_string(),
            selector_paths: Vec::new(),
            raw_value: json!([
                {
                    "name": "limit",
                    "valueType": "number",
                    "value": { "kind": "constant", "value": 10 }
                },
                {
                    "name": "title",
                    "valueType": "string",
                    "value": { "kind": "constant", "value": "10" }
                },
                {
                    "name": "options",
                    "valueType": "object",
                    "value": { "kind": "constant", "value": { "top_k": 5 } }
                },
                {
                    "name": "items",
                    "valueType": "array",
                    "value": { "kind": "constant", "value": [1, 2] }
                }
            ]),
        },
    )]);
    code_node.config["source"] = json!(
        "function main({limit, title, options, items}) { return { result: { limit_type: typeof limit, title_type: typeof title, top_k: options.top_k, item_count: items.length } }; }"
    );
    code_node.code_runtime.as_mut().unwrap().source = Some(
        "function main({limit, title, options, items}) { return { result: { limit_type: typeof limit, title_type: typeof title, top_k: options.top_k, item_count: items.length } }; }"
            .to_string(),
    );
    code_node.outputs = vec![CompiledOutput {
        key: "result".to_string(),
        title: "Result".to_string(),
        value_type: "json".to_string(),
        selector: vec!["result".to_string(), "result".to_string()],
        json_schema: None,
    }];

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &RealCodeFixtureInvoker {
            code: QuickJsCodeInvoker::default(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        outcome.variable_pool["node-code"],
        json!({
            "result": {
                "result": {
                    "limit_type": "number",
                    "title_type": "string",
                    "top_k": 5,
                    "item_count": 2
                }
            },
            "error": null
        })
    );
}

#[tokio::test]
async fn code_runtime_invoker_error_yields_stable_failed_stop_reason_and_trace_payload() {
    let outcome = start_flow_debug_run(
        &code_runtime_plan(),
        &json!({ "node-start": { "query": "hello" } }),
        &CodeFixtureInvoker {
            output_payload: json!({}),
            fail_message: Some("runtime failed: user code threw".to_string()),
        },
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.node_id, "node-code");
            assert_eq!(failure.node_alias, "Code");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("code_runtime_error")
            );
            assert!(failure.error_payload.get("error_kind").is_none());
            assert_eq!(
                failure.error_payload["message"],
                json!("code execution failed")
            );
            assert_eq!(
                failure.error_payload["runtime_message"],
                json!("runtime failed: user code threw")
            );
            assert_eq!(outcome.node_traces[1].node_type, "code");
            assert_eq!(
                outcome.node_traces[1].output_payload,
                json!({
                    "result": null,
                    "error": {
                        "error_code": "code_runtime_error",
                        "message": "code execution failed",
                        "runtime_message": "runtime failed: user code threw"
                    }
                })
            );
            assert_eq!(
                outcome.node_traces[1].error_payload.as_ref().unwrap()["error_code"],
                json!("code_runtime_error")
            );
            assert!(outcome.node_traces[1]
                .error_payload
                .as_ref()
                .unwrap()
                .get("error_kind")
                .is_none());
        }
        other => panic!("expected failed stop reason, got {other:?}"),
    }

    assert!(outcome.variable_pool.get("node-code").is_none());
    assert_eq!(outcome.node_traces.len(), 2);
}

#[tokio::test]
async fn code_runtime_missing_declared_output_projects_empty_variable_payload() {
    let outcome = start_flow_debug_run(
        &code_runtime_plan(),
        &json!({ "node-start": { "query": "hello" } }),
        &CodeFixtureInvoker {
            output_payload: json!({ "unexpected": true }),
            fail_message: None,
        },
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.node_id, "node-answer");
            assert!(failure.error_payload["message"]
                .as_str()
                .unwrap()
                .contains("selector path not found: node-code.result"));
        }
        other => panic!("expected downstream binding failure, got {other:?}"),
    }
    assert_eq!(
        outcome.node_traces[1].output_payload,
        json!({ "result": { "unexpected": true }, "error": null })
    );
}

#[tokio::test]
async fn code_runtime_rejects_declared_output_schema_mismatch() {
    let mut plan = code_runtime_plan();
    let code = plan
        .nodes
        .get_mut("node-code")
        .expect("code node should exist");
    code.outputs = vec![CompiledOutput {
        key: "chat_history".to_string(),
        title: "Chat History".to_string(),
        value_type: "array".to_string(),
        selector: vec!["result".to_string(), "chat_history".to_string()],
        json_schema: Some(json!({
            "type": "array",
            "items": {
                "type": "object",
                "required": ["role", "content"],
                "properties": {
                    "role": { "type": "string" },
                    "content": { "type": "string" }
                }
            }
        })),
    }];

    let outcome = start_flow_debug_run(
        &plan,
        &json!({ "node-start": { "query": "hello" } }),
        &CodeFixtureInvoker {
            output_payload: json!({ "chat_history": [{ "role": "user" }] }),
            fail_message: None,
        },
    )
    .await
    .unwrap();

    match outcome.stop_reason {
        ExecutionStopReason::Failed(failure) => {
            assert_eq!(failure.node_id, "node-code");
            assert_eq!(
                failure.error_payload["error_code"],
                json!("code_output_contract_error")
            );
            assert!(failure.error_payload.get("error_kind").is_none());
            assert_eq!(outcome.node_traces[1].output_payload["result"], Value::Null);
            assert_eq!(
                outcome.node_traces[1].output_payload["error"]["error_code"],
                json!("code_output_contract_error")
            );
        }
        other => panic!("expected code output contract failure, got {other:?}"),
    }
}

#[tokio::test]
async fn code_runtime_execution_engine_uses_real_quickjs_runner_for_downstream_template_node() {
    let outcome = start_flow_debug_run(
        &code_runtime_plan(),
        &json!({ "node-start": { "query": "hello" } }),
        &RealCodeFixtureInvoker {
            code: QuickJsCodeInvoker::default(),
        },
    )
    .await
    .unwrap();

    assert_eq!(outcome.stop_reason, ExecutionStopReason::Completed);
    assert_eq!(
        outcome.variable_pool["node-code"],
        json!({ "result": { "result": "hello" }, "error": null })
    );
    assert_eq!(
        outcome.variable_pool["node-answer"],
        json!({ "answer": "Code said: hello" })
    );
}
