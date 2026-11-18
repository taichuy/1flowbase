use super::*;
use orchestration_runtime::compiler::ensure_flow_analysis_valid;

fn analyze_end(
    document: &Value,
    context: &FlowCompileContext,
) -> domain::flow::FlowDocumentAnalysis {
    orchestration_runtime::compiler::analyze_flow_document_for_node(
        document,
        context,
        domain::ApplicationType::Workflow,
        "end",
    )
}

fn document(schema: Value, selector: Value) -> Value {
    json!({"schemaVersion":"1flowbase.flow/v2","meta":{"flowId":Uuid::nil(),"name":"Output analysis"},"graph":{
        "nodes":[
            {"id":"start","type":"workflow_start","alias":"Start","config":{"input_fields":[]},"bindings":{},"outputs":[]},
            {"id":"pack","type":"code","alias":"Pack","config":{"source":"function main(){return {body:'ok',count:1}}","language":"javascript","entrypoint":"main"},"bindings":{},
             "outputs":[{"key":"result","title":"Result","valueType":"object","selector":["result"],"jsonSchema":schema}]},
            {"id":"end","type":"workflow_end","alias":"End","config":{},"bindings":{"report":{"kind":"selector","value":selector}},"outputs":[{"key":"report","title":"Report","valueType":"json"}]}
        ],"edges":[{"id":"a","source":"start","target":"pack"},{"id":"b","source":"pack","target":"end"}]}})
}

fn closed_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["body","count"],"properties":{"body":{"type":"string"},"count":{"type":"number"}}})
}

#[test]
fn explicit_whole_result_and_declared_nested_fields_share_editor_and_compiler_contract() {
    for path in [json!(["pack", "result"]), json!(["pack", "result", "body"])] {
        let document = document(closed_schema(), path);
        let analysis = analyze_end(&document, &FlowCompileContext::default());
        assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
        let resolved = analysis
            .node_outputs
            .iter()
            .find(|node| node.node_id == "pack")
            .unwrap();
        assert_eq!(resolved.outputs[0].selector, vec!["result"]);
        let catalog = analysis
            .variable_catalogs
            .iter()
            .find(|catalog| catalog.node_id == "end" && catalog.field_path == "/bindings/report")
            .unwrap();
        assert!(catalog
            .variables
            .iter()
            .any(|variable| variable.selector == vec!["pack", "result", "body"]));
        let compiled = FlowCompiler::compile_workflow(
            Uuid::nil(),
            "draft",
            &document,
            &FlowCompileContext::default(),
        )
        .unwrap();
        assert_eq!(compiled.nodes["pack"].outputs[0].selector, vec!["result"]);
        ensure_plan_execution_contract(&compiled).unwrap();
    }
}

#[test]
fn closed_schema_typos_and_scalar_traversal_are_rejected_before_execution() {
    for selector in [
        json!(["pack", "result", "boddy"]),
        json!(["pack", "result", "count", "x"]),
        json!(["pack", "boddy"]),
    ] {
        let document = document(closed_schema(), selector);
        let analysis = analyze_end(&document, &FlowCompileContext::default());
        assert!(analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "selector_output_not_found"
                && diagnostic.field_path == "/bindings/report"));
        assert!(ensure_flow_analysis_valid(&analysis).is_err());
        let compiled = FlowCompiler::compile_workflow(
            Uuid::nil(),
            "draft",
            &document,
            &FlowCompileContext::default(),
        )
        .unwrap();
        assert!(ensure_plan_execution_contract(&compiled).is_err());
    }
}

#[test]
fn dynamic_json_is_a_nonblocking_warning_not_an_invented_schema_error() {
    let document = document(
        json!({"type":"object"}),
        json!(["pack", "result", "dynamic", "name"]),
    );
    let analysis = analyze_end(&document, &FlowCompileContext::default());
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.level == "warning"
            && diagnostic.code == "dynamic_selector_path"));
    ensure_flow_analysis_valid(&analysis).unwrap();
}

#[test]
fn absent_code_selector_defaults_once_and_explicit_paths_are_never_rewritten() {
    let mut document = document(closed_schema(), json!(["pack", "result", "body"]));
    document["graph"]["nodes"][1]["outputs"] =
        json!([{"key":"body","title":"Body","valueType":"string"}]);
    let analysis = analyze_end(&document, &FlowCompileContext::default());
    assert!(!analysis.has_errors());
    assert_eq!(
        analysis
            .node_outputs
            .iter()
            .find(|node| node.node_id == "pack")
            .unwrap()
            .outputs[0]
            .selector,
        vec!["result", "body"]
    );
    document["graph"]["nodes"][1]["outputs"][0]["selector"] = json!(["result"]);
    assert_eq!(
        analyze_end(&document, &FlowCompileContext::default())
            .node_outputs
            .iter()
            .find(|node| node.node_id == "pack")
            .unwrap()
            .outputs[0]
            .selector,
        vec!["result"]
    );
}

#[test]
fn analysis_reports_unreachable_sources_but_keeps_catalogs_for_unfinished_nodes() {
    let mut document = document(closed_schema(), json!(["pack", "result", "body"]));
    document["graph"]["edges"] = json!([]);
    document["graph"]["nodes"][1]["config"] = json!({});
    let analysis = analyze_end(&document, &FlowCompileContext::default());
    assert!(analysis.has_errors());
    let catalog = analysis
        .variable_catalogs
        .iter()
        .find(|catalog| catalog.node_id == "end" && catalog.field_path.is_empty())
        .unwrap();
    assert!(!catalog
        .variables
        .iter()
        .any(|variable| variable.node_id == "pack"));
    assert!(analysis
        .node_outputs
        .iter()
        .any(|node| node.node_id == "pack"));
}

#[test]
fn local_schema_refs_resolve_without_external_io() {
    let schema = json!({"type":"object","properties":{"nested":{"$ref":"#/$defs/item"}},"additionalProperties":false,
        "$defs":{"item":{"type":"object","additionalProperties":false,"properties":{"name":{"type":"string"}}}}});
    let valid = analyze_end(
        &document(schema.clone(), json!(["pack", "result", "nested", "name"])),
        &FlowCompileContext::default(),
    );
    assert!(!valid.has_errors(), "{:?}", valid.diagnostics);
    let invalid = analyze_end(
        &document(schema, json!(["pack", "result", "nested", "typo"])),
        &FlowCompileContext::default(),
    );
    assert!(invalid.has_errors());
}

#[test]
fn schema_siblings_are_conjoined_with_alternatives_and_references() {
    for schema in [
        json!({"type":"object","properties":{"name":{"type":"string"}},"additionalProperties":false,"anyOf":[{"required":["name"]}]}),
        json!({"type":"object","properties":{"name":{"type":"string"}},"additionalProperties":false,"$ref":"#/$defs/condition","$defs":{"condition":{"required":["name"]}}}),
        json!({"type":["object","array"],"properties":{"name":{"type":"string"}},"additionalProperties":false}),
    ] {
        let valid = analyze_end(
            &document(schema.clone(), json!(["pack", "result", "name"])),
            &FlowCompileContext::default(),
        );
        assert!(!valid.has_errors(), "{:?}", valid.diagnostics);
        for selector in [
            json!(["pack", "result", "typo"]),
            json!(["pack", "result", "name", "x"]),
        ] {
            let invalid = analyze_end(
                &document(schema.clone(), selector),
                &FlowCompileContext::default(),
            );
            assert!(
                invalid
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "selector_output_not_found"),
                "{:?}",
                invalid.diagnostics
            );
        }
    }
}

#[test]
fn composite_schema_properties_and_bound_dynamic_paths_are_available_in_the_catalog() {
    for keyword in ["allOf", "anyOf", "oneOf"] {
        let schema = json!({(keyword):[{"type":"object","properties":{"name":{"type":"string"}}}]});
        let analysis = analyze_end(
            &document(schema, json!(["pack", "result", "name"])),
            &FlowCompileContext::default(),
        );
        assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
        let catalog = analysis
            .variable_catalogs
            .iter()
            .find(|catalog| catalog.node_id == "end" && catalog.field_path == "/bindings/report")
            .unwrap();
        assert!(catalog
            .variables
            .iter()
            .any(|option| option.selector == vec!["pack", "result", "name"]
                && option.value_type == "string"));
    }
    let analysis = analyze_end(
        &document(
            json!({"type":"object"}),
            json!(["pack", "result", "dynamic", "nested"]),
        ),
        &FlowCompileContext::default(),
    );
    let catalog = analysis
        .variable_catalogs
        .iter()
        .find(|catalog| catalog.node_id == "end" && catalog.field_path == "/bindings/report")
        .unwrap();
    assert!(catalog
        .variables
        .iter()
        .any(|option| option.selector == vec!["pack", "result", "dynamic", "nested"]));
}

#[test]
fn an_unknown_schema_alternative_does_not_invent_a_numeric_output_type() {
    let schema = json!({"type":"object","properties":{"mixed":{"anyOf":[{"type":"number"},{}]}}});
    let analysis = analyze_end(
        &document(schema, json!(["pack", "result", "mixed"])),
        &FlowCompileContext::default(),
    );
    assert!(!analysis.has_errors());
    let catalog = analysis
        .variable_catalogs
        .iter()
        .find(|catalog| catalog.node_id == "end" && catalog.field_path == "/bindings/report")
        .unwrap();
    assert_eq!(
        catalog
            .variables
            .iter()
            .find(|option| option.selector == vec!["pack", "result", "mixed"])
            .unwrap()
            .value_type,
        "json"
    );
}

#[test]
fn unfinished_code_remains_analyzable_but_cannot_execute_even_when_legacy_issues_are_cleared() {
    let mut document = document(closed_schema(), json!(["pack", "result"]));
    document["graph"]["nodes"][1]["config"] = json!({});
    let analysis = analyze_end(&document, &FlowCompileContext::default());
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "missing_code_source"
            && diagnostic.field_path == "/config/source"));
    let mut plan = FlowCompiler::compile_workflow(
        Uuid::nil(),
        "draft",
        &document,
        &FlowCompileContext::default(),
    )
    .unwrap();
    plan.compile_issues.clear();
    assert!(ensure_plan_execution_contract(&plan)
        .unwrap_err()
        .downcast_ref::<orchestration_runtime::compiler::FlowValidationError>()
        .is_some());
}

#[test]
fn application_bound_analysis_does_not_trust_a_different_client_node_family() {
    let document = document(closed_schema(), json!(["pack", "result"]));
    let report = orchestration_runtime::compiler::analyze_flow_document_for_application(
        &document,
        &FlowCompileContext::default(),
        domain::ApplicationType::AgentFlow,
    );
    assert!(report.has_errors());
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("agent_flow document cannot contain workflow_start")));
}

#[test]
fn analysis_does_not_materialize_all_pairs_of_upstream_variable_catalogs() {
    let mut document = domain::default_flow_document_for_application(
        domain::ApplicationType::Workflow,
        Uuid::nil(),
    );
    let start = document["graph"]["nodes"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    document["graph"]["nodes"][0]["config"] =
        json!({"input_fields":[{"key":"text","label":"Text","valueType":"string"}]});
    let mut previous = start;
    let mut nodes = vec![document["graph"]["nodes"][0].clone()];
    let mut edges = Vec::new();
    for index in 0..256 {
        let id = format!("transform-{index}");
        nodes.push(json!({"id":id,"type":"template_transform","alias":id,"config":{},"bindings":{"template":{"kind":"templated_text","value":format!("{{{{ {previous}.text }}}}")}},"outputs":[{"key":"text","title":"Text","valueType":"string"}]}));
        edges.push(json!({"id":format!("edge-{index}"),"source":previous,"target":id}));
        previous = id;
    }
    let mut end = document["graph"]["nodes"][1].clone();
    end["bindings"] = json!({"report":{"kind":"selector","value":[previous,"text"]}});
    let end_id = end["id"].as_str().unwrap().to_owned();
    edges.push(json!({"id":"end-edge","source":previous,"target":end_id}));
    nodes.push(end);
    document["graph"]["nodes"] = json!(nodes);
    document["graph"]["edges"] = json!(edges);
    let report = orchestration_runtime::compiler::analyze_flow_document_for_application(
        &document,
        &FlowCompileContext::default(),
        domain::ApplicationType::Workflow,
    );
    assert!(!report.has_errors(), "{:?}", report.diagnostics);
    assert_eq!(report.node_outputs.len(), 258);
    assert!(report.variable_catalogs.is_empty());
    let report = orchestration_runtime::compiler::analyze_flow_document_for_node(
        &document,
        &FlowCompileContext::default(),
        domain::ApplicationType::Workflow,
        &end_id,
    );
    assert!(!report.has_errors());
    assert_eq!(report.variable_catalogs.len(), 2);
    assert!(report
        .variable_catalogs
        .iter()
        .all(|catalog| catalog.node_id == end_id));
}
