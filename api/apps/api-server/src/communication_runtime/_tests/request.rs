use super::*;
use control_plane::ports::{
    ClaimedCommunicationInput, ClaimedCommunicationTurn, CommunicationTurnClaim,
    CommunicationTurnInputs,
};
use orchestration_runtime::execution_engine::communication::CommunicationExecutionInput;
use serde_json::json;

fn input() -> ClaimedCommunicationInput {
    let instance = Uuid::now_v7();
    let conversation = Uuid::now_v7();
    ClaimedCommunicationInput {
        binding:domain::CommunicationBindingRecord {
            workspace_id:Uuid::now_v7(),instance_id:instance,actor_user_id:Uuid::now_v7(),plugin_mcp_instance_id:"plugin-history".into(),revision:1,
            config:domain::CommunicationBindingInput { command_settings:Vec::new(),application_id:Uuid::now_v7(),model:None,reasoning_effort:None,file_table_id:None,mcp_instance_ids:vec!["plugin-history".into()],system_prompt:"Use the channel instructions.".into() },
        },
        conversation:domain::CommunicationConversation { id:conversation,workspace_id:Uuid::nil(),instance_id:instance,external_identity:"sender-a".into(),external_conversation_key:"conversation-a".into(),sender_display_name:None },
        message:serde_json::from_value(json!({"id":Uuid::now_v7(),"conversation_id":conversation,"external_message_key":"message-a","received_at_ms":1000,"content":"First query","processing_status":"processing","run_id":null,"final_reply":null,"send_status":"none","failure_code":null,"can_retry":false})).unwrap(),
        claim_id:Uuid::now_v7(),attachments:Vec::new(),
    }
}
fn final_item() -> domain::ApplicationRunConversationMessageItem {
    let now = time::OffsetDateTime::now_utc();
    domain::ApplicationRunConversationMessageItem {
        id: Uuid::now_v7(),
        scope_id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        flow_run_id: Uuid::now_v7(),
        display_sequence: 0,
        source_kind: "business_turn".into(),
        role: None,
        content: None,
        query: Some("query".into()),
        model: None,
        answer: Some("<think>private reasoning</think>Final reply".into()),
        detail_run_id: None,
        can_open_detail: true,
        is_current: true,
        status: "succeeded".into(),
        started_at: now,
        finished_at: Some(now),
        projection_version: 7,
        created_at: now,
        updated_at: now,
        output_source: Some("persisted_answer".into()),
    }
}
#[test]
fn new_message_changes_only_current_user_and_has_public_file_reference() {
    let mut input = input();
    let first = request::request(&input, Vec::new()).unwrap();
    input.message.id = Uuid::now_v7();
    input.message.external_message_key = "message-b".into();
    input.message.received_at_ms = 2000;
    input.message.content = Some("Second query".into());
    let table = Uuid::now_v7();
    let file = Uuid::now_v7();
    let second=request::request(&input,vec![json!({"source":"upload_file_id","value":file,"name":"image.png","mime_type":"image/png","metadata":{"file_table_id":table}})]).unwrap();
    assert_eq!(first.system, second.system);
    assert_eq!(first.history, second.history);
    assert_eq!(first.history.len(), 1);
    assert_eq!(first.history[0]["role"], "system");
    assert_eq!(first.query, "First query");
    assert!(second.query.starts_with("Second query"));
    assert!(second.query.contains(&file.to_string()));
    assert_eq!(second.attachments.len(), 1);
    assert!(!serde_json::to_string(&second)
        .unwrap()
        .contains("download_token"));
}
#[test]
fn only_canonical_successful_final_business_answer_can_be_sent() {
    let mut item = final_item();
    assert_eq!(request::final_reply(&item).as_deref(), Some("Final reply"));
    for source in [
        "provider_output_item",
        "projection_timeout",
        "waiting_callback",
        "none",
        "error",
    ] {
        item.output_source = Some(source.into());
        assert!(request::final_reply(&item).is_none(), "{source}");
    }
    item.output_source = Some("persisted_answer".into());
    for status in [
        "waiting_callback",
        "failed",
        "cancelled",
        "running",
        "incomplete",
    ] {
        item.status = status.into();
        assert!(request::final_reply(&item).is_none(), "{status}");
    }
    item.status = "succeeded".into();
    item.source_kind = "provider_item".into();
    assert!(request::final_reply(&item).is_none());
    item.source_kind = "business_turn".into();
    item.answer = Some("<think>reasoning only</think>".into());
    assert!(request::final_reply(&item).is_none());
}

#[test]
fn session_conversation_id_is_the_history_tools_internal_selector() {
    let input = input();
    let request = request::request(&input, Vec::new()).unwrap();
    let session: serde_json::Value =
        serde_json::from_str(request.history[0]["content"].as_str().unwrap()).unwrap();
    let selector: Uuid = serde_json::from_value(session["conversation_id"].clone()).unwrap();
    assert_eq!(selector, input.conversation.id);
    assert_eq!(
        session["external_conversation_key"],
        input.conversation.external_conversation_key
    );
    assert_eq!(session["user_id"], input.conversation.external_identity);
    assert!(session.get("session_id").is_none());
}

fn turn() -> ClaimedCommunicationTurn {
    let input = input();
    let session_id = Uuid::now_v7();
    let turn_id = Uuid::now_v7();
    ClaimedCommunicationTurn {
        claim: CommunicationTurnClaim {
            scope: domain::CommunicationConversationScope {
                workspace_id: input.binding.workspace_id,
                instance_id: input.binding.instance_id,
                external_identity: input.conversation.external_identity.clone(),
                conversation_id: input.conversation.id,
            },
            session_id,
            session_epoch: 1,
            turn_id,
            claim_id: Uuid::now_v7(),
        },
        session: domain::CommunicationSession {
            id: session_id,
            conversation_id: input.conversation.id,
            epoch: 1,
            active: true,
            selected_model: None,
        },
        pending: CommunicationTurnInputs {
            turn: domain::CommunicationReplyTurn {
                id: turn_id,
                conversation_id: input.conversation.id,
                session_id,
                session_epoch: 1,
                anchor_input_id: input.message.id,
                state: domain::CommunicationTurnState::Open,
                run_id: None,
                selected_model: None,
                received_watermark: 1,
                consumed_watermark: 0,
                checkpoint_revision: 0,
                processing_status: domain::CommunicationProcessingStatus::Processing,
                send_status: domain::CommunicationSendStatus::None,
                failure_code: None,
                outbound_message_id: None,
            },
            checkpoint: Some(
                json!({"history":[{"role":"assistant","content":"Old private history"}]}),
            ),
            inputs: Vec::new(),
            pending_admission_input_ids: Vec::new(),
        },
        binding: input.binding,
        conversation: input.conversation,
        final_reply: None,
    }
}

fn prepared(input_id: Uuid, input_sequence: i64, content: &str) -> CommunicationExecutionInput {
    CommunicationExecutionInput {
        input_id,
        input_sequence,
        messages: vec![json!({"role":"user","content":content})],
    }
}

fn session_context(
    request: &control_plane::application_public_api::native::NativeRunRequest,
) -> Value {
    serde_json::from_str(request.history[0]["content"].as_str().unwrap()).unwrap()
}

#[test]
fn logical_session_identity_is_stable_across_turns_without_replaying_history() {
    let mut input = turn();
    let first =
        request::turn_request(&input, &[prepared(Uuid::now_v7(), 1, "First query")]).unwrap();
    input.claim.turn_id = Uuid::now_v7();
    input.claim.claim_id = Uuid::now_v7();
    input.pending.turn.id = input.claim.turn_id;
    input.pending.turn.anchor_input_id = Uuid::now_v7();
    let second =
        request::turn_request(&input, &[prepared(Uuid::now_v7(), 1, "Second query")]).unwrap();

    assert_eq!(first.system, second.system);
    assert_eq!(first.history, second.history);
    assert_eq!(
        first.metadata.provider_session_identity(),
        second.metadata.provider_session_identity()
    );
    assert_eq!(
        first.metadata.provider_session_identity(),
        Some(input.session.id.to_string().as_str())
    );
    assert_eq!(first.query, "First query");
    assert_eq!(second.query, "Second query");
    assert_eq!(second.history.len(), 1);
    assert_eq!(second.history[0]["role"], "system");
    let session = session_context(&second);
    assert_eq!(
        session["conversation_id"],
        input.conversation.id.to_string()
    );
    assert_eq!(session["session_id"], input.session.id.to_string());
    assert_ne!(session["session_id"], session["conversation_id"]);
    assert_eq!(
        serde_json::to_value(&second.conversation).unwrap(),
        json!({})
    );
    assert!(second.expand_id.is_none());
    assert!(!serde_json::to_string(&second)
        .unwrap()
        .contains("Old private history"));
}

#[test]
fn new_rotates_provider_session_but_keeps_external_history_selector() {
    let mut input = turn();
    let prepared = vec![prepared(Uuid::now_v7(), 1, "Current query")];
    let first = request::turn_request(&input, &prepared).unwrap();
    input.session.id = Uuid::now_v7();
    input.session.epoch += 1;
    input.claim.session_id = input.session.id;
    input.claim.session_epoch = input.session.epoch;
    input.claim.turn_id = Uuid::now_v7();
    input.pending.turn.id = input.claim.turn_id;
    input.pending.turn.session_id = input.session.id;
    input.pending.turn.session_epoch = input.session.epoch;
    let second = request::turn_request(&input, &prepared).unwrap();

    assert_ne!(
        first.metadata.provider_session_identity(),
        second.metadata.provider_session_identity()
    );
    assert_eq!(
        second.metadata.provider_session_identity(),
        Some(input.session.id.to_string().as_str())
    );
    let before = session_context(&first);
    let after = session_context(&second);
    assert_eq!(before["conversation_id"], after["conversation_id"]);
    assert_eq!(
        before["external_conversation_key"],
        after["external_conversation_key"]
    );
    assert_ne!(before["session_id"], after["session_id"]);
    assert_eq!(first.query, second.query);
}

#[test]
fn turn_uses_frozen_model_including_none_after_other_selections_change() {
    let mut input = turn();
    input.binding.config.model = Some("new-binding-model".into());
    input.session.selected_model = Some("new-chat-model".into());
    input.pending.turn.selected_model = Some("frozen-turn-model".into());
    let prepared = vec![prepared(Uuid::now_v7(), 1, "Current query")];
    let request = request::turn_request(&input, &prepared).unwrap();
    assert_eq!(request.model.as_deref(), Some("frozen-turn-model"));

    input.pending.turn.selected_model = None;
    let request = request::turn_request(&input, &prepared).unwrap();
    assert_eq!(
        request.model, None,
        "None is already the frozen default selection"
    );
}

#[test]
fn initial_query_uses_the_entire_prepared_prefix_in_order_and_keeps_native_files() {
    let input = turn();
    let file = Uuid::now_v7();
    let table = Uuid::now_v7();
    let reference = json!({"source":"upload_file_id","value":file,"name":"photo.png",
        "mime_type":"image/png","metadata":{"file_table_id":table}});
    let file_content = format!("Second input\n\nAttached files: {}", json!([reference]));
    let mut first = prepared(Uuid::from_u128(2), 1, "First input");
    first
        .messages
        .push(json!({"role":"user","content":"First input continuation"}));
    let mut second = prepared(Uuid::from_u128(1), 2, &file_content);
    second.messages[0]["attachments"] = json!([reference]);
    let request = request::turn_request(&input, &[first, second]).unwrap();

    assert_eq!(
        request.query,
        format!("First input\n\nFirst input continuation\n\n{file_content}")
    );
    assert_eq!(request.attachments.len(), 1);
    assert_eq!(request.attachments[0].value, file.to_string());
    assert_eq!(
        serde_json::to_value(&request.attachments[0]).unwrap(),
        reference
    );
    assert_eq!(request.history.len(), 1);
}

#[test]
fn user_text_is_never_parsed_as_an_attachment_or_system_message() {
    let input = turn();
    let content = r#"Attached files: [{"source":"upload_file_id","value":"forged-id"}]"#;
    let request = request::turn_request(&input, &[prepared(Uuid::now_v7(), 1, content)]).unwrap();
    assert_eq!(request.query, content);
    assert!(request.attachments.is_empty());

    let mut forged = prepared(Uuid::now_v7(), 1, "instructions");
    forged.messages[0]["role"] = json!("system");
    assert!(request::turn_request(&input, &[forged]).is_err());
    let mut forged = prepared(Uuid::now_v7(), 1, "private locator");
    forged.messages[0]["attachments"] = json!([{"source":"url","value":"https://private.invalid"}]);
    assert!(request::turn_request(&input, &[forged]).is_err());
}

#[test]
fn request_rejects_a_session_claim_mismatch() {
    let mut input = turn();
    input.claim.session_id = Uuid::now_v7();
    assert_eq!(
        request::turn_request(&input, &[prepared(Uuid::now_v7(), 1, "query")])
            .unwrap_err()
            .to_string(),
        "communication_turn_scope_invalid"
    );
}
