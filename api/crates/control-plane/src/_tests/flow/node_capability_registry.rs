use control_plane::flow::{
    import_application_template_document, preview_application_template_package,
    AgentFlowTemplateApplication, AgentFlowTemplatePackage, AgentFlowTemplateResourceSnapshot,
    AGENT_FLOW_TEMPLATE_SCHEMA_VERSION,
};
use orchestration_runtime::compiler::{FlowCompileContext, FlowCompiler};
use serde_json::{json, Value};
use uuid::Uuid;

fn package(application_type: &str, document: Value) -> AgentFlowTemplatePackage {
    AgentFlowTemplatePackage {
        schema_version: AGENT_FLOW_TEMPLATE_SCHEMA_VERSION.into(),
        application: AgentFlowTemplateApplication {
            application_type: application_type.into(),
            name: "Retention round trip".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        },
        flow_document: document,
        dependencies: Vec::new(),
    }
}

#[test]
fn retention_containers_compile_preview_and_import_without_unknown_nodes() {
    let document: Value =
        serde_json::from_str(include_str!("fixtures/retention-workflow.json")).unwrap();
    let plan = FlowCompiler::compile_workflow(
        Uuid::now_v7(),
        "registry-test",
        &document,
        &FlowCompileContext::default(),
    )
    .expect("the source workflow must be executable before testing portability");
    assert!(plan.compile_issues.is_empty(), "{:?}", plan.compile_issues);
    orchestration_runtime::compiler::ensure_plan_execution_contract(&plan).unwrap();
    let template = package("workflow", document.clone());
    let resources = AgentFlowTemplateResourceSnapshot::default();
    let preview = preview_application_template_package(template.clone(), &resources).unwrap();
    assert!(
        preview.unresolved_nodes.is_empty(),
        "{:?}",
        preview.unresolved_nodes
    );
    assert!(preview.dependencies.iter().all(|d| d.status == "ready"));
    assert_eq!(preview.document, document);
    let flow_id = Uuid::now_v7();
    let (imported, unresolved) =
        import_application_template_document(&template, flow_id, &resources).unwrap();
    assert!(unresolved.is_empty());
    assert_eq!(
        imported["graph"], document["graph"],
        "bindings and container topology must survive import"
    );
    assert_eq!(imported["meta"]["flowId"], flow_id.to_string());
}

#[test]
fn unknown_unavailable_and_missing_plugin_nodes_stay_unresolved() {
    let document = json!({"schemaVersion":domain::FLOW_SCHEMA_VERSION,"meta":{"flowId":"source"},
        "graph":{"nodes":[
            {"id":"unknown","type":"future_node"},
            {"id":"unavailable","type":"knowledge_retrieval"},
            {"id":"missing-plugin","type":"plugin_node","plugin_id":"not-installed",
             "plugin_version":"1.0.0","contribution_code":"example",
             "node_shell":"plugin_node","schema_version":"1"}
        ],"edges":[]}});
    let preview = preview_application_template_package(
        package("workflow", document),
        &AgentFlowTemplateResourceSnapshot::default(),
    )
    .unwrap();
    assert_eq!(preview.unresolved_nodes.len(), 3);
    assert_eq!(
        preview
            .unresolved_nodes
            .iter()
            .find(|n| n.node_id == "missing-plugin")
            .unwrap()
            .reason,
        "missing_plugin"
    );
    assert!(preview.dependencies.iter().all(|d| d.status != "ready"));
    assert!(preview.document["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["type"] == "unresolved_node"));
}

#[test]
fn application_boundary_nodes_are_not_imported_into_another_application_kind() {
    for (application_type, wrong_boundary) in [
        ("workflow", "start"),
        ("agent_flow", "workflow_start"),
        ("agent_flow", "workflow_end"),
    ] {
        let document = json!({"schemaVersion":domain::FLOW_SCHEMA_VERSION,"meta":{"flowId":"source"},
            "graph":{"nodes":[{"id":"wrong","type":wrong_boundary}],"edges":[]}});
        let preview = preview_application_template_package(
            package(application_type, document),
            &AgentFlowTemplateResourceSnapshot::default(),
        )
        .unwrap();
        assert_eq!(
            preview.unresolved_nodes.len(),
            1,
            "{application_type}/{wrong_boundary}"
        );
        assert_eq!(preview.dependencies[0].status, "missing_dependency");
    }
}
