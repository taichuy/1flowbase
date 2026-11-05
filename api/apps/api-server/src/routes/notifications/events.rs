use crate::{app_state::ApiState, error_response::ApiError};
use axum::{
    extract::State,
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
};
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Default)]
pub struct NotificationEventHub {
    channels: Mutex<HashMap<(Uuid, Uuid), watch::Sender<()>>>,
}
struct NotificationSubscription {
    hub: Weak<NotificationEventHub>,
    key: (Uuid, Uuid),
    receiver: watch::Receiver<()>,
}
impl Drop for NotificationSubscription {
    fn drop(&mut self) {
        if let Some(hub) = self.hub.upgrade() {
            let mut channels = hub
                .channels
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if channels
                .get(&self.key)
                .is_some_and(|sender| sender.receiver_count() <= 1)
            {
                channels.remove(&self.key);
            }
        }
    }
}
impl NotificationEventHub {
    fn subscribe(self: &Arc<Self>, workspace: Uuid, user: Uuid) -> NotificationSubscription {
        let receiver = self
            .channels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry((workspace, user))
            .or_insert_with(|| watch::channel(()).0)
            .subscribe();
        NotificationSubscription {
            hub: Arc::downgrade(self),
            key: (workspace, user),
            receiver,
        }
    }
}
impl control_plane::notifications::NotificationEventPublisher for NotificationEventHub {
    fn publish(&self, workspace_id: Uuid, users: &[Uuid]) {
        let channels = self
            .channels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for user in users {
            if let Some(sender) = channels.get(&(workspace_id, *user)) {
                let _ = sender.send(());
            }
        }
    }
}
// Protocol control: this transport carries only an inbox-refresh signal and no business payload.
pub async fn events(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let context = crate::middleware::require_session::require_session(&state, &headers).await?;
    let receiver = state
        .notification_events
        .subscribe(context.actor.current_workspace_id, context.actor.user_id);
    let stream =
        futures_util::stream::unfold((receiver, true), |(mut receiver, initial)| async move {
            if !initial {
                match receiver.receiver.changed().await {
                    Ok(()) => {}
                    Err(_) => return None,
                }
            }
            Some((
                Ok::<_, Infallible>(Event::default().event("notification.changed").data("{}")),
                (receiver, false),
            ))
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/events.rs"]
mod tests;
