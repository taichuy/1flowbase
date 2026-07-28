use std::collections::BTreeMap;

use orchestration_runtime::binding_runtime::resolve_node_inputs;
use orchestration_runtime::compiled_plan::{CompiledBinding, CompiledNode};
use serde_json::{json, Map, Value};

fn compiled_node(binding: CompiledBinding) -> CompiledNode {
    CompiledNode {
        node_id: "node-data-model".to_string(),
        node_type: "data_model_list".to_string(),
        alias: "Orders".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::from([("query".to_string(), binding)]),
        outputs: Vec::new(),
        config: json!({ "data_model_code": "orders" }),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

fn compiled_code_node(binding: CompiledBinding) -> CompiledNode {
    CompiledNode {
        node_id: "node-code".to_string(),
        node_type: "code".to_string(),
        alias: "Code".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::from([("named_bindings".to_string(), binding)]),
        outputs: Vec::new(),
        config: json!({ "language": "javascript" }),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

#[test]
fn resolve_named_bindings_preserves_selector_and_constant_json_types() {
    let node = compiled_code_node(CompiledBinding {
        i18n_text_ref: None,
        kind: "named_bindings".to_string(),
        raw_value: json!([
            {
                "name": "history",
                "valueType": "array",
                "value": { "kind": "selector", "selector": ["node-start", "history"] }
            },
            {
                "name": "limit",
                "valueType": "number",
                "value": { "kind": "constant", "value": 10 }
            },
            {
                "name": "prompt",
                "valueType": "string",
                "value": {
                    "kind": "templated_text",
                    "value": "User: {{ node-start.query }} / {{ node-start.score }}"
                }
            },
            {
                "name": "score",
                "valueType": "number",
                "value": {
                    "kind": "templated_text",
                    "value": "({{ node-start.score }} + 5) / 2"
                }
            }
        ]),
        selector_paths: vec![],
    });
    let variable_pool = Map::from_iter([(
        "node-start".to_string(),
        json!({ "query": "hello", "score": 20, "history": [{ "role": "user", "content": "hi" }] }),
    )]);

    let resolved = resolve_node_inputs(&node, &variable_pool).unwrap();

    assert_eq!(
        resolved["named_bindings"],
        json!({
            "history": [{ "role": "user", "content": "hi" }],
            "limit": 10,
            "prompt": "User: hello / 20",
            "score": 12.5
        })
    );
}

#[test]
fn reject_named_bindings_numeric_formula_with_non_numeric_selector() {
    let node = compiled_code_node(CompiledBinding {
        i18n_text_ref: None,
        kind: "named_bindings".to_string(),
        raw_value: json!([
            {
                "name": "score",
                "valueType": "number",
                "value": {
                    "kind": "templated_text",
                    "value": "{{ node-start.query }} + 1"
                }
            }
        ]),
        selector_paths: vec![],
    });
    let variable_pool = Map::from_iter([("node-start".to_string(), json!({ "query": "hello" }))]);

    let error = resolve_node_inputs(&node, &variable_pool).unwrap_err();

    assert!(error
        .to_string()
        .contains("numeric expression selector node-start.query is not a number"));
}

#[test]
fn resolve_data_model_query_binding_with_constant_and_selector_values() {
    let node = compiled_node(CompiledBinding {
        i18n_text_ref: None,
        kind: "data_model_query".to_string(),
        raw_value: json!({
            "filters": [
                { "field_code": "status", "operator": "eq", "value": { "kind": "constant", "value": "paid" } },
                { "field_code": "customer_id", "operator": "eq", "value": { "kind": "selector", "selector": ["node-start", "customer_id"] } }
            ],
            "sorts": [{ "field_code": "created_at", "direction": "desc" }],
            "expand_relations": ["customer"],
            "page": { "kind": "constant", "value": 1 },
            "page_size": { "kind": "selector", "selector": ["node-start", "page_size"] }
        }),
        selector_paths: vec![],
    });
    let variable_pool = Map::from_iter([(
        "node-start".to_string(),
        json!({ "customer_id": "customer-1", "page_size": 50 }),
    )]);

    let resolved = resolve_node_inputs(&node, &variable_pool).unwrap();

    assert_eq!(
        resolved["query"],
        json!({
            "filters": [
                { "field_code": "status", "operator": "eq", "value": "paid" },
                { "field_code": "customer_id", "operator": "eq", "value": "customer-1" }
            ],
            "sorts": [{ "field_code": "created_at", "direction": "desc" }],
            "expand_relations": ["customer"],
            "page": 1,
            "page_size": 50
        })
    );
}

#[test]
fn resolve_data_model_query_rejects_invalid_operator() {
    let node = compiled_node(CompiledBinding {
        i18n_text_ref: None,
        kind: "data_model_query".to_string(),
        raw_value: json!({
            "filters": [
                { "field_code": "status", "operator": "contains", "value": { "kind": "constant", "value": "paid" } }
            ],
            "sorts": [],
            "expand_relations": [],
            "page": { "kind": "constant", "value": 1 },
            "page_size": { "kind": "constant", "value": 20 }
        }),
        selector_paths: vec![],
    });

    let error = resolve_node_inputs(&node, &Map::<String, Value>::new()).unwrap_err();

    assert!(error
        .to_string()
        .contains("data_model list filter operator is unsupported"));
}
