use super::*;
use crate::ports::{RuntimeEventDurability, RuntimeEventSource};

fn item(phase: ProviderOutputItemPhase, kind: &str, name: &str) -> ProviderStreamEvent {
    ProviderStreamEvent::OutputItem {
        phase,
        output_index: 2,
        item: json!({"id":"item_2","call_id":"call_2","type":kind,"name":name,"arguments":""}),
    }
}

#[test]
fn progress_preserves_aliases_fragments_and_reserves_output_not_input_round() {
    let node = Uuid::now_v7();
    let input_round = Uuid::now_v7();
    let mut progress = ClientProgress::new(["Bash".into()].into(), Some(input_round), true);
    let start = progress
        .observe(
            "llm",
            node,
            &item(ProviderOutputItemPhase::Added, "function_call", "Bash"),
        )
        .unwrap();
    let callback_id = start.payload["callback_task_id"].clone();
    assert_ne!(callback_id, json!(input_round));
    assert_eq!(start.payload["response_round_id"], json!(input_round));
    assert_eq!(start.payload["node_run_id"], json!(node));
    assert_eq!(start.source, RuntimeEventSource::Provider);
    assert_eq!(start.durability, RuntimeEventDurability::Ephemeral);
    assert!(!start.persist_required);
    let mut pieces = String::new();
    for (alias, delta) in [("item_2", "{\n  \"command\": "), ("call_2", "\"pwd\"}")] {
        let event = progress
            .observe(
                "llm",
                node,
                &ProviderStreamEvent::ToolCallDelta {
                    call_id: alias.into(),
                    delta: json!(delta),
                },
            )
            .unwrap();
        assert_eq!(event.event_type, "provider_tool_call_delta");
        assert_eq!(event.payload["call_id"], "call_2");
        assert_eq!(event.payload["item_id"], "item_2");
        assert_eq!(event.payload["delta"], delta);
        assert_eq!(event.payload["callback_task_id"], callback_id);
        pieces.push_str(event.payload["delta"].as_str().unwrap());
    }
    assert_eq!(
        serde_json::from_str::<Value>(&pieces).unwrap(),
        json!({"command":"pwd"})
    );
    let done = item(ProviderOutputItemPhase::Done, "function_call", "Bash");
    assert!(
        progress.observe("llm", node, &done).is_none(),
        "tool stop must wait for durable admission"
    );
    let mut delivery = super::super::committed_tool_delivery_candidate("llm", node, &done)
        .unwrap()
        .unwrap();
    progress.bind_delivery(&mut delivery);
    assert_eq!(delivery.payload["reserved_callback_task_id"], callback_id);
    assert!(delivery.payload.get("committed_delivery").is_none());
}

#[test]
fn progress_does_not_publish_internal_unknown_or_foreign_source_tools() {
    let node = Uuid::now_v7();
    let mut progress = ClientProgress::new(["Bash".into()].into(), None, true);
    assert!(progress
        .observe(
            "llm",
            node,
            &item(
                ProviderOutputItemPhase::Added,
                "function_call",
                "host_private"
            )
        )
        .is_none());
    assert!(progress
        .observe(
            "llm",
            node,
            &ProviderStreamEvent::ToolCallDelta {
                call_id: "call_2".into(),
                delta: json!("{}")
            }
        )
        .is_none());
    let mut foreign = ClientProgress::new(["Bash".into()].into(), None, false);
    assert!(foreign
        .observe(
            "llm",
            node,
            &item(ProviderOutputItemPhase::Added, "function_call", "Bash")
        )
        .is_none());
    assert!(foreign
        .observe(
            "llm",
            node,
            &item(ProviderOutputItemPhase::Done, "reasoning", "")
        )
        .is_none());
}

#[test]
fn explicit_content_done_closes_reasoning_but_formal_duplicate_tool_delta_is_ignored() {
    let node = Uuid::now_v7();
    let mut progress = ClientProgress::new(["Bash".into()].into(), None, true);
    let done = progress
        .observe(
            "llm",
            node,
            &item(ProviderOutputItemPhase::Done, "reasoning", ""),
        )
        .unwrap();
    assert_eq!(done.event_type, "provider_content_block_done");
    assert!(progress.observe("llm", node, &ProviderStreamEvent::ResponsesOutputDelta {
        event: json!({"type":"response.function_call_arguments.delta","output_index":2,"item_id":"item_2","delta":"{}"}),
    }).is_none());
}

#[test]
fn canonical_writer_keeps_string_tool_fragments_exactly_once_beside_formal_lane() {
    use crate::orchestration_runtime::canonical_stream::{CanonicalCallId, CanonicalItemId};
    let mut writer = super::super::RuntimeCanonicalStreamWriter::new("llm");
    writer
        .write(&item(
            ProviderOutputItemPhase::Added,
            "function_call",
            "Bash",
        ))
        .unwrap();
    for delta in ["{\n", "  \"x\":1}"] {
        writer.write(&ProviderStreamEvent::ResponsesOutputDelta {
            event: json!({"type":"response.function_call_arguments.delta","output_index":2,"item_id":"item_2","delta":delta}),
        }).unwrap();
        writer
            .write(&ProviderStreamEvent::ToolCallDelta {
                call_id: "item_2".into(),
                delta: json!(delta),
            })
            .unwrap();
    }
    let call = CanonicalCallId::new(CanonicalItemId::new("llm"), "item_2");
    assert_eq!(
        writer
            .state()
            .accumulated()
            .tool_call(&call)
            .unwrap()
            .arguments()
            .as_str(),
        "{\n  \"x\":1}"
    );
}
