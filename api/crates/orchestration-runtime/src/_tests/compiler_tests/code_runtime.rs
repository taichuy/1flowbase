use super::*;

#[test]
fn compile_flow_document_emits_topology_selector_dependencies_and_provider_runtime() {
    let flow_id = Uuid::now_v7();
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &sample_document(flow_id),
        &compile_context(),
    )
    .unwrap();

    assert_eq!(plan.flow_id, flow_id);
    assert_eq!(plan.topological_order, vec!["node-start", "node-llm"]);
    assert_eq!(
        plan.nodes["node-llm"].dependency_node_ids,
        vec!["node-start"]
    );
    assert_eq!(
        plan.nodes["node-llm"].bindings["prompt_messages"].selector_paths,
        vec![vec!["node-start".to_string(), "query".to_string()]]
    );
    assert_eq!(
        plan.nodes["node-llm"].outputs[0].selector,
        vec!["text".to_string()]
    );
    assert_eq!(
        plan.nodes["node-llm"]
            .llm_runtime
            .as_ref()
            .unwrap()
            .provider_code,
        "fixture_provider"
    );
    assert!(plan.nodes["node-llm"]
        .llm_runtime
        .as_ref()
        .unwrap()
        .provider_instance_id
        .is_empty());
    assert!(plan.compile_issues.is_empty());
}

#[test]
fn code_js_dependency_import_enabled_by_context_has_no_issue() {
    let flow_id = Uuid::now_v7();
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &code_js_dependency_document(flow_id, "code", Some(json!(["zod"]))),
        &code_js_dependency_context("zod", "backend_code"),
    )
    .unwrap();

    assert!(
        plan.compile_issues.is_empty(),
        "enabled backend_code import should compile cleanly, got {:?}",
        plan.compile_issues
    );
}

#[test]
fn code_runtime_metadata_compiles_language_entrypoint_source_and_import_snapshot() {
    let flow_id = Uuid::now_v7();
    let mut document = code_js_dependency_document(flow_id, "code", Some(json!(["zod"])));
    document["graph"]["nodes"][1]["config"]["language"] = json!("javascript");
    document["graph"]["nodes"][1]["config"]["source"] =
        json!("export function main(input) { return input; }");
    document["graph"]["nodes"][1]["config"]["entrypoint"] = json!("main");
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &document,
        &code_js_dependency_context("zod", "backend_code"),
    )
    .unwrap();

    let runtime = plan.nodes["node-code"]
        .code_runtime
        .as_ref()
        .expect("code node should compile runtime metadata");

    assert_eq!(runtime.language, "javascript");
    assert_eq!(
        runtime.source.as_deref(),
        Some("export function main(input) { return input; }")
    );
    assert_eq!(
        plan.nodes["node-code"].outputs[0].selector,
        vec!["result".to_string(), "result".to_string()]
    );
    assert_eq!(runtime.source_ref, None);
    assert_eq!(runtime.entrypoint, "main");
    assert_eq!(runtime.imports, vec!["zod".to_string()]);
    assert_eq!(
        runtime.dependencies,
        vec![CompiledCodeDependency {
            alias: "zod".to_string(),
            target: "backend_code".to_string(),
            artifact_path: "artifacts/zod.backend.mjs".to_string(),
            artifact_hash: "sha256:zod".to_string(),
            integrity: "sha256:zod".to_string(),
        }]
    );
}

#[test]
fn code_runtime_metadata_defaults_entrypoint_and_preserves_source_ref() {
    let flow_id = Uuid::now_v7();
    let mut document = code_js_dependency_document(flow_id, "code", None);
    document["graph"]["nodes"][1]["config"]["sourceRef"] = json!("artifact://code/node-code");
    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    let runtime = plan.nodes["node-code"]
        .code_runtime
        .as_ref()
        .expect("code node should compile runtime metadata");

    assert_eq!(runtime.language, "javascript");
    assert_eq!(runtime.source, None);
    assert_eq!(
        runtime.source_ref.as_deref(),
        Some("artifact://code/node-code")
    );
    assert_eq!(runtime.entrypoint, "main");
    assert!(runtime.imports.is_empty());
    assert!(runtime.dependencies.is_empty());
}

#[test]
fn code_isolation_missing_config_uses_default_profile() {
    let flow_id = Uuid::now_v7();
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &code_js_dependency_document(flow_id, "code", None),
        &compile_context(),
    )
    .unwrap();

    let runtime = plan.nodes["node-code"]
        .code_runtime
        .as_ref()
        .expect("code node should compile runtime metadata");

    assert_eq!(
        runtime.isolation_profile,
        CodeIsolationProfile::quickjs_default()
    );
    assert!(plan.compile_issues.is_empty());
}

#[test]
fn code_isolation_valid_override_within_limit_gets_resolved() {
    let flow_id = Uuid::now_v7();
    let mut document = code_js_dependency_document(flow_id, "code", None);
    document["graph"]["nodes"][1]["config"]["isolation"] = json!({
        "mode": "vm_limited",
        "timeout_ms": 250,
        "memory_mb": 16,
        "stack_kb": 512,
        "network": "deny",
        "filesystem": "deny",
        "env": "none",
        "secrets": "none",
        "executor_id": "quickjs-local"
    });
    let plan = FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

    let runtime = plan.nodes["node-code"]
        .code_runtime
        .as_ref()
        .expect("code node should compile runtime metadata");

    assert_eq!(runtime.isolation_profile.timeout_ms, 250);
    assert_eq!(runtime.isolation_profile.memory_mb, 16);
    assert_eq!(runtime.isolation_profile.stack_kb, 512);
    assert_eq!(runtime.isolation_profile.executor_id, "quickjs-local");
    assert!(plan.compile_issues.is_empty());
}

#[test]
fn code_isolation_invalid_profile_reports_stable_issue() {
    let cases = [
        ("mode", json!("process")),
        ("network", json!("allow")),
        ("filesystem", json!("read_only")),
        ("env", json!("inherit")),
        ("secrets", json!("workspace")),
        ("timeout_ms", json!(1001)),
        ("memory_mb", json!(33)),
        ("stack_kb", json!(1025)),
    ];

    for (field, value) in cases {
        let flow_id = Uuid::now_v7();
        let mut document = code_js_dependency_document(flow_id, "code", None);
        document["graph"]["nodes"][1]["config"]["isolation"] = json!({
            "mode": "vm_limited",
            "timeout_ms": 100,
            "memory_mb": 8,
            "stack_kb": 256,
            "network": "deny",
            "filesystem": "deny",
            "env": "none",
            "secrets": "none",
            "executor_id": "quickjs-local"
        });
        document["graph"]["nodes"][1]["config"]["isolation"][field] = value;

        let plan =
            FlowCompiler::compile(flow_id, "draft-1", &document, &compile_context()).unwrap();

        assert!(
            plan.compile_issues.iter().any(|issue| {
                issue.node_id == "node-code"
                    && issue.code == CompileIssueCode::InvalidCodeIsolationProfile
                    && issue.message.contains(field)
            }),
            "expected stable isolation issue for field {field}, got {:?}",
            plan.compile_issues
        );
    }
}

#[test]
fn code_isolation_executor_capability_advertises_quickjs_local_limits() {
    let capability = orchestration_runtime::compiled_plan::CodeExecutorCapability::quickjs_local();

    assert_eq!(capability.executor_id, "quickjs-local");
    assert_eq!(capability.supported_modes, vec!["vm_limited".to_string()]);
    assert_eq!(capability.max_timeout_ms, 1000);
    assert_eq!(capability.max_memory_mb, 32);
    assert_eq!(capability.max_stack_kb, 1024);
}

#[test]
fn code_js_dependency_import_without_context_reports_not_enabled_issue() {
    let flow_id = Uuid::now_v7();
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &code_js_dependency_document(flow_id, "code", Some(json!(["zod"]))),
        &compile_context(),
    )
    .unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-code"
            && issue.code == CompileIssueCode::JsDependencyImportNotEnabled
            && issue.message.contains("node-code")
            && issue.message.contains("zod")
            && issue.message.contains("backend_code")
    }));
}

#[test]
fn code_js_dependency_missing_or_empty_imports_are_compatible() {
    let flow_id = Uuid::now_v7();
    let missing = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &code_js_dependency_document(flow_id, "code", None),
        &compile_context(),
    )
    .unwrap();
    let empty = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &code_js_dependency_document(flow_id, "code", Some(json!([]))),
        &compile_context(),
    )
    .unwrap();

    assert!(missing.compile_issues.is_empty());
    assert!(empty.compile_issues.is_empty());
}

#[test]
fn code_js_dependency_imports_are_ignored_for_non_code_nodes() {
    let flow_id = Uuid::now_v7();
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &code_js_dependency_document(flow_id, "answer", Some(json!(["zod"]))),
        &compile_context(),
    )
    .unwrap();

    assert!(plan.compile_issues.is_empty());
}

#[test]
fn code_js_dependency_invalid_imports_report_stable_issue() {
    let flow_id = Uuid::now_v7();
    let plan = FlowCompiler::compile(
        flow_id,
        "draft-1",
        &code_js_dependency_document(flow_id, "code", Some(json!(["", 42]))),
        &compile_context(),
    )
    .unwrap();

    assert!(plan.compile_issues.iter().any(|issue| {
        issue.node_id == "node-code" && issue.code == CompileIssueCode::InvalidJsDependencyImport
    }));
}
