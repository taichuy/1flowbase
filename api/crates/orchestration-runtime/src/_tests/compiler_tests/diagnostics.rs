use super::*;
use orchestration_runtime::compiler::FlowValidationError;

fn document() -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "graph": {
            "nodes": [
                {"id":"start","type":"workflow_start","alias":"Start","containerId":null,
                 "config":{"input_fields":[{"key":"name","valueType":"string"}]},"bindings":{},"outputs":[]},
                {"id":"aggregate","type":"variable_aggregator","alias":"Aggregate","containerId":null,
                 "config":{},"bindings":{"groups":{"kind":"variable_groups","value":[
                     {"key":"result","valueType":"string","candidates":[["start","name"]]}
                 ]}},"outputs":[{"key":"result","title":"result","valueType":"string"}]},
                {"id":"end","type":"workflow_end","alias":"End","containerId":null,
                 "config":{},"bindings":{"result":{"kind":"selector","value":["aggregate","result"]}},
                 "outputs":[{"key":"result","title":"Result","valueType":"string"}]}
            ],
            "edges":[{"id":"e1","source":"start","target":"aggregate"},
                     {"id":"e2","source":"aggregate","target":"end"}]
        }
    })
}

#[test]
fn node_diagnostics_aggregator_identifies_the_exact_invalid_output_and_constraint() {
    for (field, invalid, expected) in [
        ("title", json!("Result"), json!("result")),
        ("key", json!("wrong"), json!("result")),
        ("valueType", json!("number"), json!("string")),
    ] {
        let mut document = document();
        document["graph"]["nodes"][1]["outputs"][0][field] = invalid;
        let error =
            FlowCompiler::compile_workflow(Uuid::nil(), "draft", &document, &compile_context())
                .unwrap_err();
        let diagnostic = &error
            .downcast_ref::<FlowValidationError>()
            .unwrap()
            .diagnostics[0];
        assert_eq!(diagnostic.node_id.as_deref(), Some("aggregate"));
        assert_eq!(diagnostic.code, "variable_aggregator_output_mismatch");
        assert_eq!(diagnostic.field_path, format!("/outputs/0/{field}"));
        assert_eq!(diagnostic.expected, Some(expected));
    }
}

#[test]
fn node_diagnostics_valid_aggregator_preserves_execution_contract() {
    let plan =
        FlowCompiler::compile_workflow(Uuid::nil(), "draft", &document(), &compile_context())
            .unwrap();
    assert!(plan.compile_issues.is_empty());
    ensure_plan_execution_contract(&plan).unwrap();
}

#[test]
fn node_diagnostics_selector_keeps_the_offending_upstream_node_and_binding_path() {
    let mut document = document();
    document["graph"]["nodes"][1] = json!({
        "id":"transform","type":"template_transform","alias":"Transform","containerId":null,
        "config":{},"bindings":{"a/b~c":{"kind":"selector","value":["missing","value"]}},
        "outputs":[{"key":"result","title":"Result","valueType":"string"}]
    });
    document["graph"]["edges"][0]["target"] = json!("transform");
    document["graph"]["edges"][1]["source"] = json!("transform");
    document["graph"]["nodes"][2]["bindings"]["result"]["value"] = json!(["transform", "result"]);
    let plan = FlowCompiler::compile_workflow(Uuid::nil(), "draft", &document, &compile_context())
        .unwrap();
    let error = ensure_plan_execution_contract(&plan).unwrap_err();
    let diagnostic = &error
        .downcast_ref::<FlowValidationError>()
        .unwrap()
        .diagnostics[0];
    assert_eq!(diagnostic.node_id.as_deref(), Some("transform"));
    assert_eq!(diagnostic.code, "invalid_selector_source");
    assert_eq!(diagnostic.field_path, "/bindings/a~1b~0c");
    assert!(diagnostic.message.contains("missing.value"));
}

#[test]
fn node_diagnostics_malformed_bindings_are_typed_before_execution() {
    let mut document = document();
    document["graph"]["nodes"][2]["bindings"]["result"] = json!({"kind":"selector","value":17});
    let error = FlowCompiler::compile_workflow(Uuid::nil(), "draft", &document, &compile_context())
        .unwrap_err();
    let diagnostic = &error
        .downcast_ref::<FlowValidationError>()
        .unwrap()
        .diagnostics[0];
    assert_eq!(diagnostic.node_id.as_deref(), Some("end"));
    assert_eq!(diagnostic.field_path, "/bindings/result");
    assert_eq!(diagnostic.code, "invalid_node_configuration");
}

#[test]
fn node_diagnostics_invalid_graph_remains_a_document_validation_failure() {
    let error =
        FlowCompiler::compile_workflow(Uuid::nil(), "draft", &json!({}), &compile_context())
            .unwrap_err();
    let diagnostic = &error
        .downcast_ref::<FlowValidationError>()
        .unwrap()
        .diagnostics[0];
    assert_eq!(diagnostic.node_id, None);
    assert_eq!(diagnostic.code, "invalid_flow_document");
}

#[test]
fn node_diagnostics_aggregator_candidate_error_retains_its_node_and_nested_path() {
    let mut document = document();
    document["graph"]["nodes"][1]["bindings"]["groups"]["value"][0]["candidates"][0] =
        json!(["missing", "name"]);
    let error = FlowCompiler::compile_workflow(Uuid::nil(), "draft", &document, &compile_context())
        .unwrap_err();
    let diagnostic = &error
        .downcast_ref::<FlowValidationError>()
        .unwrap()
        .diagnostics[0];
    assert_eq!(diagnostic.node_id.as_deref(), Some("aggregate"));
    assert_eq!(
        diagnostic.field_path,
        "/bindings/groups/value/0/candidates/0"
    );
    assert!(diagnostic.message.contains("missing.name"));
}
