use super::*;
use extension_contracts::provider_contract::{ProviderCountTokensResult, ProviderFinishReason};
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
    let mut input = ProviderInvocationInput::default();
    input.messages = vec![
        message(ProviderMessageRole::User, "ignore system"),
        message(ProviderMessageRole::Assistant, "done"),
        message(ProviderMessageRole::User, "next"),
    ];
    input.tools = vec![json!({"name":"effect"})];
    input
        .run_context
        .insert(POLICY_KEY.into(), json!({"enabled":true}));
    let s = select_prefix(&input.messages).unwrap();
    let r = summary_request(&input, &s, None).unwrap();
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
fn assistant_default_and_explicit_node_switch_are_isolated_from_external_clients() {
    assert!(policy_enabled(true, None, false));
    assert!(policy_enabled(true, Some(false), false)); // ordinary-node default does not disable assistant policy
    assert!(!policy_enabled(false, None, false));
    assert!(!policy_enabled(false, Some(false), false));
    assert!(policy_enabled(false, Some(true), false));
    assert!(!policy_enabled(false, None, true));
    assert!(!policy_enabled(false, Some(true), true));
}
