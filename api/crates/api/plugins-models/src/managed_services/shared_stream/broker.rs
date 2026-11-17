use super::super::stream::StreamEvent;
use crate::error_response::ApiError;
use interface_runtime::UserPrincipal;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock, Weak},
};
use tokio::sync::{mpsc, watch, Mutex as AsyncMutex};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct SourceKey {
    pub installation: uuid::Uuid,
    pub generation: String,
    pub interface: String,
    pub resource: String,
    pub record: String,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SerialKey(uuid::Uuid, String, String, String);
impl SourceKey {
    fn serial(&self) -> SerialKey {
        SerialKey(
            self.installation,
            self.interface.clone(),
            self.resource.clone(),
            self.record.clone(),
        )
    }
}
pub(super) struct Producer {
    pub events: mpsc::Receiver<StreamEvent>,
    pub completion: Pin<Box<dyn Future<Output = Result<(), ApiError>> + Send>>,
}
#[async_trait::async_trait]
pub(super) trait SourceBackend: Send + Sync {
    fn ready(&self, event: &Value) -> bool;
    async fn authorize(
        &self,
        principal: &UserPrincipal,
        payload: &Value,
    ) -> Result<UserPrincipal, ApiError>;
    async fn start(
        &self,
        principal: UserPrincipal,
        payload: Value,
        interval: u64,
        cancel: watch::Receiver<bool>,
    ) -> Result<Producer, ApiError>;
    async fn cache(&self, revision: &str, interval: u64, event: &Value) -> Result<(), ApiError>;
    async fn invalidate(&self, revision: &str) -> Result<(), ApiError>;
}
#[derive(Clone)]
pub(super) struct Member {
    pub principal: UserPrincipal,
    pub payload: Value,
    pub interval: u64,
}
#[derive(Clone, Default)]
pub(super) struct Notice {
    pub sequence: u64,
    pub revision: String,
    pub event: Option<Value>,
    pub error: Option<&'static str>,
}
struct Core {
    members: BTreeMap<uuid::Uuid, Member>,
    revision: String,
    retired: bool,
}
pub(super) struct Entry {
    pub key: SourceKey,
    backend: Arc<dyn SourceBackend>,
    core: Mutex<Core>,
    signal: watch::Sender<u64>,
    pub notices: watch::Sender<Notice>,
    finished: watch::Sender<bool>,
    serial: Arc<AsyncMutex<()>>,
}
#[derive(Default)]
struct Registry {
    entries: BTreeMap<SourceKey, Arc<Entry>>,
    serial: BTreeMap<SerialKey, Weak<AsyncMutex<()>>>,
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}
fn serial_inner(all: &mut Registry, key: &SourceKey) -> Arc<AsyncMutex<()>> {
    all.serial.retain(|_, value| value.strong_count() > 0);
    if let Some(value) = all.serial.get(&key.serial()).and_then(Weak::upgrade) {
        return value;
    }
    let value = Arc::new(AsyncMutex::new(()));
    all.serial.insert(key.serial(), Arc::downgrade(&value));
    value
}
pub(super) fn serial(key: &SourceKey) -> Arc<AsyncMutex<()>> {
    serial_inner(
        &mut registry().lock().unwrap_or_else(|e| e.into_inner()),
        key,
    )
}
pub(super) fn active(key: &SourceKey) -> Option<Arc<Entry>> {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entries
        .get(key)
        .filter(|entry| {
            !entry
                .core
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .members
                .is_empty()
        })
        .cloned()
}
pub(in crate::managed_services) fn retire(installation: Option<uuid::Uuid>) {
    let all = registry().lock().unwrap_or_else(|e| e.into_inner());
    for entry in all
        .entries
        .values()
        .filter(|entry| installation.is_none_or(|id| entry.key.installation == id))
    {
        entry.core.lock().unwrap_or_else(|e| e.into_inner()).retired = true;
        entry.wake();
    }
}
pub(in crate::managed_services) async fn revoke_record_permissions(
    installation: uuid::Uuid,
    key: &domain::RecordPermissionKey,
) {
    let entries = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entries
        .values()
        .filter(|entry| {
            entry.key.installation == installation
                && entry.key.resource == key.resource.resource_code
                && entry.key.record == key.record_id
        })
        .cloned()
        .collect::<Vec<_>>();
    for entry in entries {
        let members = entry
            .core
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .members
            .iter()
            .filter(|(_, member)| {
                member.principal.actor().current_workspace_id == key.workspace_id
                    && !member.principal.actor().is_root
            })
            .map(|(id, member)| (*id, member.clone()))
            .collect::<Vec<_>>();
        for (id, member) in members {
            if entry
                .backend
                .authorize(&member.principal, &member.payload)
                .await
                .is_err()
            {
                entry.remove(id);
            }
        }
    }
}
pub(super) struct Lease {
    pub entry: Arc<Entry>,
    pub id: uuid::Uuid,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.entry.remove(self.id);
    }
}
impl Lease {
    pub async fn leave(&self) {
        self.entry.remove(self.id);
        let mut finished = self.entry.finished.subscribe();
        let mut changes = self.entry.signal.subscribe();
        loop {
            changes.borrow_and_update();
            if *finished.borrow_and_update() || self.entry.configuration().is_some() {
                break;
            }
            tokio::select! {_ = finished.changed()=>{},_ = changes.changed()=>{}}
        }
    }
    pub fn revoke(&self) {
        self.entry.remove(self.id);
    }
    pub fn update(&self, member: Member, revision: String) -> bool {
        {
            let mut core = self.entry.core.lock().unwrap_or_else(|e| e.into_inner());
            if core.retired || !core.members.contains_key(&self.id) {
                return false;
            }
            core.members.insert(self.id, member);
        }
        self.entry.refresh_revision(&revision);
        self.entry.wake();
        true
    }
    pub async fn authorize(&self) -> Result<UserPrincipal, ApiError> {
        let member = {
            let core = self.entry.core.lock().unwrap_or_else(|e| e.into_inner());
            if core.retired {
                None
            } else {
                core.members.get(&self.id).cloned()
            }
        }
        .ok_or(control_plane::errors::ControlPlaneError::PermissionDenied(
            "managed_service_subscription_revoked",
        ))?;
        self.entry
            .backend
            .authorize(&member.principal, &member.payload)
            .await
    }
}
pub(super) async fn join(
    key: SourceKey,
    backend: Arc<dyn SourceBackend>,
    mut member: Member,
    revision: String,
) -> Result<Lease, ApiError> {
    super::cache::ttl_ms(member.interval)?;
    member.principal = backend
        .authorize(&member.principal, &member.payload)
        .await?;
    let id = uuid::Uuid::now_v7();
    let (entry, created) = {
        let mut all = registry().lock().unwrap_or_else(|e| e.into_inner());
        let (entry, created) = if let Some(entry) = all.entries.get(&key) {
            (entry.clone(), false)
        } else {
            let entry = Arc::new(Entry {
                serial: serial_inner(&mut all, &key),
                key: key.clone(),
                backend,
                core: Mutex::new(Core {
                    members: BTreeMap::new(),
                    revision: revision.clone(),
                    retired: false,
                }),
                signal: watch::channel(0).0,
                notices: watch::channel(Notice::default()).0,
                finished: watch::channel(false).0,
            });
            all.entries.insert(key, entry.clone());
            (entry, true)
        };
        {
            let mut core = entry.core.lock().unwrap_or_else(|e| e.into_inner());
            if core.retired {
                return Err(control_plane::errors::ControlPlaneError::Conflict(
                    "managed_service_generation_mismatch",
                )
                .into());
            }
            if core.revision != revision {
                core.revision = revision;
                entry.notices.send_modify(|notice| {
                    notice.sequence += 1;
                    notice.event = None;
                    notice.error = None;
                    notice.revision = core.revision.clone();
                });
            }
            core.members.insert(id, member);
        }
        (entry, created)
    };
    entry.wake();
    if created {
        tokio::spawn(entry.clone().drive());
    }
    Ok(Lease { entry, id })
}
#[derive(Clone, PartialEq, Eq)]
struct Configuration {
    owner: uuid::Uuid,
    revision: String,
    interval: u64,
}
impl Entry {
    pub fn refresh_revision(&self, revision: &str) {
        let mut core = self.core.lock().unwrap_or_else(|e| e.into_inner());
        if core.revision != revision {
            core.revision = revision.to_owned();
            self.notices.send_modify(|notice| {
                notice.sequence += 1;
                notice.revision = revision.to_owned();
                notice.event = None;
                notice.error = None;
            });
            self.wake();
        }
    }
    fn wake(&self) {
        self.signal
            .send_modify(|version| *version = version.wrapping_add(1));
    }
    fn remove(&self, id: uuid::Uuid) {
        self.core
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .members
            .remove(&id);
        self.wake();
        self.notices.send_modify(|_| {});
    }
    fn configuration(&self) -> Option<(Configuration, Member)> {
        let core = self.core.lock().unwrap_or_else(|e| e.into_inner());
        if core.retired {
            return None;
        }
        let (owner, member) = core.members.first_key_value()?;
        Some((
            Configuration {
                owner: *owner,
                revision: core.revision.clone(),
                interval: core.members.values().map(|m| m.interval).min()?,
            },
            member.clone(),
        ))
    }
    fn publish(&self, revision: &str, event: Option<Value>, error: Option<&'static str>) {
        self.notices.send_modify(|notice| {
            notice.sequence += 1;
            notice.revision = revision.to_owned();
            notice.event = event;
            notice.error = error;
        });
    }
    /// Registry and member locks always follow this order, making last-leave and
    /// a concurrent new join atomic. The serial permit stays owned through cleanup.
    fn remove_if_unused(self: &Arc<Self>) -> bool {
        let mut all = registry().lock().unwrap_or_else(|e| e.into_inner());
        let core = self.core.lock().unwrap_or_else(|e| e.into_inner());
        if !core.retired && !core.members.is_empty() {
            return false;
        }
        if all
            .entries
            .get(&self.key)
            .is_some_and(|entry| Arc::ptr_eq(entry, self))
        {
            all.entries.remove(&self.key);
        }
        true
    }
    async fn drive(self: Arc<Self>) {
        let mut signal = self.signal.subscribe();
        // Also serializes old-generation drain and a legacy unary snapshot miss.
        let _serial = self.serial.clone().lock_owned().await;
        let mut failures = 0u32;
        loop {
            signal.borrow_and_update();
            let Some((configuration, member)) = self.configuration() else {
                if self.remove_if_unused() {
                    self.publish("", None, Some("managed_service_source_stopped"));
                    self.finished.send_replace(true);
                    return;
                }
                continue;
            };
            let principal = match self
                .backend
                .authorize(&member.principal, &member.payload)
                .await
            {
                Ok(principal) => principal,
                Err(_) => {
                    self.remove(configuration.owner);
                    continue;
                }
            };
            let (cancel, cancellation) = watch::channel(false);
            let producer = self
                .backend
                .start(
                    principal,
                    member.payload.clone(),
                    configuration.interval,
                    cancellation,
                )
                .await;
            let mut terminal = false;
            let mut retry = false;
            match producer {
                Ok(Producer {
                    mut events,
                    mut completion,
                }) => {
                    let mut completion_done = false;
                    let mut validation = tokio::time::interval(std::time::Duration::from_millis(
                        configuration.interval.min(30_000),
                    ));
                    validation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    validation.tick().await;
                    loop {
                        tokio::select! {
                            biased;
                            _ = signal.changed() => {
                                if self.configuration().map(|v| v.0) != Some(configuration.clone()) { break; }
                            }
                            _ = validation.tick() => {
                                if self.backend.authorize(&member.principal, &member.payload).await.is_err() { self.remove(configuration.owner); break; }
                            }
                            outcome = &mut completion => {
                                completion_done = true;
                                terminal = outcome.is_ok();
                                retry = outcome.is_err();
                                // Drain already accepted raw events before declaring a terminal.
                                while let Ok(StreamEvent(event)) = events.try_recv() {
                                    if self.backend.authorize(&member.principal, &member.payload).await.is_ok() {
                                        let _ = self.backend.cache(&configuration.revision, configuration.interval, &event.event).await;
                                        self.publish(&configuration.revision, Some(event.event), None);
                                        let _ = event.consumed.send(());
                                    }
                                }
                                break;
                            }
                            event = events.recv() => match event {
                                Some(StreamEvent(event)) => {
                                    failures=0;
                                    if self.backend.authorize(&member.principal, &member.payload).await.is_err() {
                                        self.remove(configuration.owner); break;
                                    }
                                    let _ = self.backend.cache(&configuration.revision, configuration.interval, &event.event).await;
                                    self.publish(&configuration.revision, Some(event.event), None);
                                    // Producer credit means Host acceptance, independent of each browser ACK.
                                    let _ = event.consumed.send(());
                                }
                                None => { terminal = true; break; }
                            }
                        }
                    }
                    let _ = cancel.send(true);
                    // A polled-complete future must never be polled again. A closed
                    // event channel alone never releases the sole completion owner.
                    if !completion_done {
                        let mut events_open = true;
                        loop {
                            tokio::select! {
                                outcome = &mut completion => {
                                    if terminal {terminal=outcome.is_ok();retry=outcome.is_err();}
                                    break;
                                },
                                event = events.recv(), if events_open => match event {
                                    Some(StreamEvent(event)) => {let _ = event.consumed.send(());},
                                    None => events_open = false,
                                }
                            }
                        }
                    }
                    drop(events);
                }
                Err(_) => {
                    retry = true;
                }
            }
            if retry {
                let _ = self.backend.invalidate(&configuration.revision).await;
                failures = failures.saturating_add(1);
                self.notices.send_modify(|notice| {
                    notice.sequence += 1;
                    notice.revision = configuration.revision.clone();
                    notice.error = Some("managed_service_source_retrying");
                    notice.event = None;
                });
                if self.configuration().map(|v| v.0) == Some(configuration.clone()) {
                    let base = 250u64
                        .saturating_mul(1u64 << failures.saturating_sub(1).min(7))
                        .min(30_000);
                    let delay = base / 2 + rand::random::<u64>() % (base + 1);
                    tokio::select! {_ = signal.changed()=>{},_ = tokio::time::sleep(std::time::Duration::from_millis(delay))=>{}}
                }
                continue;
            }
            if terminal {
                let _ = self.backend.invalidate(&configuration.revision).await;
                self.notices.send_modify(|notice| {
                    notice.sequence += 1;
                    notice.revision = configuration.revision.clone();
                    notice.error = Some("managed_service_source_stopped");
                    if notice
                        .event
                        .as_ref()
                        .is_some_and(|event| self.backend.ready(event))
                    {
                        notice.event = None;
                    }
                });
                // Terminal source/config failures are not blindly restarted. A new
                // desired set, revision, owner or cadence wakes the single driver.
                if self.configuration().map(|v| v.0) == Some(configuration) {
                    let _ = signal.changed().await;
                }
            }
        }
    }
}
