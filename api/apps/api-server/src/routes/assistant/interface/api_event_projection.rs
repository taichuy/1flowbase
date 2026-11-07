use super::*;
use crate::event_center::api_projection::{
    self as event, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use serde_json::{json, Value};
use std::sync::Arc;

impl ApiEventInput for AssistantSettingsInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::Get => json!({}),
            Self::Update(body) => json!({"body":event::to_value(body)?}),
        })
    }
}
impl ApiEventOutput for AssistantSettingsOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::Settings(value) => event::to_value(value),
        }
    }
}
impl ApiEventInput for AssistantConversationsInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::GetRunActivity { flow_run_id, query } => {
                json!({"path":{"flow_run_id":flow_run_id},"query":event::to_value(query)?})
            }
            Self::CreateConversation(body) => json!({"body":event::to_value(body)?}),
            Self::ListConversations(query) => json!({"query":event::to_value(query)?}),
            Self::GetConversationMessages {
                conversation_id,
                query,
            } => {
                json!({"path":{"conversation_id":conversation_id},"query":event::to_value(query)?})
            }
            Self::GetLegacySnapshotMessages { flow_run_id, query } => {
                json!({"path":{"flow_run_id":flow_run_id},"query":event::to_value(query)?})
            }
        })
    }
}
impl ApiEventOutput for AssistantConversationsOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::RunActivity(value) => event::to_value(value),
            Self::Conversation(value) => event::to_value(value),
            Self::ConversationPage(value) => event::to_value(value),
            Self::ConversationMessages(value) => event::to_value(value),
        }
    }
}
impl ApiEventInput for AssistantRunInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(json!({"body":event::to_value(&self.body)?}))
    }
}
impl ApiEventOutput for AssistantRunOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        event::to_value(&self.0)
    }
}
impl ApiEventOutput for AssistantRunStreamOutput {
    fn document_output_unavailable(
        _: &ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        Some(domain::event_center::ApiEventUnavailableReason::MissingStreamingTerminal)
    }
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Err(Arc::from(
            "Assistant stream terminal has no declared business result",
        ))
    }
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
