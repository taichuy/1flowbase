use super::*;

#[test]
fn request_history_excludes_application_prompts_and_internal_summaries() {
    let payload = json!({
        "__native_model_prompt_context": {"messages": []},
        "node-start": {"system": "中文回复", "query": "q"},
        "llm_context": {"effective_system": "stitched prompt", "summary": "internal summary"}
    });
    assert!(application_run_conversation_contexts(&payload).is_empty());
    assert!(application_run_conversation_contexts(&json!({
        "node-start": {"system": "legacy application prompt"}
    }))
    .is_empty());
}

#[test]
fn request_history_preserves_submitted_context_roles_and_repeated_messages() {
    let contexts = application_run_conversation_contexts(&json!({
        "__native_model_prompt_context": {
            "system": [{"type": "text", "text": "submitted context"}],
            "messages": [
                {"role": "developer", "content": "submitted context"},
                {"role": "developer", "content": "submitted context"},
                {"role": "system", "content": [{"text": "client compacted summary"}]},
                {"role": "user", "content": "question"}
            ]
        },
        "node-start": {"system": "application addition"}
    }));
    assert_eq!(
        contexts
            .iter()
            .map(|entry| (
                entry.role.as_str(),
                entry.context_source,
                entry.content.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("system", "client_request", "submitted context"),
            ("developer", "client_request", "submitted context"),
            ("developer", "client_request", "submitted context"),
            ("system", "client_request", "client compacted summary")
        ]
    );
}
