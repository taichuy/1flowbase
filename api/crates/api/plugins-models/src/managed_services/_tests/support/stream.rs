use super::*;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl SocketSession {
    pub fn test_new(state: Arc<ApiState>, generation: Arc<crate::generation_dispatch::GenerationDispatch>, operation: Arc<plugin_framework::ManagedServiceOperation>, headers: HeaderMap, session: String, installation: uuid::Uuid, payload: Value, input: mpsc::Sender<Value>, events: mpsc::Receiver<StreamEvent>, terminal: oneshot::Receiver<
        Result<
            interface_runtime::InterfaceStreamTerminalOutcome<
                ManagedServiceOutput,
                ConsoleInterfaceTargetError,
            >,
            interface_runtime::InterfaceInvocationFailure,
        >,
    >, cancel: watch::Receiver<bool>, lease: SocketLease) -> Self { Self { state, generation, operation, headers, session, installation, payload, input, events, terminal, cancel, lease } }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl SocketLease {
    pub fn test_new(installation: uuid::Uuid, id: uuid::Uuid, cancel: watch::Sender<bool>) -> Self { Self { installation, id, cancel } }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl RegisteredStream {
    pub fn test_new(cancel: watch::Sender<bool>, record_permission: Option<StreamRecordAdmission>) -> Self { Self { cancel, record_permission } }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRecordAdmission {
    pub fn test_new(actor: domain::ActorContext, key: domain::RecordPermissionKey) -> Self { Self { actor, key } }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRegistry {
    pub fn test_generation(&self) -> &Option<String> { &self.generation }
    pub fn test_installations(&self) -> &BTreeMap<uuid::Uuid, InstallationStreams> { &self.installations }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl InstallationStreams {
    pub fn test_sockets(&self) -> &BTreeMap<uuid::Uuid, RegisteredStream> { &self.sockets }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl SocketSession {
    pub fn test_generation(&self) -> &Arc<crate::generation_dispatch::GenerationDispatch> { &self.generation }
    pub fn test_state(&self) -> &Arc<ApiState> { &self.state }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRecordAdmission {
    pub fn test_actor(&self) -> &domain::ActorContext { &self.actor }
    pub fn test_key(&self) -> &domain::RecordPermissionKey { &self.key }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl StreamRegistry {
    pub fn test_installations_mut(&mut self) -> &mut BTreeMap<uuid::Uuid, InstallationStreams> { &mut self.installations }
}
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
impl InstallationStreams {
    pub fn test_sockets_mut(&mut self) -> &mut BTreeMap<uuid::Uuid, RegisteredStream> { &mut self.sockets }
}#[doc(hidden)]
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
