use super::*;

fn fixture() -> (
    CompiledPlan,
    CompiledNode,
    ExecutionRuntimeContext,
    Map<String, Value>,
) {
    let plan = serde_json::from_value(json!({"flow_id":uuid::Uuid::nil(),"source_draft_id":"test","schema_version":"1","topological_order":[],"nodes":{}})).unwrap();
    let node = serde_json::from_value(json!({"node_id":"llm","node_type":"llm","alias":"LLM","container_id":null,"dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[],"config":{}})).unwrap();
    let mut runtime = ExecutionRuntimeContext::default();
    runtime.native_model_prompt_context.system = blocks(&["root-1", "root-2", "root-3"]);
    runtime.native_model_prompt_context.messages = history(&["inline-1", "inline-2", "inline-3"]);
    let templates = json!({"prompt_messages":[
        {"role":"system","content":"application"},
        {"role":"user","content":"old rendered query"}
    ]})
    .as_object()
    .unwrap()
    .clone();
    (plan, node, runtime, templates)
}

fn blocks(texts: &[&str]) -> Vec<NativePromptBlock> {
    texts
        .iter()
        .map(|text| NativePromptBlock::text(*text))
        .collect()
}

fn history(instructions: &[&str]) -> Vec<Value> {
    let mut history = vec![json!({"role":"user","content":"admitted user"})];
    history.extend(
        instructions
            .iter()
            .map(|text| json!({"role":"system","content":text})),
    );
    history.push(json!({"role":"assistant","content":"","tool_calls":[{"id":"call-read","name":"Read","arguments":{}}]}));
    history.push(json!({"role":"tool","tool_call_id":"call-read","content":"result"}));
    history
}

fn assembled(
    plan: &CompiledPlan,
    node: &CompiledNode,
    runtime: &ExecutionRuntimeContext,
    templates: &Map<String, Value>,
    admitted: Option<Vec<Value>>,
    policy: Value,
) -> Vec<Value> {
    let pool = admitted
        .map(|history| {
            json!({"llm":{"__llm_tool_callback":{
                "verified_anthropic_context":true,"history":history,
                "system":[{"type":"text","text":"stale callback system"}]
            }}})
            .as_object()
            .unwrap()
            .clone()
        })
        .unwrap_or_default();
    binding_prompt_messages_with_context_sources(
        plan,
        node,
        templates,
        &Map::new(),
        &pool,
        &policy,
        runtime,
    )
    .unwrap()
}

#[test]
fn callback_system_order_matches_initial_root_inline_application_sequence() {
    let (plan, node, runtime, templates) = fixture();
    let initial = provider_context_from_prompt_messages(assembled(
        &plan,
        &node,
        &runtime,
        &templates,
        None,
        json!({}),
    ))
    .unwrap();
    let callback = provider_context_from_prompt_messages(assembled(
        &plan,
        &node,
        &runtime,
        &templates,
        Some(runtime.native_model_prompt_context.messages.clone()),
        json!({}),
    ))
    .unwrap();
    let expected = blocks(&[
        "root-1",
        "root-2",
        "root-3",
        "inline-1",
        "inline-2",
        "inline-3",
        "application",
    ]);
    assert_eq!(initial.system, expected);
    assert_eq!(callback.system, expected);
    assert_eq!(callback.messages.len(), 3);
    assert!(callback
        .messages
        .iter()
        .all(|message| message.content != "old rendered query"));
    assert_eq!(
        callback.messages[2].tool_call_id.as_deref(),
        Some("call-read")
    );
}

#[test]
fn callback_system_order_replaces_updates_deletions_and_empty_admitted_history() {
    let (plan, node, mut runtime, templates) = fixture();
    // The selector and runtime both carry old history. Neither can supplement a
    // complete replacement, including an intentionally empty replacement.
    let mut pool =
        json!({"selected":{"history":[{"role":"system","content":"old selected inline"}]}})
            .as_object()
            .unwrap()
            .clone();
    for inline in [vec!["updated-inline"], vec![]] {
        let admitted = history(&inline);
        pool.insert(
            "llm".into(),
            json!({"__llm_tool_callback":{"verified_anthropic_context":true,"history":admitted}}),
        );
        let messages = binding_prompt_messages_with_context_sources(
            &plan,
            &node,
            &templates,
            &Map::new(),
            &pool,
            &json!({"context_selector":["selected","history"]}),
            &runtime,
        )
        .unwrap();
        let context = provider_context_from_prompt_messages(messages).unwrap();
        let mut expected = blocks(&["root-1", "root-2", "root-3"]);
        expected.extend(blocks(&inline));
        expected.extend(blocks(&["application"]));
        assert_eq!(context.system, expected);
        assert_eq!(context.messages.len(), 3);
        runtime.native_model_prompt_context.messages = history(&inline);
    }
    let empty = assembled(
        &plan,
        &node,
        &runtime,
        &templates,
        Some(Vec::new()),
        json!({}),
    );
    assert!(empty.iter().all(|message| message["role"] == "system"));
    assert_eq!(
        empty.len(),
        2,
        "only run-level and application instruction owners remain"
    );
}

#[test]
fn callback_system_order_preserves_identical_blocks_from_distinct_sources() {
    let (plan, node, mut runtime, mut templates) = fixture();
    runtime.native_model_prompt_context.system = blocks(&["same", "same"]);
    runtime.native_model_prompt_context.messages = history(&["same"]);
    templates.insert(
        "prompt_messages".into(),
        json!([
            {"role":"system","content":"same"},{"role":"user","content":"old query"}
        ]),
    );
    for admitted in [
        None,
        Some(runtime.native_model_prompt_context.messages.clone()),
    ] {
        let messages = assembled(&plan, &node, &runtime, &templates, admitted, json!({}));
        let context = provider_context_from_prompt_messages(messages).unwrap();
        assert_eq!(context.system, blocks(&["same", "same", "same", "same"]));
        assert_eq!(context.system_sources.len(), 3);
        assert_eq!(context.system_sources[0]["source_kind"], "run_level_system");
        assert_eq!(context.system_sources[2]["source_kind"], "node_prompt");
    }
}

#[test]
fn callback_system_order_disabled_policy_keeps_existing_admitted_context_semantics() {
    let (plan, node, runtime, templates) = fixture();
    let messages = assembled(
        &plan,
        &node,
        &runtime,
        &templates,
        Some(history(&["admitted-inline"])),
        json!({"integration_context":"disabled"}),
    );
    let context = provider_context_from_prompt_messages(messages).unwrap();
    assert_eq!(context.system, blocks(&["admitted-inline", "application"]));
    assert_eq!(context.messages.len(), 3);
}

#[test]
fn callback_system_order_communication_retains_app_channel_owners_and_replaces_session() {
    let (plan, node, mut runtime, templates) = fixture();
    runtime.native_model_prompt_context.system = blocks(&["channel", "plugin"]);
    runtime.native_model_prompt_context.messages = vec![json!({
        "role":"user","content":"stale session","__communication_session":true
    })];
    let initial = provider_context_from_prompt_messages(assembled(
        &plan,
        &node,
        &runtime,
        &templates,
        None,
        json!({}),
    ))
    .unwrap();
    assert_eq!(
        initial.system,
        blocks(&["application", "channel", "plugin"])
    );
    let callback = provider_context_from_prompt_messages(assembled(
        &plan,
        &node,
        &runtime,
        &templates,
        Some(history(&["fresh inline"])),
        json!({}),
    ))
    .unwrap();
    assert_eq!(
        callback.system,
        blocks(&["application", "channel", "plugin", "fresh inline"])
    );
    assert_eq!(callback.messages.len(), 3);
    assert!(callback.messages.iter().all(|message| !matches!(
        message.content.as_str(),
        "stale session" | "old rendered query"
    )));
}
