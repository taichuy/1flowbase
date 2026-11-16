use super::*;

#[tokio::test]
async fn live_debug_run_code_success_persists_output_and_completes() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Code Node Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: Some(code_flow_document(
                seeded.flow_id,
                "function main(inputs) { return { result: inputs.query + ' from code' }; }",
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload["result"],
        json!({ "result": "hello from code" })
    );
    assert!(completed.flow_run.error_payload.is_none());

    let code_node = completed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-code")
        .expect("code node should be persisted");
    assert_eq!(code_node.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(
        code_node.output_payload["result"],
        json!({ "result": "hello from code" })
    );
    assert!(code_node.error_payload.is_none());
    assert_eq!(code_node.metrics_payload["language"], "javascript");
    assert_eq!(code_node.metrics_payload["entrypoint"], "main");
    assert_eq!(code_node.metrics_payload["error"], false);
    assert_eq!(code_node.metrics_payload["executor_id"], "quickjs-local");
    assert_eq!(code_node.metrics_payload["isolation_mode"], "vm_limited");
    assert_eq!(code_node.metrics_payload["timeout_ms"], 100);
    assert_eq!(code_node.metrics_payload["memory_mb"], 8);
    assert_eq!(code_node.metrics_payload["stack_kb"], 256);
    assert!(code_node.debug_payload.as_object().unwrap().is_empty());
    assert_eq!(completed.node_runs.len(), 2);
}

#[tokio::test]
async fn live_debug_run_code_dependency_zod_artifact_validates_input() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Code Dependency Agent")
        .await;
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
    let artifact_path = write_js_dependency_artifact_for_test("zod", artifact_source);
    let artifact_hash = format!(
        "sha256:{:x}",
        base16ct::HexDisplay(&Sha256::digest(artifact_source.as_bytes()))
    );
    service
        .replace_js_dependency_selection_for_tests(&ReplaceApplicationJsDependencySelectionInput {
            actor_user_id: seeded.actor_user_id,
            workspace_id: Uuid::nil(),
            application_id: seeded.application_id,
            installation_id: Uuid::now_v7(),
            provider_code: "fixture_js_dependency_pack".into(),
            plugin_id: "fixture_js_dependency_pack@3.24.0".into(),
            plugin_version: "3.24.0".into(),
            alias: "zod".into(),
            package: "zod".into(),
            version: "3.24.0".into(),
            target: "backend_code".into(),
            artifact_path,
            artifact_hash: artifact_hash.clone(),
            integrity: artifact_hash,
            permissions: domain::JsDependencyPermissions {
                network: "deny".into(),
                filesystem: "deny".into(),
                env: "deny".into(),
            },
        })
        .await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: Some(code_flow_document_with_imports(
                seeded.flow_id,
                r#"
function main(inputs) {
  const parsed = zod.object({ query: zod.string() }).parse(inputs);
  return { result: parsed.query + " from artifact" };
}
"#,
                vec!["zod"],
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload,
        json!({
            "result": { "result": "hello from artifact" },
            "error": null
        })
    );
    let code_node = completed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-code")
        .expect("code node should be persisted");
    assert_eq!(code_node.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(code_node.metrics_payload["imports"], json!(["zod"]));
    assert_eq!(code_node.metrics_payload["dependency_count"], json!(1));
}

#[tokio::test]
async fn live_debug_run_code_output_is_available_to_downstream_answer() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Code Downstream Agent")
        .await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: Some(code_to_answer_flow_document(
                seeded.flow_id,
                "function main(inputs) { return { result: inputs.query + ' downstream' }; }",
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        "Code said: hello downstream"
    );

    let answer_node = completed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-answer")
        .expect("answer node should be persisted");
    assert_eq!(answer_node.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(
        answer_node.output_payload["answer"],
        "Code said: hello downstream"
    );
}

#[tokio::test]
async fn live_debug_run_code_runtime_error_fails_without_host_stack() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Code Error Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: Some(code_flow_document(
                seeded.flow_id,
                "function main(inputs) { throw new Error('user failure'); }",
            )),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let failed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(failed.flow_run.status, domain::FlowRunStatus::Failed);
    let flow_error_payload = failed
        .flow_run
        .error_payload
        .as_ref()
        .expect("flow error payload should be persisted");
    assert_eq!(
        flow_error_payload["error_code"],
        json!("code_runtime_error")
    );
    assert_eq!(
        flow_error_payload["message"],
        json!("user failure")
    );
    assert!(!flow_error_payload
        .to_string()
        .to_ascii_lowercase()
        .contains("stack"));

    let code_node = failed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-code")
        .expect("code node should be persisted");
    assert_eq!(code_node.status, domain::NodeRunStatus::Failed);
    let node_error_payload = code_node
        .error_payload
        .as_ref()
        .expect("code node error should be persisted");
    assert_eq!(
        node_error_payload["error_code"],
        json!("code_runtime_error")
    );
    assert_eq!(
        node_error_payload["message"],
        json!("user failure")
    );
    assert!(!node_error_payload
        .to_string()
        .to_ascii_lowercase()
        .contains("stack"));
    assert_eq!(code_node.metrics_payload["error"], true);
    assert_eq!(failed.node_runs.len(), 2);
}
