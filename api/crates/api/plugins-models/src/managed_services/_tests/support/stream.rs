use super::*;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct SocketSessionTestContext {
    pub state: Arc<ApiState>,
    pub generation: Arc<crate::generation_dispatch::GenerationDispatch>,
    pub operation: Arc<plugin_framework::ManagedServiceOperation>,
    pub headers: HeaderMap,
    pub session: String,
    pub installation: uuid::Uuid,
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub struct SocketSessionTestTransport {
    pub payload: Value,
    pub input: mpsc::Sender<Value>,
    pub events: mpsc::Receiver<StreamEvent>,
    pub terminal: oneshot::Receiver<
        Result<
            interface_runtime::InterfaceStreamTerminalOutcome<
                ManagedServiceOutput,
                ConsoleInterfaceTargetError,
            >,
            interface_runtime::InterfaceInvocationFailure,
        >,
    >,
    pub cancel: watch::Receiver<bool>,
    pub lease: SocketLease,
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl SocketSession {
    pub fn test_new(
        context: SocketSessionTestContext,
        transport: SocketSessionTestTransport,
    ) -> Self {
        Self {
            state: context.state,
            generation: context.generation,
            operation: context.operation,
            headers: context.headers,
            session: context.session,
            installation: context.installation,
            payload: transport.payload,
            input: transport.input,
            events: transport.events,
            terminal: transport.terminal,
            cancel: transport.cancel,
            lease: transport.lease,
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl SocketLease {
    pub fn test_new(installation: uuid::Uuid, id: uuid::Uuid, cancel: watch::Sender<bool>) -> Self {
        Self {
            installation,
            id,
            cancel,
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl RegisteredStream {
    pub fn test_new(
        cancel: watch::Sender<bool>,
        record_permission: Option<StreamRecordAdmission>,
    ) -> Self {
        Self {
            cancel,
            record_permission,
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRecordAdmission {
    pub fn test_new(actor: domain::ActorContext, key: domain::RecordPermissionKey) -> Self {
        Self { actor, key }
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRegistry {
    pub fn test_generation(&self) -> &Option<String> {
        &self.generation
    }
    pub fn test_installations(&self) -> &BTreeMap<uuid::Uuid, InstallationStreams> {
        &self.installations
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl InstallationStreams {
    pub fn test_sockets(&self) -> &BTreeMap<uuid::Uuid, RegisteredStream> {
        &self.sockets
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl SocketSession {
    pub fn test_generation(&self) -> &Arc<crate::generation_dispatch::GenerationDispatch> {
        &self.generation
    }
    pub fn test_state(&self) -> &Arc<ApiState> {
        &self.state
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRecordAdmission {
    pub fn test_actor(&self) -> &domain::ActorContext {
        &self.actor
    }
    pub fn test_key(&self) -> &domain::RecordPermissionKey {
        &self.key
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRegistry {
    pub fn test_installations_mut(&mut self) -> &mut BTreeMap<uuid::Uuid, InstallationStreams> {
        &mut self.installations
    }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl InstallationStreams {
    pub fn test_sockets_mut(&mut self) -> &mut BTreeMap<uuid::Uuid, RegisteredStream> {
        &mut self.sockets
    }
}
#[doc(hidden)]
pub async fn register_test_stream(
    state: &ApiState,
    headers: &HeaderMap,
    installation: uuid::Uuid,
    operation: &plugin_framework::ManagedServiceOperation,
    payload: &Value,
) -> Result<(SocketLease, watch::Receiver<bool>), ApiError> {
    let context = crate::middleware::require_session::require_session(state, headers).await?;
    let session = context.cookie_session()?.session_id.clone();
    let admission = revalidate(state, headers, &session, operation, installation, payload).await?;
    let generation = crate::managed_services::stream_ticket::generation(state)?;
    register(installation, epoch(installation), &generation, admission)
}
#[doc(hidden)]
pub async fn revalidate_test_stream(
    state: &ApiState,
    headers: &HeaderMap,
    session: &str,
    operation: &plugin_framework::ManagedServiceOperation,
    installation: uuid::Uuid,
    payload: &Value,
) -> Result<(), ApiError> {
    revalidate(state, headers, session, operation, installation, payload)
        .await
        .map(|_| ())
}
