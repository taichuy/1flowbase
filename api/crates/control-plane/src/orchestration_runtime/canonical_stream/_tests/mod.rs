use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderOutputItemPhase, ProviderRuntimeError, ProviderRuntimeErrorKind,
    ProviderUsage,
};
use serde_json::json;

use super::{
    CanonicalBlockId, CanonicalCallId, CanonicalContentKind, CanonicalItemId, CanonicalStreamEvent,
    CanonicalStreamState, CanonicalStreamTransitionError, CanonicalTerminal,
};

fn item_id(value: &str) -> CanonicalItemId {
    CanonicalItemId::new(value)
}

fn block_id(item: &str, block: &str) -> CanonicalBlockId {
    CanonicalBlockId::new(item_id(item), block)
}

fn call_id(item: &str, call: &str) -> CanonicalCallId {
    CanonicalCallId::new(item_id(item), call)
}

#[test]
fn ac_001_text_and_reasoning_preserve_exact_event_bytes_and_repetitions() {
    let text_block = block_id("item-1", "text-1");
    let reasoning_block = block_id("item-1", "reasoning-1");
    let mut state = CanonicalStreamState::default();

    for delta in ["alpha  ", "\n", "\n", "`code`", "  omega", "  omega"] {
        state
            .apply(CanonicalStreamEvent::TextDelta {
                block_id: text_block.clone(),
                delta: delta.to_string(),
            })
            .unwrap();
    }
    for delta in ["think ", "think ", "\n```reasoning```\n"] {
        state
            .apply(CanonicalStreamEvent::ReasoningDelta {
                block_id: reasoning_block.clone(),
                delta: delta.to_string(),
            })
            .unwrap();
    }

    assert_eq!(
        state.accumulated().text().as_str(),
        "alpha  \n\n`code`  omega  omega"
    );
    assert_eq!(
        state.accumulated().text().segments(),
        &["alpha  ", "\n", "\n", "`code`", "  omega", "  omega"]
    );
    assert_eq!(
        state.accumulated().reasoning().as_str(),
        "think think \n```reasoning```\n"
    );
}

#[test]
fn ac_001_text_materialization_follows_events_across_block_identities() {
    let first = block_id("item-1", "block-a");
    let second = block_id("item-2", "block-b");
    let mut state = CanonicalStreamState::default();

    for (block_id, delta) in [
        (first.clone(), "same"),
        (second, "  middle\n"),
        (first.clone(), "same"),
    ] {
        state
            .apply(CanonicalStreamEvent::TextDelta {
                block_id,
                delta: delta.to_string(),
            })
            .unwrap();
    }

    assert_eq!(state.accumulated().text().as_str(), "same  middle\nsame");
    assert_eq!(
        state
            .accumulated()
            .block(&first)
            .unwrap()
            .content()
            .as_str(),
        "samesame"
    );
}

#[test]
fn ac_003_tool_argument_segments_remain_ordered_per_call() {
    let first = call_id("item-tools", "call-a");
    let second = call_id("item-tools", "call-b");
    let mut state = CanonicalStreamState::default();

    for (call_id, delta) in [
        (first.clone(), "{\"query\":\""),
        (second.clone(), "{\"id\":"),
        (first.clone(), "same same"),
        (first.clone(), "same same"),
        (second.clone(), "42}"),
        (first.clone(), "\"}"),
    ] {
        state
            .apply(CanonicalStreamEvent::ToolArgumentsDelta {
                call_id,
                delta: delta.to_string(),
            })
            .unwrap();
    }

    let calls = state.accumulated().items()[0].tool_calls();
    assert_eq!(calls[0].id(), &first);
    assert_eq!(calls[1].id(), &second);
    assert_eq!(
        calls[0].arguments().segments(),
        &["{\"query\":\"", "same same", "same same", "\"}"]
    );
    assert_eq!(
        calls[0].arguments().as_str(),
        "{\"query\":\"same samesame same\"}"
    );
    assert_eq!(calls[1].arguments().as_str(), "{\"id\":42}");
}

#[test]
fn ac_004_usage_deltas_add_and_partial_snapshots_replace_present_fields() {
    let mut state = CanonicalStreamState::default();
    state
        .apply(CanonicalStreamEvent::UsageDelta {
            usage: ProviderUsage {
                input_tokens: Some(2),
                cache_read_tokens: Some(1),
                total_tokens: Some(2),
                ..ProviderUsage::default()
            },
        })
        .unwrap();
    state
        .apply(CanonicalStreamEvent::UsageDelta {
            usage: ProviderUsage {
                input_tokens: Some(3),
                total_tokens: Some(3),
                ..ProviderUsage::default()
            },
        })
        .unwrap();
    state
        .apply(CanonicalStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                input_tokens: Some(10),
                output_tokens: Some(4),
                total_tokens: Some(14),
                ..ProviderUsage::default()
            },
        })
        .unwrap();
    state
        .apply(CanonicalStreamEvent::UsageDelta {
            usage: ProviderUsage {
                output_tokens: Some(1),
                total_tokens: Some(1),
                ..ProviderUsage::default()
            },
        })
        .unwrap();

    assert_eq!(
        state.accumulated().usage().value(),
        &ProviderUsage {
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: Some(1),
            total_tokens: Some(15),
            ..ProviderUsage::default()
        }
    );
}

#[test]
fn ac_006_terminal_state_is_absorbing_and_rejects_every_later_event() {
    let mut state = CanonicalStreamState::default();
    state
        .apply(CanonicalStreamEvent::TextDelta {
            block_id: block_id("item-1", "text-1"),
            delta: "done".to_string(),
        })
        .unwrap();
    state
        .apply(CanonicalStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        })
        .unwrap();
    let terminal_state = state.clone();

    let rejected_events = vec![
        CanonicalStreamEvent::TextDelta {
            block_id: block_id("item-1", "text-1"),
            delta: " must not appear".to_string(),
        },
        CanonicalStreamEvent::ReasoningDelta {
            block_id: block_id("item-1", "reasoning-1"),
            delta: "must not appear".to_string(),
        },
        CanonicalStreamEvent::ToolArgumentsDelta {
            call_id: call_id("item-1", "call-1"),
            delta: "{}".to_string(),
        },
        CanonicalStreamEvent::UsageDelta {
            usage: ProviderUsage {
                output_tokens: Some(1),
                ..ProviderUsage::default()
            },
        },
        CanonicalStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                total_tokens: Some(1),
                ..ProviderUsage::default()
            },
        },
        CanonicalStreamEvent::Finish {
            reason: ProviderFinishReason::Length,
        },
        CanonicalStreamEvent::Fail {
            error: ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderUpstreamError,
                "late failure",
            ),
        },
    ];
    for event in rejected_events {
        let error = state.apply(event).unwrap_err();
        assert_eq!(error, CanonicalStreamTransitionError::StreamAlreadyTerminal);
        assert_eq!(state, terminal_state);
    }

    assert_eq!(state.accumulated().text().as_str(), "done");
    assert_eq!(
        state.terminal(),
        Some(&CanonicalTerminal::Finished {
            reason: ProviderFinishReason::Stop
        })
    );
}

#[test]
fn ac_006_failure_is_a_typed_terminal_state() {
    let failure = ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderInvalidResponse,
        "invalid provider frame",
    );
    let mut state = CanonicalStreamState::default();

    state
        .apply(CanonicalStreamEvent::Fail {
            error: failure.clone(),
        })
        .unwrap();

    assert_eq!(
        state.terminal(),
        Some(&CanonicalTerminal::Failed { error: failure })
    );
    assert_eq!(
        state
            .apply(CanonicalStreamEvent::Finish {
                reason: ProviderFinishReason::Stop,
            })
            .unwrap_err(),
        CanonicalStreamTransitionError::StreamAlreadyTerminal
    );
}

#[test]
fn ac_001_logical_text_is_invariant_under_event_partitioning() {
    let id = block_id("item-1", "text-1");
    let mut one_event = CanonicalStreamState::default();
    let mut many_events = CanonicalStreamState::default();

    one_event
        .apply(CanonicalStreamEvent::TextDelta {
            block_id: id.clone(),
            delta: "same  text\n`code`".to_string(),
        })
        .unwrap();
    for delta in ["same", "  ", "text", "\n", "`", "code", "`"] {
        many_events
            .apply(CanonicalStreamEvent::TextDelta {
                block_id: id.clone(),
                delta: delta.to_string(),
            })
            .unwrap();
    }

    assert_eq!(
        one_event.accumulated().text().as_str(),
        many_events.accumulated().text().as_str()
    );
    assert_ne!(
        one_event.accumulated().text().segments(),
        many_events.accumulated().text().segments()
    );
}

#[test]
fn ac_003_a_block_identity_cannot_change_content_kind() {
    let id = block_id(" item ", " block ");
    let mut state = CanonicalStreamState::default();
    state
        .apply(CanonicalStreamEvent::TextDelta {
            block_id: id.clone(),
            delta: "exact".to_string(),
        })
        .unwrap();
    let unchanged = state.clone();

    let error = state
        .apply(CanonicalStreamEvent::ReasoningDelta {
            block_id: id.clone(),
            delta: "wrong kind".to_string(),
        })
        .unwrap_err();

    assert_eq!(
        error,
        CanonicalStreamTransitionError::ContentKindConflict {
            block_id: id.clone(),
            existing: CanonicalContentKind::Text,
            incoming: CanonicalContentKind::Reasoning,
        }
    );
    assert_eq!(state, unchanged);
    assert_eq!(state.accumulated().items()[0].id().as_str(), " item ");
    assert_eq!(
        state.accumulated().block(&id).unwrap().id().as_str(),
        " block "
    );
}

#[test]
fn provider_output_item_phases_preserve_order_and_require_matching_added_item() {
    let added = json!({
        "id": "approval_1",
        "type": "mcp_approval_request",
        "name": "delete_record"
    });
    let done = json!({
        "id": "approval_1",
        "type": "mcp_approval_request",
        "name": "delete_record",
        "status": "completed"
    });
    let mut state = CanonicalStreamState::default();
    state
        .apply(CanonicalStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Added,
            output_index: 3,
            item: added.clone(),
        })
        .unwrap();
    state
        .apply(CanonicalStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Done,
            output_index: 3,
            item: done.clone(),
        })
        .unwrap();

    let phases = state.accumulated().output_items();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].phase(), ProviderOutputItemPhase::Added);
    assert_eq!(phases[0].item(), &added);
    assert_eq!(phases[1].phase(), ProviderOutputItemPhase::Done);
    assert_eq!(phases[1].output_index(), 3);
    assert_eq!(phases[1].item(), &done);
}

#[test]
fn provider_output_item_rejects_unknown_type_mismatch_and_post_terminal_phase() {
    let mut state = CanonicalStreamState::default();
    let invalid = state
        .apply(CanonicalStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Added,
            output_index: 0,
            item: json!({ "id": "computer_1", "type": "computer_call" }),
        })
        .unwrap_err();
    assert!(matches!(
        invalid,
        CanonicalStreamTransitionError::InvalidOutputItem { .. }
    ));

    state
        .apply(CanonicalStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Added,
            output_index: 0,
            item: json!({ "id": "call_1", "type": "mcp_call" }),
        })
        .unwrap();
    assert_eq!(
        state
            .apply(CanonicalStreamEvent::OutputItem {
                phase: ProviderOutputItemPhase::Done,
                output_index: 0,
                item: json!({ "id": "call_2", "type": "mcp_call" }),
            })
            .unwrap_err(),
        CanonicalStreamTransitionError::OutputItemDoneMismatch { output_index: 0 }
    );
    state
        .apply(CanonicalStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        })
        .unwrap();
    assert_eq!(
        state
            .apply(CanonicalStreamEvent::OutputItem {
                phase: ProviderOutputItemPhase::Done,
                output_index: 0,
                item: json!({ "id": "call_1", "type": "mcp_call" }),
            })
            .unwrap_err(),
        CanonicalStreamTransitionError::StreamAlreadyTerminal
    );
}
