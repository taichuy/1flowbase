//! Host lifecycle for durable communication receive and execution. Provider credentials stay here.
use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{ensure, Context, Result};
use control_plane::ports::{AuthRepository, NotificationRepository, WorkspaceRepository};
use domain::{
    ActorContext, CommunicationBindingRecord, NotificationInstanceRecord,
    NotificationInstanceStatus, UserStatus,
};
use extension_contracts::{
    NotificationAuthorizationStatus, NotificationRuntimeInput, NotificationTypeDescriptor,
};
use serde_json::Value;
use tokio::{sync::watch, task::JoinSet};
use uuid::Uuid;

use crate::{app_state::ApiState, notification_runtime::ManagedNotificationProvider};

mod attachments;
mod execution;
mod receive;
mod request;
mod scoped_tools;

const LEASE_SECONDS: u32 = 120;
const RETRY_DELAY: Duration = Duration::from_secs(2);

pub(crate) struct CommunicationWorker {
    stop: watch::Sender<bool>,
    task: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}
impl CommunicationWorker {
    pub(crate) fn spawn(state: Arc<ApiState>) -> Self {
        let (stop, signal) = watch::channel(false);
        Self {
            stop,
            task: tokio::sync::Mutex::new(Some(tokio::spawn(supervise(state, signal)))),
        }
    }
    #[cfg(test)]
    pub(crate) fn inactive() -> Self {
        let (stop, _) = watch::channel(false);
        Self {
            stop,
            task: tokio::sync::Mutex::new(None),
        }
    }
    pub(crate) async fn stop(&self) {
        let _ = self.stop.send(true);
        if let Some(task) = self.task.lock().await.take() {
            let _ = task.await;
        }
    }
}

async fn supervise(state: Arc<ApiState>, mut stop: watch::Receiver<bool>) {
    let mut workers = HashMap::<(Uuid, Uuid), watch::Sender<bool>>::new();
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            _ = tokio::time::sleep(RETRY_DELAY) => {
                if let Ok(bindings) = state.notification_service.enabled_bound_channels().await {
                    workers.retain(|key, signal| {
                        let active = !signal.is_closed() && bindings.iter().any(|b| (b.workspace_id,b.instance_id)==*key);
                        if !active { let _ = signal.send(true); }
                        active
                    });
                    for binding in bindings {
                        let key = (binding.workspace_id, binding.instance_id);
                        if workers.contains_key(&key) { continue; }
                        let (signal, receiver) = watch::channel(false);
                        workers.insert(key, signal);
                        let state = state.clone();
                        tasks.spawn(async move {
                            tokio::join!(receive::run(state.clone(), key, receiver.clone()), execution::run(state, key, receiver));
                            key
                        });
                    }
                }
            }
            Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                if let Ok(key) = result {
                    if workers.get(&key).is_some_and(|sender| sender.is_closed()) { workers.remove(&key); }
                }
            }
        }
    }
    for signal in workers.values() {
        let _ = signal.send(true);
    }
    // Each child stops at its select boundary. RunStarted/SendStarted durable reservations
    // make interrupted external work non-replayable on restart.
    while tasks.join_next().await.is_some() {}
}

async fn active_binding(state: &ApiState, key: (Uuid, Uuid)) -> Result<CommunicationBindingRecord> {
    state
        .notification_service
        .enabled_bound_channels()
        .await?
        .into_iter()
        .find(|b| (b.workspace_id, b.instance_id) == key)
        .context("channel_disabled")
}

async fn actor(state: &ApiState, binding: &CommunicationBindingRecord) -> Result<ActorContext> {
    let user = state
        .store
        .find_user_by_id(binding.actor_user_id)
        .await?
        .context("actor_missing")?;
    ensure!(user.status == UserStatus::Active, "actor_disabled");
    let workspace = state
        .store
        .get_accessible_workspace(user.id, binding.workspace_id)
        .await?
        .context("workspace_access_denied")?;
    // Load live roles in the bound workspace. Never manufacture a root actor or use a user's
    // default workspace, which can differ from the channel's workspace.
    state
        .store
        .load_actor_context(user.id, workspace.tenant_id, workspace.id, None)
        .await
}

struct ProviderContext {
    actor: ActorContext,
    instance: NotificationInstanceRecord,
    descriptor: NotificationTypeDescriptor,
    config: Value,
    authorization: control_plane::ports::NotificationAuthorizationSnapshot,
    provider: ManagedNotificationProvider,
}
impl ProviderContext {
    async fn load(state: &ApiState, binding: &CommunicationBindingRecord) -> Result<Self> {
        let actor = actor(state, binding).await?;
        let instance = state
            .store
            .get_notification_instance(binding.workspace_id, binding.instance_id)
            .await?
            .context("channel_missing")?;
        ensure!(
            instance.status == NotificationInstanceStatus::Enabled,
            "channel_disabled"
        );
        let entry = state
            .notification_service
            .types(&actor)
            .await?
            .into_iter()
            .find(|entry| {
                entry.installation_id == instance.installation_id
                    && entry.descriptor.type_code == instance.type_code
            })
            .context("channel_type_unavailable")?;
        ensure!(
            entry.available && entry.descriptor.receive.is_some(),
            "receive_unavailable"
        );
        let mut config = instance
            .config_json
            .as_object()
            .cloned()
            .context("channel_config_invalid")?;
        let secret = state
            .store
            .resolve_notification_secret_config(
                binding.workspace_id,
                binding.instance_id,
                &state.provider_secret_master_key,
            )
            .await?;
        config.extend(
            secret
                .as_object()
                .cloned()
                .context("channel_config_invalid")?,
        );
        let config = Value::Object(config);
        entry.descriptor.validate_config(&config)?;
        let authorization = state
            .store
            .resolve_notification_authorization(
                binding.workspace_id,
                binding.instance_id,
                &state.provider_secret_master_key,
            )
            .await?;
        if entry.descriptor.authorization.is_some() {
            ensure!(
                authorization
                    .authorization
                    .as_ref()
                    .is_some_and(|v| v.status == NotificationAuthorizationStatus::Authorized),
                "authorization_required"
            );
        }
        Ok(Self {
            actor,
            instance,
            descriptor: entry.descriptor,
            config,
            authorization,
            provider: ManagedNotificationProvider::new(
                state.provider_runtime.managed_composition()?,
            ),
        })
    }
    async fn invoke(
        &self,
        input: NotificationRuntimeInput,
    ) -> Result<extension_contracts::NotificationRuntimeResult> {
        self.provider
            .execute_receive(
                &self.actor,
                self.instance
                    .installation_id
                    .context("installation_missing")?,
                &self.descriptor,
                self.config.clone(),
                input,
            )
            .await
    }
    fn input(
        &self,
        operation: extension_contracts::NotificationOperation,
    ) -> NotificationRuntimeInput {
        NotificationRuntimeInput {
            delivery_window: self
                .authorization
                .authorization
                .as_ref()
                .and_then(|v| v.delivery_window.clone()),
            operation,
            managed_secrets: self.authorization.managed_secrets.clone(),
        }
    }
}

#[cfg(test)]
#[path = "_tests/request.rs"]
mod request_tests;
