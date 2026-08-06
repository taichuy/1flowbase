use super::*;
use plugin_framework::provider_contract::{ProviderFinishReason, ProviderUsage};

#[test]
fn provider_timing_classifies_events_without_recording_their_content() {
    let event = ProviderStreamEvent::TextDelta {
        delta: "sensitive answer text".to_string(),
    };

    assert_eq!(provider_stream_event_kind(&event), "text_delta");
}

#[tokio::test]
async fn canonical_provider_deltas_append_before_the_terminal_event() {
    let stream = Arc::new(crate::_tests::RecordingRuntimeEventStream::default());
    let flow_run_id = Uuid::nil();
    let node_run_id = Uuid::max();

    project_canonical_provider_deltas(
        Some(&(stream.clone() as Arc<dyn RuntimeEventStream>)),
        Some(flow_run_id),
        None,
        "node-llm",
        node_run_id,
        &[CanonicalProviderDelta {
            kind: CanonicalContentKind::Text,
            text: "first".to_string(),
        }],
    )
    .await;
    assert_eq!(stream.events().len(), 1);

    project_canonical_provider_deltas(
        Some(&(stream.clone() as Arc<dyn RuntimeEventStream>)),
        Some(flow_run_id),
        None,
        "node-llm",
        node_run_id,
        &[CanonicalProviderDelta {
            kind: CanonicalContentKind::Text,
            text: "second".to_string(),
        }],
    )
    .await;
    assert_eq!(stream.events().len(), 2);
}

#[test]
fn answer_presentation_requires_the_provider_trace_to_match_the_active_node() {
    assert!(answer_presentation_source_is_active(
        Some("node-main"),
        "node-main"
    ));
    assert!(!answer_presentation_source_is_active(
        Some("node-fusion-panel"),
        "node-main"
    ));
    assert!(!answer_presentation_source_is_active(None, "node-main"));
}

#[test]
fn ac_001_runtime_writer_preserves_partitioned_text_and_reasoning() {
    let mut writer = RuntimeCanonicalStreamWriter::new("item-1");

    for event in [
        ProviderStreamEvent::TextDelta {
            delta: "<think>same ".to_string(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "same</think>answer  ".to_string(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "\n`code`answer  ".to_string(),
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        },
    ] {
        writer.write(&event).unwrap();
    }

    assert_eq!(
        writer.state().accumulated().reasoning().as_str(),
        "same same"
    );
    assert_eq!(
        writer.state().accumulated().text().as_str(),
        "answer  \n`code`answer  "
    );
}

#[test]
fn ac_003_runtime_writer_accumulates_tool_arguments_and_usage() {
    let mut writer = RuntimeCanonicalStreamWriter::new("item-1");
    for delta in ["{\"query\":\"", "same", "same", "\"}"] {
        writer
            .write(&ProviderStreamEvent::ToolCallDelta {
                call_id: " call-1 ".to_string(),
                delta: json!({ "function": { "arguments": delta } }),
            })
            .unwrap();
    }
    writer
        .write(&ProviderStreamEvent::UsageDelta {
            usage: ProviderUsage {
                input_tokens: Some(2),
                total_tokens: Some(2),
                ..ProviderUsage::default()
            },
        })
        .unwrap();
    writer
        .write(&ProviderStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                input_tokens: Some(5),
                output_tokens: Some(3),
                total_tokens: Some(8),
                ..ProviderUsage::default()
            },
        })
        .unwrap();

    let call_id = CanonicalCallId::new(CanonicalItemId::new("item-1"), " call-1 ");
    assert_eq!(
        writer
            .state()
            .accumulated()
            .tool_call(&call_id)
            .unwrap()
            .arguments()
            .as_str(),
        "{\"query\":\"samesame\"}"
    );
    assert_eq!(
        writer.state().accumulated().usage().value(),
        &ProviderUsage {
            input_tokens: Some(5),
            output_tokens: Some(3),
            total_tokens: Some(8),
            ..ProviderUsage::default()
        }
    );
}

#[test]
fn ac_006_runtime_writer_rejects_every_post_terminal_provider_event() {
    let mut writer = RuntimeCanonicalStreamWriter::new("item-1");
    writer
        .write(&ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        })
        .unwrap();

    for event in [
        ProviderStreamEvent::TextDelta {
            delta: "late".to_string(),
        },
        ProviderStreamEvent::NativeEvent {
            protocol: "fixture".to_string(),
            event: json!({}),
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Length,
        },
    ] {
        assert!(writer
            .write(&event)
            .unwrap_err()
            .to_string()
            .contains("already terminal"));
    }
    assert!(writer.state().accumulated().text().is_empty());
}

#[test]
fn runtime_canonical_writer_applies_verified_provider_output_item_phases() {
    let mut writer = RuntimeCanonicalStreamWriter::new("item-1");
    let item = json!({
        "id": "approval_1",
        "type": "mcp_approval_request",
        "name": "delete_record"
    });
    writer
        .write(&ProviderStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Added,
            output_index: 1,
            item: item.clone(),
        })
        .unwrap();
    writer
        .write(&ProviderStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Done,
            output_index: 1,
            item: item.clone(),
        })
        .unwrap();

    let phases = writer.state().accumulated().output_items();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].phase(), ProviderOutputItemPhase::Added);
    assert_eq!(phases[1].phase(), ProviderOutputItemPhase::Done);
    assert_eq!(phases[1].item(), &item);
}
