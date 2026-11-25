use super::super::{
    interface::{ManagedServiceOutput, Operation},
    stream::{StreamEvent, FRAME_BYTES},
    ManagedServiceRegistration,
};
use super::{
    broker::{self, Lease, Member, Notice},
    cache,
    host::{self, ManagedSource},
};
use crate::{
    app_state::ApiState, error_response::ApiError,
    routes::console_interface::ConsoleInterfaceTargetError,
};
use control_plane::errors::ControlPlaneError as Error;
use futures_util::{stream, StreamExt};
use interface_runtime::{
    InterfaceEventStream, InterfaceStreamPublisher, InterfaceStreamTerminal, UserPrincipal,
};
use plugin_framework::ManagedServiceOperation;
use runtime_core::runtime_backend::{RuntimeManagedDuplex, RuntimeManagedDuplexEvent};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tokio::sync::{oneshot, watch};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Desired {
    record_id: String,
    interval_ms: u64,
}
#[derive(Clone, Default)]
pub(super) struct ViewerNotice {
    sequence: u64,
    event: Option<Value>,
    error: Option<&'static str>,
}
type Latest = BTreeMap<String, ViewerNotice>;
pub(super) struct Viewer {
    seed: Option<Value>,
    lease: Arc<Lease>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
impl Viewer {
    pub(super) async fn stop(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
        self.lease.leave().await;
    }

    pub(super) fn take_events(&mut self, notice: &mut ViewerNotice) -> [Option<Value>; 2] {
        let seed = self.seed.take();
        // Sequence 1 is the broker's copy of this same seed, not a separate event.
        let event = notice
            .event
            .take()
            .filter(|_| seed.is_none() || notice.sequence != 1);
        let seed = seed.filter(|_| {
            notice.error.is_none() && self.lease.entry.notices.borrow().seed_replayable
        });
        [seed, event]
    }
}
fn desired(value: &Value) -> Result<Vec<Desired>, ApiError> {
    let records: Vec<Desired> = serde_json::from_value(value.clone())
        .map_err(|_| Error::InvalidInput("managed_service_records"))?;
    let mut ids = BTreeSet::new();
    for record in &records {
        if record.record_id.is_empty() || !ids.insert(&record.record_id) {
            return Err(Error::InvalidInput("managed_service_records").into());
        }
        cache::ttl_ms(record.interval_ms)?;
    }
    Ok(records)
}
fn notify(latest: &watch::Sender<Latest>, record: &str, notice: &Notice) {
    latest.send_modify(|values| {
        values.insert(
            record.to_owned(),
            ViewerNotice {
                sequence: notice.sequence,
                event: notice.event.clone(),
                error: notice.error,
            },
        );
    });
}
fn denied(latest: &watch::Sender<Latest>, record: &str) {
    latest.send_modify(|values| {
        let next = values.get(record).map_or(1, |v| v.sequence.wrapping_add(1));
        values.insert(
            record.to_owned(),
            ViewerNotice {
                sequence: next,
                event: None,
                error: Some("managed_service_subscription_denied"),
            },
        );
    });
}
pub(super) fn viewer(mut lease: Lease, record: String, latest: watch::Sender<Latest>) -> Viewer {
    let seed = lease.seed.take();
    let lease = Arc::new(lease);
    let (stop, mut stopping) = watch::channel(false);
    let observer = lease.clone();
    let mut notices = lease.entry.notices.subscribe();
    let initial = notices.borrow_and_update().clone();
    if initial.sequence > 0 {
        notify(&latest, &record, &initial);
    }
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = stopping.changed() => break,
                result = notices.changed() => if result.is_err() { break; },
            }
            let notice = notices.borrow_and_update().clone();
            let authorized = tokio::select! {
                _ = stopping.changed() => break,
                result = observer.authorize() => result.is_ok(),
            };
            if !authorized {
                observer.revoke();
                denied(&latest, &record);
                break;
            }
            notify(&latest, &record, &notice);
        }
    });
    Viewer {
        seed,
        lease,
        stop,
        task,
    }
}
struct Session {
    state: Arc<ApiState>,
    registration: Arc<ManagedServiceRegistration>,
    principal: UserPrincipal,
    operation: ManagedServiceOperation,
    source: Arc<ManagedServiceOperation>,
    aggregate: bool,
    payload: Value,
    viewers: BTreeMap<String, Viewer>,
    latest: watch::Sender<Latest>,
}
impl Session {
    async fn authorize_outer(&self) -> Result<(), ApiError> {
        let principal = super::authority::live_principal(&self.state, &self.principal).await?;
        if !super::super::record_acl::operation_allowed(
            &self.state,
            principal.actor(),
            &self.operation,
        )
        .await?
        {
            return Err(Error::PermissionDenied("console_operation_permission_denied").into());
        }
        Ok(())
    }
    fn candidate(&self, record: &Desired) -> Result<(ManagedSource, Value), ApiError> {
        let mut payload = if self.aggregate {
            host::record_payload(&self.source, &record.record_id, record.interval_ms)?
        } else {
            let mut payload = self.payload.clone();
            let interval = &self
                .source
                .stream
                .as_ref()
                .and_then(|s| s.shared_source.as_ref())
                .ok_or(Error::Conflict("managed_service_shared_source"))?
                .interval_pointer;
            host::set_pointer(&mut payload, interval, json!(record.interval_ms))?;
            payload
        };
        let source = ManagedSource::new(
            self.state.clone(),
            self.registration.clone(),
            self.source.clone(),
            &self.principal,
            &payload,
        )?;
        host::set_pointer(
            &mut payload,
            host::record_pointer(&self.source)?,
            json!(source.key.record),
        )?;
        Ok((source, payload))
    }

    async fn replace(
        &mut self,
        records: Vec<Desired>,
        sent: &mut BTreeMap<String, u64>,
    ) -> Admissions {
        let mut keep = BTreeSet::new();
        let mut invalid = Vec::new();
        let mut candidates = Vec::new();
        // UUID aliases share one Viewer; the last requested interval wins.
        for record in records.into_iter().rev() {
            match self.candidate(&record) {
                Ok((source, payload)) if keep.insert(source.key.record.clone()) => {
                    let previous = self
                        .viewers
                        .get(&source.key.record)
                        .map(|v| v.lease.clone());
                    candidates.push((source, payload, record.interval_ms, previous));
                }
                Ok(_) => {}
                Err(_) => invalid.push(record.record_id),
            }
        }
        candidates.reverse();
        let removed = std::mem::take(&mut self.viewers)
            .into_iter()
            .filter_map(|(id, viewer)| {
                if keep.contains(&id) {
                    self.viewers.insert(id, viewer);
                    None
                } else {
                    Some(viewer.stop())
                }
            });
        futures_util::future::join_all(removed).await;
        self.latest
            .send_modify(|values| values.retain(|id, _| keep.contains(id)));
        sent.retain(|id, _| keep.contains(id));
        for id in invalid {
            denied(&self.latest, &id);
        }
        let principal = self.principal.clone();
        concurrent_admissions(candidates.into_iter().map(
            move |(source, payload, interval, previous)| {
                let principal = principal.clone();
                async move {
                    let id = source.key.record.clone();
                    let result = async {
                        let _inspection = broker::inspection(&source.key).lock_owned().await;
                        let principal = source.admit(&principal, &payload).await?;
                        let revision = source.resolve(&principal, &payload).await?;
                        let member = Member {
                            principal,
                            payload,
                            interval,
                        };
                        if previous
                            .is_some_and(|lease| lease.update(member.clone(), revision.clone()))
                        {
                            return Ok(None);
                        }
                        broker::join_admitted(
                            source.key.clone(),
                            Arc::new(source),
                            member,
                            revision,
                        )
                        .await
                        .map(Some)
                    }
                    .await;
                    (id, result)
                }
            },
        ))
        .boxed()
    }

    async fn attach(
        &mut self,
        id: String,
        admitted: Result<Option<Lease>, ApiError>,
        sent: &mut BTreeMap<String, u64>,
    ) {
        if matches!(admitted, Ok(None)) {
            return;
        }
        if let Some(old) = self.viewers.remove(&id) {
            old.stop().await;
        }
        self.latest.send_modify(|values| {
            values.remove(&id);
        });
        sent.remove(&id);
        match admitted {
            Ok(Some(lease)) => {
                self.viewers
                    .insert(id.clone(), viewer(lease, id, self.latest.clone()));
            }
            Err(_) => denied(&self.latest, &id),
            Ok(None) => unreachable!(),
        }
    }

    async fn stop(&mut self) {
        futures_util::future::join_all(
            std::mem::take(&mut self.viewers)
                .into_values()
                .map(Viewer::stop),
        )
        .await;
    }
    fn initial(&self) -> Result<Vec<Desired>, ApiError> {
        let records = if self.aggregate {
            desired(
                self.payload
                    .pointer("/body/records")
                    .ok_or(Error::InvalidInput("managed_service_records"))?,
            )?
        } else {
            vec![Desired {
                record_id: self
                    .payload
                    .pointer(host::record_pointer(&self.source)?)
                    .and_then(Value::as_str)
                    .ok_or(Error::InvalidInput("record_id"))?
                    .to_owned(),
                interval_ms: host::interval(&self.source, &self.payload)?,
            }]
        };
        Ok(records)
    }
    async fn set(&self, value: Value) -> Result<Option<Vec<Desired>>, ApiError> {
        self.authorize_outer().await?;
        match (self.aggregate, value.get("type").and_then(Value::as_str)) {
            (true, Some("set")) => Ok(Some(desired(
                value
                    .get("records")
                    .ok_or(Error::InvalidInput("managed_service_records"))?,
            )?)),
            (false, Some("close")) => Ok(None),
            (true, _) => Err(Error::InvalidInput("managed_service_set").into()),
            (false, _) => Err(Error::InvalidInput("managed_service_stream_input").into()),
        }
    }
}
type Admissions = stream::BoxStream<'static, (String, Result<Option<Lease>, ApiError>)>;

pub(super) fn concurrent_admissions<F: std::future::Future>(
    admissions: impl IntoIterator<Item = F>,
) -> impl futures_util::Stream<Item = F::Output> {
    // This limits only in-flight work, never the subscription's record count.
    stream::iter(admissions).buffer_unordered(16)
}

async fn emit(
    publisher: &InterfaceStreamPublisher<
        StreamEvent,
        ManagedServiceOutput,
        ConsoleInterfaceTargetError,
    >,
    validator: &jsonschema::Validator,
    value: Value,
    cancel: &mut watch::Receiver<bool>,
) -> bool {
    if !validator.is_valid(&value)
        || serde_json::to_vec(&value).map_or(true, |bytes| bytes.len() > FRAME_BYTES - 128)
    {
        return false;
    }
    let (consumed, _receiver) = oneshot::channel();
    tokio::select! {
        _ = cancel.changed() => false,
        result = publisher.emit(StreamEvent(RuntimeManagedDuplexEvent {event:value,consumed})) => result.is_ok(),
    }
}
pub(in crate::managed_services) async fn subscribe(
    state: Arc<ApiState>,
    registration: ManagedServiceRegistration,
    principal: UserPrincipal,
    compiled: Arc<Operation>,
    payload: Value,
    duplex: RuntimeManagedDuplex,
    mut cancel: watch::Receiver<bool>,
) -> Result<
    InterfaceEventStream<StreamEvent, ManagedServiceOutput, ConsoleInterfaceTargetError>,
    ApiError,
> {
    if principal.authenticated_session().is_none() {
        return Err(Error::PermissionDenied("cookie_session_required").into());
    }
    let operation = compiled.declaration.clone();
    let aggregate_id = operation
        .stream
        .as_ref()
        .and_then(|s| s.aggregate_source_interface_id.as_deref());
    let aggregate = aggregate_id.is_some();
    let source = host::operation(
        &registration,
        aggregate_id.unwrap_or(&operation.interface_id),
    )?;
    let event_validator = compiled
        .event
        .clone()
        .ok_or(Error::InvalidInput("managed_service_stream"))?;
    let (publisher, stream) = interface_runtime::interface_stream_channel(16);
    let latest = watch::channel(Latest::new()).0;
    let mut changes = latest.subscribe();
    let mut session = Session {
        state,
        registration: Arc::new(registration),
        principal,
        operation,
        source: Arc::new(source),
        aggregate,
        payload,
        viewers: BTreeMap::new(),
        latest,
    };
    tokio::spawn(async move {
        let mut sent = BTreeMap::new();
        let mut close_requested = false;
        if let Ok(initial) = session.initial() {
            let mut admissions = session.replace(initial, &mut sent).await.fuse();
            'session: loop {
                let input = async { duplex.input.lock().await.recv().await };
                tokio::select! {
                    _ = cancel.changed() => break,
                    value = input => {
                        let Some(value) = value else { break; };
                        // Dropping this generation cancels pending futures and drops any
                        // undispatched leases before the new membership is installed.
                        drop(admissions);
                        let records = tokio::select! {
                            _ = cancel.changed() => break,
                            result = session.set(value) => result,
                        };
                        match records {
                            Ok(Some(records)) => admissions = session.replace(records, &mut sent).await.fuse(),
                            Ok(None) => { close_requested = true; break; }
                            Err(_) => break,
                        }
                    },
                    result = changes.changed() => {
                        if result.is_err() { break; }
                        let values = changes.borrow_and_update().clone();
                        if tokio::select! { _ = cancel.changed()=>true, result=session.authorize_outer()=>result.is_err() } { break; }
                        for (record, mut notice) in values {
                            if sent.get(&record) == Some(&notice.sequence) { continue; }
                            sent.insert(record.clone(), notice.sequence);
                            let [seed, event] = match session.viewers.get_mut(&record) {
                                Some(viewer) => viewer.take_events(&mut notice),
                                None => [None, notice.event.take()],
                            };
                            let project = |event| {
                                if aggregate { json!({"type":"source","record_id":record,"event":event}) } else { event }
                            };
                            if let Some(seed) = seed {
                                // A replay waiting for channel capacity must still yield to a
                                // newer ready event, revision or error before it is enqueued.
                                let mut source = session.viewers[&record].lease.entry.notices.subscribe();
                                let accepted = tokio::select! {
                                    biased;
                                    _ = source.wait_for(|notice| !notice.seed_replayable) => true,
                                    result = emit(&publisher, &event_validator, project(seed), &mut cancel) => result,
                                };
                                if !accepted { break 'session; }
                            }
                            if let Some(event) = event {
                                if !emit(&publisher, &event_validator, project(event), &mut cancel).await { break 'session; }
                            }
                            if let Some(error) = notice.error {
                                if !aggregate {
                                    if error == "managed_service_source_retrying" { continue; }
                                    break 'session;
                                }
                                let event = json!({"type":"subscription_error","record_id":record,"error_code":error});
                                if !emit(&publisher, &event_validator, event, &mut cancel).await { break 'session; }
                                if error == "managed_service_subscription_denied" {
                                    if let Some(viewer) = session.viewers.remove(&record) { viewer.stop().await; }
                                }
                            }
                        }
                    },
                    Some((id, admitted)) = admissions.next() => session.attach(id, admitted, &mut sent).await,
                }
            }
        }
        session.stop().await;
        let close_output = session
            .source
            .stream
            .as_ref()
            .and_then(|stream| stream.shared_source.as_ref())
            .and_then(|source| source.close_output.clone());
        let terminal = if close_requested {
            close_output
                .map(|output| {
                    InterfaceStreamTerminal::Completed(ManagedServiceOutput(
                        output,
                        super::super::event_projection::OutputPolicy::BusinessResult(Arc::new(
                            super::super::event_projection::OperationPolicy::new(
                                &session.operation,
                            ),
                        )),
                    ))
                })
                .unwrap_or(InterfaceStreamTerminal::Cancelled)
        } else {
            InterfaceStreamTerminal::Cancelled
        };
        let _ = publisher.finish(terminal).await;
    });
    Ok(stream)
}
