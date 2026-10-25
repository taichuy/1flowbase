use super::*;
use control_plane::ports::ClaimedCommunicationInput;
use serde_json::json;

fn input() -> ClaimedCommunicationInput {
    let instance = Uuid::now_v7();
    let conversation = Uuid::now_v7();
    ClaimedCommunicationInput {
        binding:domain::CommunicationBindingRecord {
            workspace_id:Uuid::now_v7(),instance_id:instance,actor_user_id:Uuid::now_v7(),plugin_mcp_instance_id:"plugin-history".into(),revision:1,
            config:domain::CommunicationBindingInput { application_id:Uuid::now_v7(),model:None,reasoning_effort:None,file_table_id:None,mcp_instance_ids:vec!["plugin-history".into()],system_prompt:"Use the channel instructions.".into() },
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
