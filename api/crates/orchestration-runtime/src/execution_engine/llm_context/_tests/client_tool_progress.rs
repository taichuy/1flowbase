use super::*;

fn node() -> CompiledNode {
    serde_json::from_value(json!({
        "node_id":"llm","node_type":"llm","alias":"LLM","container_id":null,
        "dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[],
        "config":{
            "visible_internal_llm_tools_enabled":true,
            "visible_internal_llm_tools":[{"type":"visible_internal_llm_tool","name":"image_agent","target_node_id":"vision"}]
        }
    })).unwrap()
}

#[test]
fn client_progress_names_come_from_external_owner_and_exclude_visible_tool_collisions() {
    let node = node();
    let inputs = json!({"tools":[{"name":"Bash"},{"name":"image_agent"}]})
        .as_object()
        .unwrap()
        .clone();
    let names = client_tool_progress_names(
        &node,
        &inputs,
        &Map::new(),
        &Map::new(),
        &ExecutionRuntimeContext::default(),
    );
    assert_eq!(names, vec!["Bash"]);
    // Even inherited external tools in a nested agent do not establish the
    // direct public callback ownership required for an early tool_use id.
    let nested = json!({"visible_internal_llm_tool":{"external_tool_policy":"inherited"}})
        .as_object()
        .unwrap()
        .clone();
    assert!(client_tool_progress_names(
        &node,
        &inputs,
        &Map::new(),
        &nested,
        &ExecutionRuntimeContext::default()
    )
    .is_empty());
    let control = json!({"tools":[{"name":"Bash"}],"compatibility":{"claude_code_control":"compact_summary"}})
        .as_object().unwrap().clone();
    assert!(client_tool_progress_names(
        &node,
        &control,
        &Map::new(),
        &Map::new(),
        &ExecutionRuntimeContext::default()
    )
    .is_empty());
}

#[test]
fn live_output_item_names_use_the_same_canonical_tool_spelling_as_committed_calls() {
    let tools = vec![json!({"type":"function","function":{"name":"Bash"}})];
    for phase in [
        extension_contracts::provider_contract::ProviderOutputItemPhase::Added,
        extension_contracts::provider_contract::ProviderOutputItemPhase::Done,
    ] {
        let mut event = ProviderStreamEvent::OutputItem {
            phase,
            output_index: 0,
            item: json!({"type":"function_call","id":"item","call_id":"call","name":"bash","arguments":""}),
        };
        canonicalize_provider_stream_event_tool_call_name(&mut event, &tools);
        let ProviderStreamEvent::OutputItem { item, .. } = event else {
            unreachable!()
        };
        assert_eq!(item["name"], "Bash");
    }
}
