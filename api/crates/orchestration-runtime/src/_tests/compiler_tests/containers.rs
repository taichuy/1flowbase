use super::*;

fn node(id: &str, kind: &str, parent: Option<&str>) -> Value {
    json!({"id":id,"type":kind,"alias":id,"containerId":parent,
        "config":{},"bindings":{},"outputs":[]})
}

fn document() -> Value {
    let mut container = node("loop", "loop", None);
    container["config"] = json!({"max_rounds":3,"round_timeout_ms":5000,"loop_variables":[{"key":"total","value_type":"number"}]});
    container["bindings"] = json!({
        "variables":{"kind":"named_bindings","value":[{"name":"total","value":{"kind":"constant","value":0}}]},
        "variable_updates":{"kind":"named_bindings","value":[{"name":"total","value":{"kind":"selector","selector":["body","value"]}}]},
        "output":{"kind":"selector","value":["body","value"]},
        "break_condition":{"kind":"condition_group","value":{"operator":"and","conditions":[{"left":["loop","variables","total"],"comparator":"greater_than","right":{"kind":"constant","value":100}}]}}
    });
    container["outputs"] = json!([
        {"key":"result","title":"Result","valueType":"array"},
        {"key":"variables","title":"Variables","valueType":"object"}
    ]);
    let mut body = node("body", "template_transform", Some("loop"));
    body["bindings"] = json!({"template":{"kind":"templated_text","value":"{{loop.variables.total}} {{start.input}}"}});
    body["outputs"] = json!([{"key":"value","title":"Value","valueType":"number"}]);
    let mut after = node("after", "template_transform", None);
    after["bindings"] = json!({"template":{"kind":"templated_text","value":"{{loop.result}}"}});
    after["outputs"] = json!([{"key":"result","title":"Result","valueType":"string"}]);
    json!({"schemaVersion":"1flowbase.flow/v2","graph":{
        "nodes":[node("start","start",None),container,node("entry","container_start",Some("loop")),body,after],
        "edges":[{"id":"a","source":"start","target":"loop"},{"id":"b","source":"entry","target":"body","containerId":"loop"},{"id":"c","source":"loop","target":"after"}]
    }})
}

fn compile(document: &Value) -> anyhow::Result<orchestration_runtime::compiled_plan::CompiledPlan> {
    FlowCompiler::compile(
        Uuid::nil(),
        "containers",
        document,
        &FlowCompileContext::default(),
    )
}

#[test]
fn container_compiler_accepts_lexical_inputs_and_delayed_body_bindings() {
    let plan = compile(&document()).unwrap();
    assert!(plan.compile_issues.is_empty(), "{:?}", plan.compile_issues);
    ensure_plan_execution_contract(&plan).unwrap();
}

#[test]
fn container_compiler_rejects_missing_or_unbounded_config() {
    for key in ["max_rounds", "round_timeout_ms"] {
        for value in [Value::Null, json!(0), json!(-1), json!(1.5), json!("3")] {
            let mut doc = document();
            doc["graph"]["nodes"][1]["config"][key] = value;
            assert!(compile(&doc).is_err(), "{key}");
        }
    }
    let mut doc = document();
    doc["graph"]["nodes"][1]["config"]["max_rounds"] = json!(u64::MAX);
    // The integer representation is the boundary, not an arbitrary product capacity.
    assert!(compile(&doc).is_ok());
}

#[test]
fn container_compiler_rejects_cross_boundary_edges_and_ownership_cycles() {
    let mut cross = document();
    cross["graph"]["edges"][2]["source"] = json!("body");
    assert!(compile(&cross)
        .unwrap_err()
        .to_string()
        .contains("boundary"));
    let mut cycle = document();
    cycle["graph"]["nodes"][1]["containerId"] = json!("loop");
    assert!(compile(&cycle).unwrap_err().to_string().contains("cycle"));
    let mut ordinary = document();
    ordinary["graph"]["nodes"][3]["containerId"] = json!("start");
    assert!(compile(&ordinary)
        .unwrap_err()
        .to_string()
        .contains("parent"));
}

#[test]
fn container_compiler_requires_one_entry_and_connected_children() {
    let mut missing = document();
    missing["graph"]["nodes"][2]["type"] = json!("template_transform");
    assert!(compile(&missing).is_err());
    let mut duplicate = document();
    duplicate["graph"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(node("entry2", "container_start", Some("loop")));
    assert!(compile(&duplicate).is_err());
    let mut disconnected = document();
    disconnected["graph"]["edges"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    assert!(compile(&disconnected)
        .unwrap_err()
        .to_string()
        .contains("reachable"));
}

#[test]
fn container_compiler_does_not_leak_body_outputs_or_iteration_locals() {
    for selector in [json!(["body", "value"]), json!(["loop", "index"])] {
        let mut doc = document();
        doc["graph"]["nodes"][4]["bindings"] =
            json!({"value":{"kind":"selector","value":selector}});
        let plan = compile(&doc).unwrap();
        assert!(ensure_plan_execution_contract(&plan).is_err());
    }
    let mut doc = document();
    doc["graph"]["nodes"][1]["bindings"]["variables"]["value"][0]["value"] =
        json!({"kind":"selector","selector":["body","value"]});
    let plan = compile(&doc).unwrap();
    assert!(
        ensure_plan_execution_contract(&plan).is_err(),
        "initial values cannot read future body outputs"
    );
}

#[test]
fn container_compiler_rejects_unknown_loop_variables_and_early_body_conditions() {
    let mut doc = document();
    doc["graph"]["nodes"][3]["bindings"] =
        json!({"value":{"kind":"selector","value":["loop","variables","unknown"]}});
    assert!(ensure_plan_execution_contract(&compile(&doc).unwrap()).is_err());
    doc = document();
    doc["graph"]["nodes"][1]["bindings"]["break_condition"]["value"]["conditions"][0]["left"] =
        json!(["body", "value"]);
    assert!(ensure_plan_execution_contract(&compile(&doc).unwrap()).is_err());
}

#[test]
fn container_compiler_accepts_nested_iteration_without_exposing_grandchildren() {
    let mut doc = document();
    let mut nested = node("nested", "iteration", Some("loop"));
    nested["config"] = json!({"round_timeout_ms":1000,"parallelism":2});
    nested["bindings"] = json!({"items":{"kind":"selector","value":["start","items"]},"output":{"kind":"selector","value":["leaf","result"]}});
    nested["outputs"] = json!([{"key":"result","title":"Result","valueType":"array"}]);
    let mut leaf = node("leaf", "template_transform", Some("nested"));
    leaf["bindings"] =
        json!({"template":{"kind":"templated_text","value":"{{nested.item}} {{loop.index}}"}});
    leaf["outputs"] = json!([{"key":"result","title":"Result","valueType":"string"}]);
    doc["graph"]["nodes"].as_array_mut().unwrap().extend([
        nested,
        node("nested-entry", "container_start", Some("nested")),
        leaf,
    ]);
    doc["graph"]["edges"].as_array_mut().unwrap().extend([
        json!({"id":"d","source":"body","target":"nested","containerId":"loop"}),
        json!({"id":"e","source":"nested-entry","target":"leaf","containerId":"nested"}),
    ]);
    doc["graph"]["nodes"][1]["bindings"]["output"]["value"] = json!(["nested", "result"]);
    ensure_plan_execution_contract(&compile(&doc).unwrap()).unwrap();
    doc["graph"]["nodes"][1]["bindings"]["output"]["value"] = json!(["leaf", "result"]);
    assert!(ensure_plan_execution_contract(&compile(&doc).unwrap()).is_err());
}
