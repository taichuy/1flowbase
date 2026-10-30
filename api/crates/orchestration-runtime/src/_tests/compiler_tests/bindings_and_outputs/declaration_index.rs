use super::*;
use crate::compiler::FlowCompileRunLevelVariable;

#[test]
fn node_output_queries_do_not_materialize_unrelated_declarations() {
    let document = serde_json::json!({
        "variables": { "conversation": [{"name": "unused"}, {"name": "unused"}] }
    });
    let context = FlowCompileContext {
        run_level_variables: vec![FlowCompileRunLevelVariable {
            selector: vec!["sys".into(), "clock".into()],
            value_type: "number".into(),
        }],
        ..FlowCompileContext::default()
    };
    let node = CompiledNode {
        node_id: "source".into(),
        node_type: "code".into(),
        alias: String::new(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: vec![compiled_run_level_output(&["value".into()], "number")],
        config: Value::Null,
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    };
    let nodes = BTreeMap::from([("source".into(), node)]);
    let mut declarations = None;
    let output = variable_aggregator_output_for_selector(
        &mut declarations,
        &document,
        &context,
        &nodes,
        &["source".into(), "value".into()],
    )
    .unwrap();
    assert_eq!(output.value_type, "number");
    assert!(declarations.is_none());
    let missing = variable_aggregator_output_for_selector(
        &mut declarations,
        &document,
        &context,
        &nodes,
        &["missing".into(), "value".into()],
    )
    .unwrap_err();
    assert_eq!(
        missing.to_string(),
        "selector missing.value is not a declared upstream node output"
    );
    assert!(declarations.is_none());

    let output = variable_aggregator_output_for_selector(
        &mut declarations,
        &document,
        &context,
        &nodes,
        &["sys".into(), "clock".into()],
    )
    .unwrap();
    assert_eq!(output.value_type, "number");
    let declarations = declarations.expect("a declaration query initializes the shared index");
    assert_eq!(declarations.run_level.len(), 1);
    assert!(declarations.conversation["unused"].1);
}
