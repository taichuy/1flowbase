use super::*;
use control_plane::{
    application_public_api::native::{
        NativeAttachment, NativeAttachmentSource, NativeExecution, NativeObject,
        NativeRequestMetadata, NativeRunRequest,
    },
    ports::{ClaimedCommunicationInput, ClaimedCommunicationTurn},
};
use extension_contracts::provider_contract::NativePromptBlock;
use orchestration_runtime::execution_engine::communication::CommunicationExecutionInput;
use serde_json::json;

pub(super) fn request(
    input: &ClaimedCommunicationInput,
    files: Vec<Value>,
) -> Result<NativeRunRequest> {
    let binding = &input.binding;
    let mut request = binding_request(binding)?;
    // Stable session identity excludes message id, timestamps and query. A new input changes
    // only the final user message, preserving prompt cache affinity within the conversation.
    let session = history_context(binding, &input.conversation).to_string();
    request.query = input.message.content.clone().unwrap_or_default();
    if !files.is_empty() {
        request.query.push_str("\n\nAttached files: ");
        request.query.push_str(&serde_json::to_string(&files)?);
    }
    request.history = vec![json!({"role":"system","content":session})];
    request.attachments = files
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<_, _>>()?;
    Ok(request)
}

pub(super) fn turn_request(
    input: &ClaimedCommunicationTurn,
    prepared: &[CommunicationExecutionInput],
) -> Result<NativeRunRequest> {
    ensure!(
        input.session.id == input.claim.session_id
            && input.session.epoch == input.claim.session_epoch
            && input.session.conversation_id == input.conversation.id
            && input.pending.turn.session_id == input.session.id
            && input.pending.turn.session_epoch == input.session.epoch
            && input.pending.turn.id == input.claim.turn_id,
        "communication_turn_scope_invalid"
    );
    let mut request = binding_request(&input.binding)?;
    // Admission freezes the effective model, including the absence of an override.
    // A later binding/chat selection must not change this turn's request.
    request.model = input.pending.turn.selected_model.clone();
    request
        .metadata
        .set_provider_session_identity(Some(input.session.id.to_string()));
    let mut session = history_context(&input.binding, &input.conversation);
    session["session_id"] = json!(input.session.id);
    request.history = vec![json!({"role":"system","content":session.to_string()})];

    let mut contents = Vec::new();
    for input in prepared {
        for message in &input.messages {
            ensure!(
                message.get("role").and_then(Value::as_str) == Some("user"),
                "communication_prepared_message_invalid"
            );
            contents.push(
                message
                    .get("content")
                    .and_then(Value::as_str)
                    .context("communication_prepared_message_invalid")?,
            );
            if let Some(files) = message.get("attachments") {
                let files: Vec<NativeAttachment> = serde_json::from_value(files.clone())?;
                ensure!(
                    files
                        .iter()
                        .all(|file| file.source == NativeAttachmentSource::UploadFileId),
                    "communication_prepared_attachment_invalid"
                );
                request.attachments.extend(files);
            }
        }
    }
    // Only this claimed prefix belongs in the initial query. External-chat history stays
    // available through scoped tools and is never automatically replayed into a new turn.
    request.query = contents.join("\n\n");
    Ok(request)
}

fn history_context(
    binding: &CommunicationBindingRecord,
    conversation: &domain::CommunicationConversation,
) -> Value {
    json!({"conversation_id":conversation.id,"channel_id":binding.instance_id,
        "workspace_id":binding.workspace_id,"user_id":conversation.external_identity,
        "external_conversation_key":conversation.external_conversation_key})
}

fn binding_request(binding: &CommunicationBindingRecord) -> Result<NativeRunRequest> {
    let execution: NativeExecution = match &binding.config.reasoning_effort {
        Some(effort) => serde_json::from_value(
            json!({"model_parameters":{"reasoning":{"mode":"adaptive","effort":effort}}}),
        )?,
        None => NativeExecution::default(),
    };
    Ok(NativeRunRequest {
        query: String::new(),
        system: vec![NativePromptBlock::text(
            binding.config.system_prompt.clone(),
        )],
        model: binding.config.model.clone(),
        inputs: NativeObject::default(),
        history: Vec::new(),
        attachments: Vec::new(),
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
    // The model has already authored Markdown. Select that explicit content kind so it
    // remains byte-for-byte intact and does not inherit the command reply size budget.
    (!reply.trim().is_empty())
        .then(|| extension_contracts::CommunicationMessageContent::markdown(reply).into_markdown())
}
