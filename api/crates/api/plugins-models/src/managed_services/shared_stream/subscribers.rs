use super::super::{
    interface::ManagedServiceOutput,
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
struct ViewerNotice {
    sequence: u64,
    event: Option<Value>,
    error: Option<&'static str>,
}
type Latest = BTreeMap<String, ViewerNotice>;
struct Viewer {
    lease: Arc<Lease>,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
impl Viewer {
    async fn stop(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
        self.lease.leave().await;
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
fn viewer(lease: Lease, record: String, latest: watch::Sender<Latest>) -> Viewer {
    let lease = Arc::new(lease);
    let (stop, mut stopping) = watch::channel(false);
    let observer = lease.clone();
    let mut notices = lease.entry.notices.subscribe();
    let task = tokio::spawn(async move {
        loop {
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
            if notice.sequence > 0 {
                notify(&latest, &record, &notice);
            }
            tokio::select! {
                _ = stopping.changed() => break,
                result = notices.changed() => if result.is_err() { break; },
            }
        }
    });
    Viewer { lease, stop, task }
}
struct Session {
    state: Arc<ApiState>,
    registration: ManagedServiceRegistration,
    principal: UserPrincipal,
    operation: ManagedServiceOperation,
    source: ManagedServiceOperation,
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
    async fn replace(&mut self, records: Vec<Desired>) {
        let mut keep = BTreeSet::new();
        for record in records {
            let admitted = async {
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
                let source = Arc::new(ManagedSource::new(
                    self.state.clone(),
                    self.registration.clone(),
                    self.source.clone(),
                    &self.principal,
                    &payload,
                )?);
                host::set_pointer(
                    &mut payload,
                    host::record_pointer(&self.source)?,
                    json!(source.key.record),
                )?;
                let _inspection = broker::inspection(&source.key).lock_owned().await;
                let principal = source.admit(&self.principal, &payload).await?;
                let revision = source.resolve(&principal, &payload).await?;
                let record_id = source.key.record.clone();
                let member = Member {
                    principal,
                    payload,
                    interval: record.interval_ms,
                };
                let updated = self
                    .viewers
                    .get(&record_id)
                    .is_some_and(|viewer| viewer.lease.update(member.clone(), revision.clone()));
                if !updated {
                    if let Some(old) = self.viewers.remove(&record_id) {
                        old.stop().await;
                    }
                    let lease = broker::join(source.key.clone(), source, member, revision).await?;
                    self.viewers.insert(
                        record_id.clone(),
                        viewer(lease, record_id.clone(), self.latest.clone()),
                    );
                }
                Ok::<_, ApiError>(record_id)
            }
            .await;
            match admitted {
                Ok(record) => {
                    keep.insert(record);
                }
                Err(_) => {
                    denied(&self.latest, &record.record_id);
                }
            }
        }
        let remove = self
            .viewers
            .keys()
            .filter(|id| !keep.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        for id in remove {
            if let Some(viewer) = self.viewers.remove(&id) {
                viewer.stop().await;
            }
            self.latest.send_modify(|values| {
                if values.get(&id).is_some_and(|notice| notice.error.is_none()) {
                    values.remove(&id);
                }
            });
        }
    }
    async fn stop(&mut self) {
        for (_, viewer) in std::mem::take(&mut self.viewers) {
            viewer.stop().await;
        }
    }
    async fn initial(&mut self) -> Result<(), ApiError> {
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
        self.replace(records).await;
        Ok(())
    }
    async fn set(&mut self, value: Value) -> Result<(), ApiError> {
        self.authorize_outer().await?;
        if self.aggregate {
            if value.get("type").and_then(Value::as_str) != Some("set") {
                return Err(Error::InvalidInput("managed_service_set").into());
            }
            self.replace(desired(
                value
                    .get("records")
                    .ok_or(Error::InvalidInput("managed_service_records"))?,
            )?)
            .await;
        } else {
            if value.get("type").and_then(Value::as_str) != Some("close") {
                return Err(Error::InvalidInput("managed_service_stream_input").into());
            }
            self.replace(Vec::new()).await;
        }
        Ok(())
    }
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
    operation: ManagedServiceOperation,
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
    let aggregate_id = operation
        .stream
        .as_ref()
        .and_then(|s| s.aggregate_source_interface_id.as_deref());
    let aggregate = aggregate_id.is_some();
    let source = host::operation(
        &registration,
        aggregate_id.unwrap_or(&operation.interface_id),
    )?;
    let event_validator = jsonschema::validator_for(
        &operation
            .stream
            .as_ref()
            .ok_or(Error::InvalidInput("managed_service_stream"))?
            .event_schema,
    )?;
    let (publisher, stream) = interface_runtime::interface_stream_channel(16);
    let latest = watch::channel(Latest::new()).0;
    let mut changes = latest.subscribe();
    let mut session = Session {
        state,
        registration,
        principal,
        operation,
        source,
        aggregate,
        payload,
        viewers: BTreeMap::new(),
        latest,
    };
    tokio::spawn(async move {
        let initialize = tokio::select! {_ = cancel.changed() => Err(ApiError::from(Error::Conflict("managed_service_stream_cancelled"))),result=session.initial()=>result};
        let mut sent = BTreeMap::new();
        let mut close_requested = false;
        if initialize.is_ok() {
            loop {
                if tokio::select! {_ = cancel.changed()=>true,result=session.authorize_outer()=>result.is_err()}
                {
                    break;
                }
                let values = changes.borrow_and_update().clone();
                let mut closed = false;
                for (record, notice) in values {
                    if sent.get(&record) == Some(&notice.sequence) {
                        continue;
                    }
                    sent.insert(record.clone(), notice.sequence);
                    let authorized = if let Some(viewer) = session.viewers.get(&record) {
                        tokio::select! {_ = cancel.changed()=>{closed=true;false},result=viewer.lease.authorize()=>result.is_ok()}
                    } else {
                        false
                    };
                    if closed {
                        break;
                    }
                    if authorized {
                        if let Some(event) = notice.event {
                            let event = if aggregate {
                                json!({"type":"source","record_id":record,"event":event})
                            } else {
                                event
                            };
                            if !emit(&publisher, &event_validator, event, &mut cancel).await {
                                closed = true;
                                break;
                            }
                        }
                    }
                    if notice.error.is_some() || !authorized {
                        if !aggregate
                            && authorized
                            && notice.error == Some("managed_service_source_retrying")
                        {
                            continue;
                        }
                        if !aggregate {
                            closed = true;
                            break;
                        }
                        let error = notice
                            .error
                            .unwrap_or("managed_service_subscription_denied");
                        let event = if aggregate {
                            json!({"type":"subscription_error","record_id":record,"error_code":error})
                        } else {
                            json!({"type":"error","error_code":error})
                        };
                        if !emit(&publisher, &event_validator, event, &mut cancel).await {
                            closed = true;
                            break;
                        }
                        if !authorized {
                            if let Some(viewer) = session.viewers.remove(&record) {
                                viewer.stop().await;
                            }
                        }
                    }
                }
                if closed {
                    break;
                }
                let input = async { duplex.input.lock().await.recv().await };
                tokio::select! {
                    _ = cancel.changed() => break,
                    result = changes.changed() => if result.is_err() {break;},
                    value = input => match value {
                        Some(value) => {
                            let closing=!aggregate && value.get("type").and_then(Value::as_str)==Some("close");
                            if tokio::select! {_ = cancel.changed()=>true,result=session.set(value)=>result.is_err()} {break;}
                            if closing {close_requested=true;break;}
                            sent.retain(|record,_|session.viewers.contains_key(record));
                        }
                        None => break,
                    }
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
