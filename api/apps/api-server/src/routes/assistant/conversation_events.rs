use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, atomic::Ordering, Mutex, MutexGuard},
};

use control_plane::application_public_api::run_service::AssistantConversationSummary;
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use tokio::sync::broadcast;
use utoipa::ToSchema;
use uuid::Uuid;

const ASSISTANT_CONVERSATION_EVENT_CAPACITY: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssistantConversationEventScope {
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub actor_user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AssistantConversationSummaryResponse {
    pub conversation_id: Option<Uuid>,
    pub legacy_flow_run_id: Option<Uuid>,
    pub latest_flow_run_id: Option<Uuid>,
    pub latest_flow_run_status: Option<String>,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<AssistantConversationSummary> for AssistantConversationSummaryResponse {
    fn from(summary: AssistantConversationSummary) -> Self {
        Self {
            conversation_id: summary.conversation_id,
            legacy_flow_run_id: summary.legacy_flow_run_id,
            latest_flow_run_id: summary.latest_flow_run_id,
            latest_flow_run_status: summary.latest_flow_run_status,
            title: summary.title,
            created_at: format_timestamp(summary.created_at),
            updated_at: format_timestamp(summary.updated_at),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantConversationEventKind {
    Created,
    Updated,
}

impl AssistantConversationEventKind {
    fn event_type(self) -> &'static str {
        match self {
            Self::Created => "conversation.created",
            Self::Updated => "conversation.updated",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantConversationEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub event_id: String,
    pub item: AssistantConversationSummaryResponse,
}

#[derive(Default)]
pub struct AssistantConversationEventHub {
    channels: Mutex<
        HashMap<AssistantConversationEventScope, broadcast::Sender<AssistantConversationEvent>>,
    >,
    next_sequence: AtomicU64,
}

impl AssistantConversationEventHub {
    pub fn subscribe(
        &self,
        scope: AssistantConversationEventScope,
    ) -> broadcast::Receiver<AssistantConversationEvent> {
        let mut channels = self.lock_channels();
        channels
            .entry(scope)
            .or_insert_with(|| broadcast::channel(ASSISTANT_CONVERSATION_EVENT_CAPACITY).0)
            .subscribe()
    }

    pub fn publish(
        &self,
        scope: AssistantConversationEventScope,
        kind: AssistantConversationEventKind,
        item: AssistantConversationSummaryResponse,
    ) {
        let sender = {
            let mut channels = self.lock_channels();
            channels
                .entry(scope)
                .or_insert_with(|| broadcast::channel(ASSISTANT_CONVERSATION_EVENT_CAPACITY).0)
                .clone()
        };
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = sender.send(AssistantConversationEvent {
            event_type: kind.event_type(),
            event_id: format!("assistant-conversation:{sequence}"),
            item,
        });
    }

    fn lock_channels(
        &self,
    ) -> MutexGuard<
        '_,
        HashMap<AssistantConversationEventScope, broadcast::Sender<AssistantConversationEvent>>,
    > {
        match self.channels.lock() {
            Ok(channels) => channels,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn format_timestamp(value: time::OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime must format as RFC3339")
}

#[cfg(test)]
#[path = "_tests/conversation_events.rs"]
mod tests;
