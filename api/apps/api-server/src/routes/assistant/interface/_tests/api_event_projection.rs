use super::*;
fn operation(id: &str) -> ApiOperationContext {
    let declaration = DECLARATIONS
        .iter()
        .chain(CONVERSATION_DECLARATIONS)
        .chain(RUN_DECLARATIONS)
        .chain(RUN_STREAM_DECLARATIONS)
        .find(|declaration| declaration.interface_id == id)
        .expect("declared Assistant operation");
    ApiOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: declaration.path.into(),
    }
}
#[test]
fn assistant_before_keeps_real_page_content_and_omits_transport_credentials() {
    let op = operation("assistant.runs.create");
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "authorization",
        axum::http::HeaderValue::from_static("Bearer HEADER-SECRET-SENTINEL"),
    );
    headers.insert(
        "cookie",
        axum::http::HeaderValue::from_static("session=SESSION-SECRET-SENTINEL"),
    );
    let input = AssistantRunInput {
        body: StartAssistantRunBody {
            application_id: Uuid::nil(),
            conversation_id: None,
            query: "Public business query".repeat(300),
            page_references: (0..40)
                .map(|i| AssistantPageReferenceBody {
                    page_url: format!("https://app.test/{i}"),
                    page_title: "Page".into(),
                    outer_html: "<p>Full public context</p>".repeat(200),
                })
                .collect(),
            title: Some("Assistant".into()),
        },
        headers,
    };
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(
        value["body"]["page_references"].as_array().unwrap().len(),
        40
    );
    assert!(
        value["body"]["page_references"][0]["outer_html"]
            .as_str()
            .unwrap()
            .len()
            > 1000
    );
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(value.get("headers").is_none());
}
#[test]
fn assistant_real_result_is_complete_and_unit_stream_terminal_is_explicitly_unavailable() {
    let op = operation("assistant.runs.create");
    let payload = json!({"rows":(0..90).collect::<Vec<_>>(),"business_metadata":{"secret_label":"Public instruction"}});
    let response = AssistantRunResponse {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        conversation_id: Uuid::nil(),
        status: "completed".into(),
        answer: Some("Answer".repeat(500)),
        output_payload: payload.clone(),
        error_payload: None,
    };
    let expected = serde_json::to_value(&response).unwrap();
    assert_eq!(
        AssistantRunOutput(response)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
    assert_eq!(
        AssistantRunStreamOutput::document_output_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::MissingStreamingTerminal)
    );
    assert!(AssistantRunStreamOutput
        .project_document_output(&op)
        .is_err());
}
#[test]
fn assistant_conversation_query_preserves_documented_keys_and_omits_absent_optional_fields() {
    let op = operation("assistant.conversations.list");
    let input = AssistantConversationsInput::ListConversations(ListAssistantConversationsQuery {
        application_id: Uuid::nil(),
        page: None,
        page_size: Some(50),
    });
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"query":{"application_id":Uuid::nil(),"page_size":50}})
    );
    let input = AssistantConversationsInput::GetConversationMessages {
        conversation_id: Uuid::nil(),
        query: crate::routes::assistant::AssistantConversationMessagesQuery {
            application_id: Uuid::nil(),
        },
    };
    let op = operation("assistant.conversations.messages.get");
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"conversation_id":Uuid::nil()},"query":{"application_id":Uuid::nil()}})
    );
}
