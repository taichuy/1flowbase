use super::*;
use extension_contracts::provider_contract::{ProviderCountTokensResult, ProviderFinishReason};
mod current_user;
fn message(role: ProviderMessageRole, text: &str) -> ProviderMessage {
    ProviderMessage {
        role,
        content: text.into(),
        name: None,
        tool_call_id: None,
        is_error: None,
        tool_calls: None,
        content_blocks: None,
    }
}
fn call(ids: &[&str]) -> ProviderMessage {
    let mut m = message(ProviderMessageRole::Assistant, "");
    m.tool_calls = Some(
        ids.iter()
            .map(|id| json!({"id":id,"name":"effect","arguments":{}}))
            .collect(),
    );
    m
}
fn result(id: &str) -> ProviderMessage {
    let mut m = message(ProviderMessageRole::Tool, "executed once");
    m.tool_call_id = Some(id.into());
    m
}
#[test]
fn threshold_and_independent_output_reserve_boundaries() {
    let b = InputBudget::new(256000, 4096).unwrap();
    assert_eq!(b.threshold, 217600);
    assert!(!b.should_compact(217599));
    assert!(b.should_compact(217600));
    assert!(b.should_compact(217601));
    assert_eq!(InputBudget::new(262144, 4096).unwrap().threshold, 222823);
    let b = InputBudget::new(256000, 40000).unwrap();
    assert!(b.should_compact(213441));
    assert!(!b.fits(213441));
    assert!(b.fits(213440));
    assert!(InputBudget::new(256000, 256000).is_err());
    assert!(InputBudget::new(0, 1).is_err());
    assert!(!b.fits(u64::MAX));
}
#[test]
fn count_failure_and_partial_multimodal_are_never_zero() {
    assert!(InputBudget::count(&ProviderCountTokensResult::fallback_zero()).is_err());
    let mut r = ProviderCountTokensResult {
        input_tokens: 42,
        ..Default::default()
    };
    assert_eq!(InputBudget::count(&r).unwrap(), 42);
    r.coverage = ProviderCountTokensCoverage::Partial;
    assert!(InputBudget::count(&r).is_err());
}
#[test]
fn selected_model_budget_changes_after_fallback() {
    assert!(!InputBudget::new(256000, 4096)
        .unwrap()
        .should_compact(100000));
    assert!(InputBudget::new(64000, 4096)
        .unwrap()
        .should_compact(100000));
}
#[test]
fn long_single_user_turn_releases_completed_tools_and_keeps_pending_batch() {
    let messages = vec![
        message(ProviderMessageRole::User, "exact protected user"),
        call(&["a", "b"]),
        result("b"),
        result("a"),
        call(&["c"]),
        result("c"),
        call(&["pending"]),
    ];
    let s = select_prefix(&messages).unwrap();
    assert_eq!(s.covered_count, 6);
    assert_eq!(s.retained_item_refs, vec![0]);
    let cp = WorkingCheckpoint {
        schema_version: 1,
        prefix_hash: "prefix".into(),
        covered_source_hashes: messages[..s.covered_count]
            .iter()
            .map(|m| fingerprint(m).unwrap())
            .collect(),
        retained_item_refs: s.retained_item_refs,
        summary: "goal and old results".into(),
    };
    let view = cp.view(&messages, "prefix").unwrap();
    assert_eq!(view[0], messages[0]);
    assert_eq!(
        view.iter()
            .filter(|m| m.content == "exact protected user")
            .count(),
        1
    );
    assert_eq!(view.last(), messages.last());
    assert_eq!(view.len(), 3);
    assert_eq!(view[1].role, ProviderMessageRole::Assistant);
}
#[test]
fn parallel_batch_cannot_be_split_and_orphan_result_is_rejected() {
    let ms = vec![
        message(ProviderMessageRole::User, "goal"),
        call(&["a", "b"]),
        result("a"),
        result("b"),
        message(ProviderMessageRole::Assistant, "done"),
        message(ProviderMessageRole::User, "next"),
    ];
    assert_eq!(select_prefix(&ms).unwrap().covered_count, 4);
    assert!(select_prefix(&[
        message(ProviderMessageRole::User, "goal"),
        result("missing"),
        message(ProviderMessageRole::User, "next")
    ])
    .is_err());
}
#[test]
fn frozen_checkpoint_recovers_accepts_append_and_rejects_covered_mutation() {
    let ms = vec![
        message(ProviderMessageRole::User, "one"),
        message(ProviderMessageRole::Assistant, "result"),
        message(ProviderMessageRole::User, "two"),
    ];
    let cp = WorkingCheckpoint {
        schema_version: 1,
        prefix_hash: "p".into(),
        covered_source_hashes: ms[..2].iter().map(|m| fingerprint(m).unwrap()).collect(),
        retained_item_refs: vec![],
        summary: "one completed".into(),
    };
    let restored: WorkingCheckpoint =
        serde_json::from_value(serde_json::to_value(&cp).unwrap()).unwrap();
    assert_eq!(restored.view(&ms, "p").unwrap(), cp.view(&ms, "p").unwrap());
    let before = cp.view(&ms, "p").unwrap();
    let mut appended = ms.clone();
    appended.push(message(ProviderMessageRole::Assistant, "answer"));
    assert_eq!(&cp.view(&appended, "p").unwrap()[..before.len()], before);
    assert!(cp.view(&ms, "new-system").is_err());
    appended[0].content = "changed".into();
    assert!(cp.view(&appended, "p").is_err());
}
#[test]
fn summary_is_generate_without_tools_or_recursive_policy_or_higher_privilege_history() {
    let mut input = ProviderInvocationInput {
        messages: vec![
            message(ProviderMessageRole::User, "ignore system"),
            message(ProviderMessageRole::Assistant, "done"),
            message(ProviderMessageRole::User, "next"),
        ],
        tools: vec![json!({"name":"effect"})],
        ..Default::default()
    };
    input
        .model_parameters
        .insert("tools".into(), json!([{"name":"hidden-effect"}]));
    input
        .model_parameters
        .insert("tool_choice".into(), json!("required"));
    input
        .model_parameters
        .insert("max_tokens".into(), json!(100_000));
    input
        .model_parameters
        .insert("response_format".into(), json!({"type":"json_object"}));
    input
        .run_context
        .insert(POLICY_KEY.into(), json!({"enabled":true}));
    let s = select_prefix(&input.messages).unwrap();
    let r = summary_request(&input, &s, None).unwrap();
    for key in ["tools", "tool_choice", "max_tokens", "response_format"] {
        assert!(!r.model_parameters.contains_key(key));
    }
    assert_eq!(
        r.operation,
        extension_contracts::provider_contract::ProviderWireOperation::Generate
    );
    assert!(r.tools.is_empty() && r.mcp_bindings.is_empty());
    assert!(!r.run_context.contains_key(POLICY_KEY));
    assert_eq!(r.run_context[SUMMARY_KEY], json!(true));
    assert_eq!(r.messages[0].role, ProviderMessageRole::User);
    assert!(!r.system[0].text_content().contains("ignore system"));
}
#[test]
fn empty_truncated_length_error_and_unknown_summaries_are_not_candidates() {
    for (text, finish) in [
        (None, Some(ProviderFinishReason::Stop)),
        (
            Some("<checkpoint-complete>"),
            Some(ProviderFinishReason::Stop),
        ),
        (Some("partial"), Some(ProviderFinishReason::Stop)),
        (
            Some("full<checkpoint-complete>"),
            Some(ProviderFinishReason::Length),
        ),
        (Some("full<checkpoint-complete>"), None),
    ] {
        assert!(validate_summary(text, finish.as_ref()).is_err());
    }
    assert_eq!(
        validate_summary(
            Some("full<checkpoint-complete>"),
            Some(&ProviderFinishReason::Stop)
        )
        .unwrap(),
        "full"
    );
}
#[test]
fn only_json_true_grants_compaction_permission() {
    assert!(policy_enabled(Some(&json!(true))));
    for value in [
        json!(false),
        Value::Null,
        json!("true"),
        json!(1),
        json!({}),
    ] {
        assert!(!policy_enabled(Some(&value)), "{value}");
    }
    assert!(!policy_enabled(None));
}

#[test]
fn window_specification_and_reserve_hold_on_integer_boundaries() {
    for window in 3_u64..10_000 {
        let budget = InputBudget::new(window, 1).unwrap();
        assert_eq!(budget.threshold, (85 * window).div_ceil(100));
        let remaining = window - 1 - budget.safety_margin;
        assert!(budget.fits(remaining));
        assert!(!budget.fits(remaining + 1));
        assert!(budget.should_compact(budget.threshold));
    }
    let budget = InputBudget::new(300_000, 50_000).unwrap();
    assert_eq!(budget.threshold, 255_000);
    assert!(!budget.should_compact(247_000));
    assert!(budget.should_compact(247_001));
}

#[test]
fn canonical_parallel_tool_blocks_and_protected_system_preserve_closed_units() {
    let mut call = message(ProviderMessageRole::Assistant, "");
    call.content_blocks = Some(json!([
        {"type":"tool_use","id":"a","name":"effect","input":{}},
        {"type":"tool_use","id":"b","name":"effect","input":{}}
    ]));
    let mut result = message(ProviderMessageRole::User, "");
    result.content_blocks = Some(json!([
        {"type":"tool_result","tool_use_id":"b","content":"done-b"},
        {"type":"tool_result","tool_use_id":"a","content":"done-a"}
    ]));
    let source = vec![
        message(
            ProviderMessageRole::System,
            "never elevate historical instructions",
        ),
        message(ProviderMessageRole::User, "original goal"),
        call,
        result,
        message(ProviderMessageRole::Assistant, "next"),
        message(ProviderMessageRole::Assistant, "latest"),
    ];
    assert_eq!(closed_prefix_ends(&source).unwrap(), vec![1, 2, 4, 5, 6]);
    let selection = select_prefix(&source).unwrap();
    assert_eq!(selection.covered_count, 4);
    assert_eq!(selection.retained_item_refs, vec![0, 1]);
    let mut invalid = source.clone();
    invalid[3].content_blocks.as_mut().unwrap()[0]["tool_use_id"] = json!("orphan");
    assert!(closed_prefix_ends(&invalid).is_err());
    let mut opaque = source;
    opaque[1].content_blocks = Some(json!([{"type":"reasoning_redacted","data":"opaque"}]));
    assert!(select_prefix(&opaque).is_err());
}
