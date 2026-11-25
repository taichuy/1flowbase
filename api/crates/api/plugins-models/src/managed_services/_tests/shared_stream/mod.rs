use super::{broker::*, cache};
use crate::{error_response::ApiError, managed_services::stream::StreamEvent};
use interface_runtime::UserPrincipal;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};
mod admission;
mod lifecycle;
mod snapshot_cache;

#[derive(Clone)]
struct Started {
    actor: uuid::Uuid,
    interval: u64,
    events: mpsc::Sender<StreamEvent>,
    finish: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
}
struct Fake {
    denied: Mutex<BTreeSet<uuid::Uuid>>,
    snapshot: Mutex<Option<(String, Value)>>,
    startup_event: Mutex<Option<Value>>,
    reads: AtomicUsize,
    starts: Mutex<Vec<Started>>,
    active: AtomicUsize,
    max_active: AtomicUsize,
    stopping: AtomicUsize,
    changed: watch::Sender<usize>,
    drain: watch::Sender<bool>,
}
impl Fake {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            denied: Mutex::default(),
            snapshot: Mutex::default(),
            startup_event: Mutex::default(),
            reads: AtomicUsize::new(0),
            starts: Mutex::default(),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            stopping: AtomicUsize::new(0),
            changed: watch::channel(0).0,
            drain: watch::channel(true).0,
        })
    }
    fn signal(&self) {
        self.changed.send_modify(|value| *value += 1);
    }
    async fn until(&self, predicate: impl Fn() -> bool) {
        let mut changed = self.changed.subscribe();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                changed.borrow_and_update();
                if predicate() {
                    break;
                }
                changed.changed().await.unwrap();
            }
        })
        .await
        .expect("finite broker state transition");
    }
    async fn started(&self, n: usize) -> Started {
        self.until(|| self.starts.lock().unwrap().len() >= n).await;
        self.starts.lock().unwrap()[n - 1].clone()
    }
    async fn event(&self, n: usize, value: Value) {
        let producer = self.started(n).await;
        let (consumed, received) = oneshot::channel();
        producer
            .events
            .send(StreamEvent(
                runtime_core::runtime_backend::RuntimeManagedDuplexEvent {
                    event: value,
                    consumed,
                },
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), received)
            .await
            .unwrap()
            .unwrap();
    }
}
#[async_trait::async_trait]
impl SourceBackend for Arc<Fake> {
    fn is_ready(&self, event: &Value) -> bool {
        event.get("type").and_then(Value::as_str) == Some("sample")
    }
    async fn authorize(
        &self,
        principal: &UserPrincipal,
        _: &Value,
    ) -> Result<UserPrincipal, ApiError> {
        if self
            .denied
            .lock()
            .unwrap()
            .contains(&principal.actor().user_id)
        {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "fixture_denied",
            )
            .into());
        }
        Ok(principal.clone())
    }
    async fn start(
        &self,
        principal: UserPrincipal,
        _: Value,
        interval: u64,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<Producer, ApiError> {
        let (events, receiver) = mpsc::channel(16);
        if let Some(event) = self.startup_event.lock().unwrap().clone() {
            let (consumed, _) = oneshot::channel();
            events
                .try_send(StreamEvent(
                    runtime_core::runtime_backend::RuntimeManagedDuplexEvent { event, consumed },
                ))
                .unwrap_or_else(|_| panic!("fresh producer channel accepts its first event"));
        }
        let (finish, finished) = oneshot::channel();
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        self.starts.lock().unwrap().push(Started {
            actor: principal.actor().user_id,
            interval,
            events,
            finish: Arc::new(Mutex::new(Some(finish))),
        });
        self.signal();
        let this = self.clone();
        let mut drain = self.drain.subscribe();
        Ok(Producer {
            events: receiver,
            completion: Box::pin(async move {
                let failed = if !*cancel.borrow() {
                    tokio::select! {_ = cancel.changed()=>false,result = finished=>result.unwrap_or(false)}
                } else {
                    false
                };
                this.stopping.fetch_add(1, Ordering::SeqCst);
                this.signal();
                while !*drain.borrow_and_update() {
                    drain.changed().await.unwrap();
                }
                this.active.fetch_sub(1, Ordering::SeqCst);
                this.signal();
                if failed {
                    Err(
                        control_plane::errors::ControlPlaneError::UpstreamUnavailable(
                            "fixture_worker_crashed",
                        )
                        .into(),
                    )
                } else {
                    Ok(())
                }
            }),
        })
    }
    async fn cache(&self, _: &str, _: u64, _: &Value) -> Result<(), ApiError> {
        Ok(())
    }
    async fn cached(&self, revision: &str) -> Result<Option<Value>, ApiError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .snapshot
            .lock()
            .unwrap()
            .as_ref()
            .filter(|(key, _)| key == revision)
            .map(|(_, event)| event.clone()))
    }
    async fn invalidate(&self, _: &str) -> Result<(), ApiError> {
        Ok(())
    }
}
fn key() -> SourceKey {
    SourceKey {
        installation: uuid::Uuid::now_v7(),
        generation: "generation-1".into(),
        interface: "fixture.source".into(),
        resource: "records".into(),
        record: "same-record".into(),
    }
}
fn member(interval: u64) -> Member {
    Member {
        principal: UserPrincipal::server_delegation(domain::ActorContext::scoped(
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
            "member",
            [],
        )),
        payload: json!({"body":{"record_id":"same-record"}}),
        interval,
    }
}
async fn add(key: SourceKey, fake: &Arc<Fake>, member: Member, revision: &str) -> Lease {
    join(key, Arc::new(fake.clone()), member, revision.to_owned())
        .await
        .unwrap()
}
