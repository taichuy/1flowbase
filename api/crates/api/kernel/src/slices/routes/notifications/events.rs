use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::watch;
use uuid::Uuid;

#[derive(Default)]
pub struct NotificationEventHub {
    channels: Mutex<HashMap<(Uuid, Uuid), watch::Sender<()>>>,
}
pub struct NotificationSubscription {
    hub: Weak<NotificationEventHub>,
    key: (Uuid, Uuid),
    pub receiver: watch::Receiver<()>,
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
    pub fn subscribe(self: &Arc<Self>, workspace: Uuid, user: Uuid) -> NotificationSubscription {
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

#[cfg(test)]
#[path = "_tests/events.rs"]
mod tests;
