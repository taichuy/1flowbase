use super::*;
use control_plane::{
    application_public_api::native::{
        NativeExecution, NativeObject, NativeRequestMetadata, NativeRunRequest,
    },
    ports::ClaimedCommunicationInput,
};
use extension_contracts::provider_contract::NativePromptBlock;
use serde_json::json;

pub(super) fn request(
    input: &ClaimedCommunicationInput,
    files: Vec<Value>,
) -> Result<NativeRunRequest> {
    let binding = &input.binding;
    let execution: NativeExecution = match &binding.config.reasoning_effort {
        Some(effort) => serde_json::from_value(
            json!({"model_parameters":{"reasoning":{"mode":"adaptive","effort":effort}}}),
        )?,
        None => NativeExecution::default(),
    };
    // Stable session identity excludes message id, timestamps and query. A new input changes
    // only the final user message, preserving prompt cache affinity within the conversation.
    let session = json!({"session_id":input.conversation.id,"channel_id":binding.instance_id,
        "workspace_id":binding.workspace_id,"user_id":input.conversation.external_identity,
        "conversation_id":input.conversation.external_conversation_key})
    .to_string();
    let mut query = input.message.content.clone().unwrap_or_default();
    if !files.is_empty() {
        query.push_str("\n\nAttached files: ");
        query.push_str(&serde_json::to_string(&files)?);
    }
    Ok(NativeRunRequest {
        query,
        system: vec![NativePromptBlock::text(
            binding.config.system_prompt.clone(),
        )],
        model: binding.config.model.clone(),
        inputs: NativeObject::default(),
        history: vec![json!({"role":"system","content":session})],
        attachments: files
            .into_iter()
            .map(serde_json::from_value)
            .collect::<std::result::Result<_, _>>()?,
        conversation: NativeObject::default(),
        expand_id: None,
        response_mode: None,
        stream_options: Default::default(),
        execution,
        metadata: NativeRequestMetadata::default(),
        request_context: Default::default(),
        title: None,
        client_protocol_envelope: None,
    })
}

pub(super) fn final_reply(item: &domain::ApplicationRunConversationMessageItem) -> Option<String> {
    // This business-turn projection sets persisted_answer only for final_answer_observed.
    // provider_output_item/projection_timeout/waiting_callback are intentionally ineligible.
    if item.status != "succeeded"
        || item.source_kind != "business_turn"
        || item.output_source.as_deref() != Some("persisted_answer")
    {
        return None;
    }
    let text = item.answer.as_deref()?;
    let reply = orchestration_runtime::answer_projection::answer_segments_from_text(text)
        .into_iter()
        .filter(|segment| {
            segment.kind
                == orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Message
        })
        .map(|segment| segment.text)
        .collect::<String>();
    (!reply.trim().is_empty()).then_some(reply)
}
