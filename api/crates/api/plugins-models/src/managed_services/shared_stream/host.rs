use super::super::{
    interface::{ManagedServiceInput, ManagedServiceOutput},
    stream::{StreamEvent, StreamInput},
    ManagedServiceRegistration,
};
use super::{
    authority,
    broker::{Producer, SourceBackend, SourceKey},
    cache,
};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    routes::console_interface::{self, ConsoleInvocationPreparation},
};
use control_plane::errors::ControlPlaneError as Error;
use interface_runtime::{
    CompiledInterfaceRegistry, InterfaceContract, InvocationEnvelope, UserPrincipal,
};
use plugin_framework::{ManagedServiceOperation, ManagedServiceRecordSelector};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub(super) struct ManagedSource {
    pub state: Arc<ApiState>,
    pub registration: ManagedServiceRegistration,
    pub operation: ManagedServiceOperation,
    pub key: SourceKey,
}
pub(super) struct SharedPreparation;
#[async_trait::async_trait]
impl<I: InterfaceContract> ConsoleInvocationPreparation<I> for SharedPreparation {
    async fn prepare(
        &self,
        _: &ApiState,
        _: &Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, UserPrincipal>,
    ) -> Result<InvocationEnvelope<I, UserPrincipal>, ApiError> {
        Ok(envelope)
    }
}
pub(super) fn operation(
    registration: &ManagedServiceRegistration,
    id: &str,
) -> Result<ManagedServiceOperation, ApiError> {
    registration
        .declaration
        .operations
        .iter()
        .find(|op| op.interface_id == id)
        .cloned()
        .ok_or_else(|| Error::Conflict("managed_service_shared_source").into())
}
pub(super) fn input(
    operation: &ManagedServiceOperation,
    payload: Value,
    stream: Option<StreamInput>,
) -> ManagedServiceInput {
    ManagedServiceInput {
        interface_id: operation.interface_id.clone(),
        payload,
        stream,
        event_policy: Arc::new(super::super::event_projection::OperationPolicy::new(
            operation,
        )),
    }
}
pub(super) fn set_pointer(
    value: &mut Value,
    pointer: &str,
    replacement: Value,
) -> Result<(), ApiError> {
    let parts = pointer
        .strip_prefix('/')
        .ok_or(Error::InvalidInput("managed_service_selector"))?
        .split('/')
        .map(|part| part.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    let mut current = value;
    for (index, part) in parts.iter().enumerate() {
        if !current.is_object() {
            return Err(Error::InvalidInput("managed_service_selector").into());
        }
        if index + 1 == parts.len() {
            current[part] = replacement;
            return Ok(());
        }
        if current.get(part).is_none() {
            current[part] = json!({});
        }
        current = &mut current[part];
    }
    Err(Error::InvalidInput("managed_service_selector").into())
}
pub(super) fn record_pointer(operation: &ManagedServiceOperation) -> Result<&str, ApiError> {
    match operation.record_permission.as_ref().map(|p| &p.selector) {
        Some(ManagedServiceRecordSelector::Record {
            pointer,
            allow_missing: false,
        }) => Ok(pointer),
        _ => Err(Error::Conflict("managed_service_shared_source").into()),
    }
}
pub(super) fn interval(
    operation: &ManagedServiceOperation,
    payload: &Value,
) -> Result<u64, ApiError> {
    let pointer = &operation
        .stream
        .as_ref()
        .and_then(|s| s.shared_source.as_ref())
        .ok_or(Error::Conflict("managed_service_shared_source"))?
        .interval_pointer;
    interval_at(operation, payload, pointer)
}
pub(super) fn interval_at(
    operation: &ManagedServiceOperation,
    payload: &Value,
    pointer: &str,
) -> Result<u64, ApiError> {
    let mut schema = &operation.input_schema;
    for part in pointer
        .strip_prefix('/')
        .ok_or(Error::Conflict("managed_service_interval"))?
        .split('/')
    {
        let key = part.replace("~1", "/").replace("~0", "~");
        schema = schema
            .get("properties")
            .and_then(|properties| properties.get(key))
            .ok_or(Error::Conflict("managed_service_interval"))?;
    }
    let interval = payload
        .pointer(pointer)
        .or_else(|| schema.get("default"))
        .and_then(Value::as_u64)
        .ok_or(Error::InvalidInput("managed_service_interval"))?;
    cache::ttl_ms(interval)?;
    Ok(interval)
}
pub(super) fn record_payload(
    operation: &ManagedServiceOperation,
    record: &str,
    interval: u64,
) -> Result<Value, ApiError> {
    let mut payload = json!({"path":{},"query":{},"body":{}});
    set_pointer(&mut payload, record_pointer(operation)?, json!(record))?;
    let pointer = &operation
        .stream
        .as_ref()
        .and_then(|s| s.shared_source.as_ref())
        .ok_or(Error::Conflict("managed_service_shared_source"))?
        .interval_pointer;
    set_pointer(&mut payload, pointer, json!(interval))?;
    Ok(payload)
}
impl ManagedSource {
    pub fn new(
        state: Arc<ApiState>,
        registration: ManagedServiceRegistration,
        operation: ManagedServiceOperation,
        principal: &UserPrincipal,
        payload: &Value,
    ) -> Result<Self, ApiError> {
        let permission = operation
            .record_permission
            .as_ref()
            .ok_or(Error::Conflict("managed_service_shared_source"))?;
        let record = payload
            .pointer(record_pointer(&operation)?)
            .and_then(Value::as_str)
            .ok_or(Error::InvalidInput("record_id"))?;
        let resource =
            super::super::record_acl::resource(&registration, &permission.resource_code)?;
        let key =
            super::super::record_acl::key(&registration, principal.actor(), resource, record)?;
        let key = SourceKey {
            installation: registration.installation_id,
            generation: super::super::stream_ticket::generation(&state)?,
            interface: operation.interface_id.clone(),
            resource: permission.resource_code.clone(),
            record: key.record_id,
        };
        Ok(Self {
            state,
            registration,
            operation,
            key,
        })
    }
    pub async fn admit(
        &self,
        principal: &UserPrincipal,
        payload: &Value,
    ) -> Result<UserPrincipal, ApiError> {
        let principal = authority::authorize(self, principal, payload).await?;
        let invocation = console_interface::invoke_server_stream_with_principal_prepared::<
            ManagedServiceInput,
            StreamEvent,
            ManagedServiceOutput,
            _,
        >(
            self.state.clone(),
            &format!("http.{}.v1", self.operation.interface_id),
            principal.clone(),
            input(
                &self.operation,
                payload.clone(),
                Some(StreamInput::SourceAdmission),
            ),
            SharedPreparation,
        )
        .await?;
        let (events, completion) = invocation.into_parts();
        drop(events);
        let outcome = completion
            .complete()
            .await
            .map_err(|_| Error::PermissionDenied("managed_service_source_admission"))?;
        match outcome.into_parts().0 {
            interface_runtime::InterfaceStreamTerminal::Completed(_) => Ok(principal),
            _ => Err(Error::PermissionDenied("managed_service_source_admission").into()),
        }
    }
    pub async fn resolve(
        &self,
        principal: &UserPrincipal,
        payload: &Value,
    ) -> Result<String, ApiError> {
        let declaration = self
            .operation
            .stream
            .as_ref()
            .and_then(|s| s.shared_source.as_ref())
            .ok_or(Error::Conflict("managed_service_shared_source"))?;
        let resolver = operation(&self.registration, &declaration.resolve_interface_id)?;
        let ManagedServiceOutput(value, _) = console_interface::invoke_with_principal_prepared::<
            ManagedServiceInput,
            ManagedServiceOutput,
            _,
        >(
            self.state.clone(),
            &format!("http.{}.v1", resolver.interface_id),
            principal.clone(),
            input(&resolver, payload.clone(), None),
            SharedPreparation,
        )
        .await?;
        value
            .get("revision")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| Error::UpstreamUnavailable("managed_service_source_revision").into())
    }
    fn projections(&self) -> Vec<String> {
        self.registration
            .declaration
            .operations
            .iter()
            .filter(|op| {
                op.shared_snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.source_interface_id == self.operation.interface_id
                })
            })
            .map(|op| op.interface_id.clone())
            .collect()
    }
    pub fn ready(&self, event: &Value) -> bool {
        self.registration
            .declaration
            .operations
            .iter()
            .filter_map(|op| op.shared_snapshot.as_ref())
            .any(|projection| {
                projection.source_interface_id == self.operation.interface_id
                    && event.pointer(&projection.ready_pointer) == Some(&projection.ready_value)
                    && event.pointer(&projection.value_pointer).is_some()
            })
    }
}
#[async_trait::async_trait]
impl SourceBackend for ManagedSource {
    fn ready(&self, event: &Value) -> bool {
        ManagedSource::ready(self, event)
    }
    async fn authorize(
        &self,
        principal: &UserPrincipal,
        payload: &Value,
    ) -> Result<UserPrincipal, ApiError> {
        authority::authorize(self, principal, payload).await
    }
    async fn start(
        &self,
        principal: UserPrincipal,
        mut payload: Value,
        interval: u64,
        mut cancel: watch::Receiver<bool>,
    ) -> Result<Producer, ApiError> {
        if principal.authenticated_session().is_none() {
            return Err(Error::PermissionDenied("cookie_session_required").into());
        }
        let specification = self
            .operation
            .stream
            .as_ref()
            .and_then(|s| s.shared_source.as_ref())
            .ok_or(Error::Conflict("managed_service_shared_source"))?;
        set_pointer(
            &mut payload,
            &specification.interval_pointer,
            json!(interval),
        )?;
        let (input_sender, input_receiver) = mpsc::channel(1);
        let (unused_events, _) = mpsc::channel(1);
        let (raw_cancel, raw_cancellation) = watch::channel(false);
        let invocation = console_interface::invoke_server_stream_with_principal_prepared::<
            ManagedServiceInput,
            StreamEvent,
            ManagedServiceOutput,
            _,
        >(
            self.state.clone(),
            &format!("http.{}.v1", self.operation.interface_id),
            principal,
            input(
                &self.operation,
                payload,
                Some(StreamInput::Source {
                    duplex: runtime_core::runtime_backend::RuntimeManagedDuplex {
                        input: Arc::new(tokio::sync::Mutex::new(input_receiver)),
                        events: unused_events,
                    },
                    cancel: raw_cancellation,
                }),
            ),
            SharedPreparation,
        )
        .await?;
        let (events, completion) = invocation.into_parts();
        Ok(Producer {
            events,
            completion: Box::pin(async move {
                let _raw_cancel = raw_cancel;
                let completion = completion.complete();
                tokio::pin!(completion);
                let outcome = tokio::select! {
                    result = &mut completion => result,
                    _ = async { if !*cancel.borrow() {let _ = cancel.changed().await;} } => {
                        // Closing Host input requests the existing multiplex Cancel and
                        // keeps its call future alive until the worker acknowledges it.
                        // Dropping the raw execution future would only queue cancellation.
                        drop(input_sender);
                        completion.await
                    }
                };
                match outcome
                    .map_err(|_| Error::UpstreamUnavailable("managed_service_source_execution"))?
                    .into_parts()
                    .0
                {
                    interface_runtime::InterfaceStreamTerminal::Completed(_) => Ok(()),
                    _ => Err(Error::UpstreamUnavailable("managed_service_source_execution").into()),
                }
            }),
        })
    }
    async fn cache(&self, revision: &str, interval: u64, event: &Value) -> Result<(), ApiError> {
        cache::invalidate_projections(
            self.state.infrastructure.ephemeral_kv_store().as_ref(),
            &self.key,
            revision,
            &self.projections(),
        )
        .await?;
        if self.ready(event) {
            cache::put(
                self.state.infrastructure.ephemeral_kv_store().as_ref(),
                &self.key,
                revision,
                interval,
                event.clone(),
            )
            .await?;
        } else {
            self.invalidate(revision).await?;
        }
        Ok(())
    }
    async fn invalidate(&self, revision: &str) -> Result<(), ApiError> {
        cache::invalidate(
            self.state.infrastructure.ephemeral_kv_store().as_ref(),
            &self.key,
            revision,
            &self.projections(),
        )
        .await
    }
}
