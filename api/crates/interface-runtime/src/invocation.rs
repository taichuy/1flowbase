use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::SystemTime,
};

use thiserror::Error;
use uuid::Uuid;

use crate::{
    AdmissionAdapterReference, ArtifactIdentity, AuthenticationAdapterReference,
    AuthorizationAdapterReference, BindingFingerprint, BindingId, CompiledInterfaceRegistry,
    ContractIdentity, GraphFingerprint, HandlerReference, InterfaceContract, InterfaceDefinition,
    InterfaceHandlerContext, InterfaceHookContext, InterfaceId, InterfaceStreamCompletion,
    InterfaceStreamInvocation, InterfaceStreamTerminalOutcome, InterfaceTargetError,
    InvocationPrincipal, PlanFingerprint, PluginIdentity, PrincipalSummary, ProtocolBinding,
    RegistryFingerprint, RuntimeGeneration, RuntimeTargetIdentity, TargetReference,
    TypedInterfaceHookPlan, UserPrincipal, WorkerGeneration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InvocationInterruption {
    DeadlineElapsed,
    Cancelled,
}

async fn await_in_flight<T>(
    controls: &InvocationControls,
    future: impl Future<Output = T>,
) -> Result<T, InvocationInterruption> {
    let cancellation = controls.cancellation.cancelled();
    tokio::pin!(cancellation);
    if controls.cancellation.is_cancelled() {
        return Err(InvocationInterruption::Cancelled);
    }
    match controls.deadline {
        Some(deadline) => {
            let remaining = deadline
                .duration_since(SystemTime::now())
                .map_err(|_| InvocationInterruption::DeadlineElapsed)?;
            tokio::select! {
                _ = &mut cancellation => Err(InvocationInterruption::Cancelled),
                result = tokio::time::timeout(remaining, future) => {
                    result.map_err(|_| InvocationInterruption::DeadlineElapsed)
                }
            }
        }
        None => tokio::select! {
            _ = &mut cancellation => Err(InvocationInterruption::Cancelled),
            result = future => Ok(result),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InvocationId(Uuid);

impl InvocationId {
    pub fn new(value: Uuid) -> Self {
        Self(value)
    }

    pub fn now_v7() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn value(self) -> Uuid {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExecutionAttemptId(Uuid);

impl ExecutionAttemptId {
    pub fn value(self) -> Uuid {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionTargetPin {
    BuiltIn {
        handler: HandlerReference,
        target: TargetReference,
    },
    Runtime {
        handler: HandlerReference,
        target: TargetReference,
        plugin: PluginIdentity,
        artifact: ArtifactIdentity,
        runtime: RuntimeTargetIdentity,
        runtime_generation: RuntimeGeneration,
        worker_generation: WorkerGeneration,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionAttempt {
    attempt_id: ExecutionAttemptId,
    ordinal: u32,
    target: ExecutionTargetPin,
}

impl ExecutionAttempt {
    fn first(target: ExecutionTargetPin) -> Self {
        Self {
            attempt_id: ExecutionAttemptId(Uuid::now_v7()),
            ordinal: 1,
            target,
        }
    }

    pub fn retry(&self, target: ExecutionTargetPin) -> Self {
        Self {
            attempt_id: ExecutionAttemptId(Uuid::now_v7()),
            ordinal: self.ordinal + 1,
            target,
        }
    }

    pub fn attempt_id(&self) -> ExecutionAttemptId {
        self.attempt_id
    }

    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn target(&self) -> &ExecutionTargetPin {
        &self.target
    }
}

#[derive(Clone, Default)]
pub struct InvocationCancellation {
    cancelled: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

impl std::fmt::Debug for InvocationCancellation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InvocationCancellation")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl InvocationCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    async fn cancelled(&self) {
        let notified = self.notify.notified();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdempotencyKey(Arc<str>);

impl IdempotencyKey {
    pub fn new(value: impl AsRef<str>) -> Result<Self, IdempotencyKeyError> {
        let value = value.as_ref().trim();
        if value.is_empty() || value.len() > 256 {
            return Err(IdempotencyKeyError);
        }
        Ok(Self(Arc::from(value)))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("idempotency key must contain between 1 and 256 bytes")]
pub struct IdempotencyKeyError;

#[derive(Clone, Debug)]
pub struct InvocationControls {
    deadline: Option<SystemTime>,
    cancellation: InvocationCancellation,
    idempotency_key: Option<IdempotencyKey>,
}

impl InvocationControls {
    pub fn new(
        deadline: Option<SystemTime>,
        cancellation: InvocationCancellation,
        idempotency_key: Option<IdempotencyKey>,
    ) -> Self {
        Self {
            deadline,
            cancellation,
            idempotency_key,
        }
    }

    pub fn deadline(&self) -> Option<SystemTime> {
        self.deadline
    }

    pub fn cancellation(&self) -> &InvocationCancellation {
        &self.cancellation
    }

    pub fn idempotency_key(&self) -> Option<&IdempotencyKey> {
        self.idempotency_key.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InvocationLineageError {
    #[error("invocation identity {0} forms a parent cycle")]
    Cycle(Uuid),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationLineage {
    chain: Arc<[InvocationId]>,
}

impl InvocationLineage {
    pub fn root(invocation_id: InvocationId) -> Self {
        Self {
            chain: Arc::from([invocation_id]),
        }
    }

    pub fn child(&self, invocation_id: InvocationId) -> Result<Self, InvocationLineageError> {
        if self.chain.contains(&invocation_id) {
            return Err(InvocationLineageError::Cycle(invocation_id.value()));
        }
        let mut chain = self.chain.to_vec();
        chain.push(invocation_id);
        Ok(Self {
            chain: Arc::from(chain),
        })
    }

    pub fn invocation_id(&self) -> InvocationId {
        *self
            .chain
            .last()
            .expect("invocation lineage always contains its current identity")
    }

    pub fn parent_invocation_id(&self) -> Option<InvocationId> {
        self.chain
            .len()
            .checked_sub(2)
            .and_then(|index| self.chain.get(index).copied())
    }

    pub fn identities(&self) -> &[InvocationId] {
        &self.chain
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceProtocol {
    Http,
    Mcp,
    Internal,
    Worker,
}

pub struct InvocationEnvelope<I, P = UserPrincipal>
where
    I: InterfaceContract,
    P: InvocationPrincipal,
{
    lineage: InvocationLineage,
    binding_id: BindingId,
    protocol: InterfaceProtocol,
    authentication_adapter: AuthenticationAdapterReference,
    principal: P,
    controls: InvocationControls,
    input: I,
}

impl<I, P> InvocationEnvelope<I, P>
where
    I: InterfaceContract,
    P: InvocationPrincipal,
{
    pub fn with_principal(
        lineage: InvocationLineage,
        binding_id: BindingId,
        protocol: InterfaceProtocol,
        authentication_adapter: AuthenticationAdapterReference,
        principal: P,
        deadline: Option<SystemTime>,
        input: I,
    ) -> Self {
        Self::with_principal_and_controls(
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            principal,
            InvocationControls::new(deadline, InvocationCancellation::new(), None),
            input,
        )
    }

    pub fn with_principal_and_controls(
        lineage: InvocationLineage,
        binding_id: BindingId,
        protocol: InterfaceProtocol,
        authentication_adapter: AuthenticationAdapterReference,
        principal: P,
        controls: InvocationControls,
        input: I,
    ) -> Self {
        Self {
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            principal,
            controls,
            input,
        }
    }

    pub fn lineage(&self) -> &InvocationLineage {
        &self.lineage
    }

    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }

    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }

    pub fn authentication_adapter(&self) -> &AuthenticationAdapterReference {
        &self.authentication_adapter
    }

    pub fn principal(&self) -> &P {
        &self.principal
    }

    pub fn principal_summary(&self) -> PrincipalSummary {
        self.principal.summary()
    }

    pub fn controls(&self) -> &InvocationControls {
        &self.controls
    }
}

impl<I> InvocationEnvelope<I, UserPrincipal>
where
    I: InterfaceContract,
{
    pub fn new(
        lineage: InvocationLineage,
        binding_id: BindingId,
        protocol: InterfaceProtocol,
        authentication_adapter: AuthenticationAdapterReference,
        actor: domain::ActorContext,
        deadline: Option<SystemTime>,
        input: I,
    ) -> Self {
        Self::with_principal(
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            UserPrincipal::server_delegation(actor),
            deadline,
            input,
        )
    }
}

#[derive(Clone, Debug)]
pub struct InterfaceAuthorizationRequest<P = UserPrincipal>
where
    P: InvocationPrincipal,
{
    principal: P,
    definition: InterfaceDefinition,
    binding: ProtocolBinding,
    protocol: InterfaceProtocol,
}

impl<P> InterfaceAuthorizationRequest<P>
where
    P: InvocationPrincipal,
{
    fn new(
        principal: P,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            principal,
            definition,
            binding,
            protocol,
        }
    }

    pub fn principal(&self) -> &P {
        &self.principal
    }

    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }

    pub fn binding(&self) -> &ProtocolBinding {
        &self.binding
    }

    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }
}

#[derive(Debug, Error)]
#[error("interface authorization rejected with {classification}")]
pub struct InterfaceAuthorizationError {
    classification: Arc<str>,
    payload: Option<Box<dyn std::any::Any + Send + Sync>>,
}

impl InterfaceAuthorizationError {
    pub fn classified(classification: impl AsRef<str>) -> Self {
        Self {
            classification: Arc::from(classification.as_ref()),
            payload: None,
        }
    }

    pub fn with_source<T>(classification: impl AsRef<str>, source: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self {
            classification: Arc::from(classification.as_ref()),
            payload: Some(Box::new(source)),
        }
    }

    pub fn classification(&self) -> &str {
        self.classification.as_ref()
    }

    pub fn into_source<T>(self) -> Option<T>
    where
        T: Send + Sync + 'static,
    {
        self.payload?.downcast::<T>().ok().map(|source| *source)
    }
}

pub type InterfaceAuthorizationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), InterfaceAuthorizationError>> + Send + 'a>>;

pub trait InterfaceAuthorizationPort<P = UserPrincipal>: Send + Sync + 'static
where
    P: InvocationPrincipal,
{
    fn adapter_reference(&self) -> AuthorizationAdapterReference;

    fn authorize(
        &self,
        request: InterfaceAuthorizationRequest<P>,
    ) -> InterfaceAuthorizationFuture<'_>;
}

#[derive(Clone, Debug)]
pub struct InterfaceTargetAdmissionRequest {
    principal: PrincipalSummary,
    definition: InterfaceDefinition,
    binding: ProtocolBinding,
    protocol: InterfaceProtocol,
}

impl InterfaceTargetAdmissionRequest {
    fn new(
        principal: PrincipalSummary,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            principal,
            definition,
            binding,
            protocol,
        }
    }

    pub fn principal(&self) -> &PrincipalSummary {
        &self.principal
    }

    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }

    pub fn binding(&self) -> &ProtocolBinding {
        &self.binding
    }

    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }
}

#[derive(Debug, Error)]
#[error("interface target admission rejected with {classification}")]
pub struct InterfaceTargetAdmissionError {
    classification: Arc<str>,
    payload: Option<Box<dyn std::any::Any + Send + Sync>>,
}

impl InterfaceTargetAdmissionError {
    pub fn classified(classification: impl AsRef<str>) -> Self {
        Self {
            classification: Arc::from(classification.as_ref()),
            payload: None,
        }
    }

    pub fn with_source<T>(classification: impl AsRef<str>, source: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        Self {
            classification: Arc::from(classification.as_ref()),
            payload: Some(Box::new(source)),
        }
    }

    pub fn classification(&self) -> &str {
        self.classification.as_ref()
    }

    pub fn into_source<T>(self) -> Option<T>
    where
        T: Send + Sync + 'static,
    {
        self.payload?.downcast::<T>().ok().map(|source| *source)
    }
}

pub type InterfaceTargetAdmissionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), InterfaceTargetAdmissionError>> + Send + 'a>>;

pub trait InterfaceTargetAdmissionPort: Send + Sync + 'static {
    fn adapter_reference(&self) -> AdmissionAdapterReference;

    fn admit(&self, request: InterfaceTargetAdmissionRequest)
        -> InterfaceTargetAdmissionFuture<'_>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceInvocationStage {
    Received,
    Resolved,
    PrincipalEstablished,
    Authorized,
    Admitted,
    Prepared,
    Dispatched,
    Executing,
    PostProcessed,
    Projected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceStageRecord {
    stage: InterfaceInvocationStage,
    at: SystemTime,
}

impl InterfaceStageRecord {
    pub fn stage(&self) -> InterfaceInvocationStage {
        self.stage
    }

    pub fn at(&self) -> SystemTime {
        self.at
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceInvocationTerminal {
    Completed,
    Failed,
    Rejected,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedInvocationPin {
    interface_version: crate::InterfaceVersion,
    binding_id: BindingId,
    binding_fingerprint: BindingFingerprint,
    plan_fingerprint: PlanFingerprint,
}

impl ResolvedInvocationPin {
    pub fn interface_version(&self) -> &crate::InterfaceVersion {
        &self.interface_version
    }

    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }

    pub fn binding_fingerprint(&self) -> &BindingFingerprint {
        &self.binding_fingerprint
    }

    pub fn plan_fingerprint(&self) -> &PlanFingerprint {
        &self.plan_fingerprint
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceInvocationReceipt {
    invocation_id: InvocationId,
    parent_invocation_id: Option<InvocationId>,
    interface_id: Option<InterfaceId>,
    resolved: Option<ResolvedInvocationPin>,
    protocol: InterfaceProtocol,
    principal: PrincipalSummary,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
    stages: Vec<InterfaceStageRecord>,
    attempt: Option<ExecutionAttempt>,
    idempotency_key: Option<IdempotencyKey>,
    terminal: InterfaceInvocationTerminal,
    terminal_at: SystemTime,
}

impl InterfaceInvocationReceipt {
    pub fn invocation_id(&self) -> InvocationId {
        self.invocation_id
    }

    pub fn parent_invocation_id(&self) -> Option<InvocationId> {
        self.parent_invocation_id
    }

    pub fn interface_id(&self) -> Option<&InterfaceId> {
        self.interface_id.as_ref()
    }

    pub fn resolved(&self) -> Option<&ResolvedInvocationPin> {
        self.resolved.as_ref()
    }

    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }

    pub fn principal(&self) -> &PrincipalSummary {
        &self.principal
    }

    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn registry_fingerprint(&self) -> &RegistryFingerprint {
        &self.registry_fingerprint
    }

    pub fn stage_records(&self) -> &[InterfaceStageRecord] {
        &self.stages
    }

    pub fn stages(&self) -> impl ExactSizeIterator<Item = InterfaceInvocationStage> + '_ {
        self.stages.iter().map(InterfaceStageRecord::stage)
    }

    pub fn attempt(&self) -> Option<&ExecutionAttempt> {
        self.attempt.as_ref()
    }

    pub fn idempotency_key(&self) -> Option<&IdempotencyKey> {
        self.idempotency_key.as_ref()
    }

    pub fn terminal(&self) -> InterfaceInvocationTerminal {
        self.terminal
    }

    pub fn terminal_at(&self) -> SystemTime {
        self.terminal_at
    }

    pub fn projected(mut self) -> Self {
        if !self
            .stages
            .iter()
            .any(|record| record.stage == InterfaceInvocationStage::Projected)
        {
            self.stages.push(InterfaceStageRecord {
                stage: InterfaceInvocationStage::Projected,
                at: SystemTime::now(),
            });
        }
        self
    }
}

#[derive(Debug, Error)]
pub enum InterfaceInvocationError {
    #[error("protocol binding is not registered")]
    UnknownBinding,
    #[error("envelope protocol does not match its binding projection")]
    ProtocolBindingMismatch,
    #[error("authentication adapter does not match the compiled invocation plan")]
    AuthenticationAdapterMismatch,
    #[error("authorization adapter does not match the compiled invocation plan")]
    AuthorizationAdapterMismatch,
    #[error("admission adapter does not match the compiled invocation plan")]
    AdmissionAdapterMismatch,
    #[error("invocation contract does not match the registered interface")]
    ContractMismatch,
    #[error("invocation principal profile does not match the registered interface")]
    PrincipalProfileMismatch,
    #[error("hook plan fingerprint does not match the invocation snapshot")]
    HookPlanFingerprintMismatch,
    #[error(transparent)]
    AuthorizationRejected(InterfaceAuthorizationError),
    #[error(transparent)]
    AdmissionRejected(InterfaceTargetAdmissionError),
    #[error(transparent)]
    BeforeHookRejected(crate::InterfaceBeforeHookError),
    #[error(transparent)]
    TargetFailed(InterfaceTargetError),
    #[error("interface invocation deadline elapsed")]
    DeadlineElapsed,
    #[error("interface invocation was cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceTargetFailure<E> {
    classification: Arc<str>,
    error: E,
}

impl<E> InterfaceTargetFailure<E>
where
    E: InterfaceContract,
{
    pub fn new(classification: impl AsRef<str>, error: E) -> Self {
        Self {
            classification: Arc::from(classification.as_ref()),
            error,
        }
    }

    pub fn classification(&self) -> &str {
        self.classification.as_ref()
    }

    pub fn error(&self) -> &E {
        &self.error
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterfaceStreamTerminal<O, E>
where
    O: InterfaceContract,
    E: InterfaceContract,
{
    Completed(O),
    Failed(InterfaceTargetFailure<E>),
    Rejected { classification: Arc<str> },
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceServerStream<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    events: Vec<S>,
    terminal: InterfaceStreamTerminal<O, E>,
}

impl<S, O, E> InterfaceServerStream<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub fn events(&self) -> &[S] {
        &self.events
    }

    pub fn terminal(&self) -> &InterfaceStreamTerminal<O, E> {
        &self.terminal
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum InterfaceStreamStateError {
    #[error("stream terminal is missing")]
    MissingTerminal,
    #[error("stream already has a terminal")]
    DuplicateTerminal,
    #[error("stream event cannot follow its terminal")]
    EventAfterTerminal,
}

pub struct InterfaceStreamAccumulator<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    events: Vec<S>,
    terminal: Option<InterfaceStreamTerminal<O, E>>,
}

impl<S, O, E> Default for InterfaceStreamAccumulator<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    fn default() -> Self {
        Self {
            events: Vec::new(),
            terminal: None,
        }
    }
}

impl<S, O, E> InterfaceStreamAccumulator<S, O, E>
where
    S: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, event: S) -> Result<(), InterfaceStreamStateError> {
        if self.terminal.is_some() {
            return Err(InterfaceStreamStateError::EventAfterTerminal);
        }
        self.events.push(event);
        Ok(())
    }

    pub fn finish(
        &mut self,
        terminal: InterfaceStreamTerminal<O, E>,
    ) -> Result<(), InterfaceStreamStateError> {
        if self.terminal.is_some() {
            return Err(InterfaceStreamStateError::DuplicateTerminal);
        }
        self.terminal = Some(terminal);
        Ok(())
    }

    pub fn into_stream(self) -> Result<InterfaceServerStream<S, O, E>, InterfaceStreamStateError> {
        Ok(InterfaceServerStream {
            events: self.events,
            terminal: self
                .terminal
                .ok_or(InterfaceStreamStateError::MissingTerminal)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalInvocationResult<O, E, S>
where
    O: InterfaceContract,
    E: InterfaceContract,
    S: InterfaceContract,
{
    Unary(Result<O, InterfaceTargetFailure<E>>),
    ServerStream(InterfaceServerStream<S, O, E>),
    AsyncAck(O),
    PlatformFailure { classification: Arc<str> },
}

#[derive(Debug)]
pub struct InterfaceInvocationFailure {
    error: InterfaceInvocationError,
    receipt: InterfaceInvocationReceipt,
}

impl InterfaceInvocationFailure {
    pub fn error(&self) -> &InterfaceInvocationError {
        &self.error
    }

    pub fn receipt(&self) -> &InterfaceInvocationReceipt {
        &self.receipt
    }

    pub fn into_error(self) -> InterfaceInvocationError {
        self.error
    }
}

#[derive(Debug)]
pub struct InterfaceInvocationOutcome<O>
where
    O: InterfaceContract,
{
    value: O,
    receipt: InterfaceInvocationReceipt,
}

impl<O> InterfaceInvocationOutcome<O>
where
    O: InterfaceContract,
{
    pub fn into_value(self) -> O {
        self.value
    }

    pub fn value(&self) -> &O {
        &self.value
    }

    pub fn receipt(&self) -> &InterfaceInvocationReceipt {
        &self.receipt
    }
}

pub type InterfaceInvocationResult<O> =
    Result<InterfaceInvocationOutcome<O>, InterfaceInvocationFailure>;

pub struct InterfaceInvocationKernel<P = UserPrincipal>
where
    P: InvocationPrincipal,
{
    authorization: Arc<dyn InterfaceAuthorizationPort<P>>,
    target_admission: Option<Arc<dyn InterfaceTargetAdmissionPort>>,
}

impl<P> InterfaceInvocationKernel<P>
where
    P: InvocationPrincipal,
{
    pub fn new(authorization: Arc<dyn InterfaceAuthorizationPort<P>>) -> Self {
        Self {
            authorization,
            target_admission: None,
        }
    }

    pub fn with_target_admission(
        authorization: Arc<dyn InterfaceAuthorizationPort<P>>,
        target_admission: Arc<dyn InterfaceTargetAdmissionPort>,
    ) -> Self {
        Self {
            authorization,
            target_admission: Some(target_admission),
        }
    }

    pub async fn invoke<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        self.invoke_internal::<I, O, E>(snapshot, envelope, None, None)
            .await
    }

    pub async fn invoke_with_dispatch_target<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        target: ExecutionTargetPin,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        self.invoke_internal::<I, O, E>(snapshot, envelope, None, Some(target))
            .await
    }

    pub async fn invoke_with_hook_plan<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        hook_plan: &TypedInterfaceHookPlan<I, O>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        self.invoke_internal::<I, O, E>(snapshot, envelope, Some(hook_plan), None)
            .await
    }

    pub async fn invoke_server_stream_with_dispatch_target<I, S, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        target: ExecutionTargetPin,
    ) -> Result<InterfaceStreamInvocation<S, O, E>, InterfaceInvocationFailure>
    where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        let InvocationEnvelope {
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            principal,
            controls,
            input,
        } = envelope;
        let mut receipt = ReceiptBuilder::new(
            &snapshot,
            lineage.invocation_id(),
            lineage.parent_invocation_id(),
            protocol,
            principal.summary(),
            &controls,
        );
        let interruption = if controls.cancellation.is_cancelled() {
            Some(InvocationInterruption::Cancelled)
        } else if controls
            .deadline
            .is_some_and(|deadline| deadline <= SystemTime::now())
        {
            Some(InvocationInterruption::DeadlineElapsed)
        } else {
            None
        };
        if let Some(interruption) = interruption {
            return Err(receipt.fail(
                interruption_error(interruption),
                InterfaceInvocationTerminal::Cancelled,
            ));
        }
        let Some(plan) = snapshot.plan(&binding_id).cloned() else {
            return Err(receipt.fail(
                InterfaceInvocationError::UnknownBinding,
                InterfaceInvocationTerminal::Rejected,
            ));
        };
        let definition = plan.definition().clone();
        let binding = plan.binding().clone();
        receipt.resolve(&plan);
        receipt.stage(InterfaceInvocationStage::Resolved);
        if binding.projection().protocol() != protocol {
            return Err(receipt.fail(
                InterfaceInvocationError::ProtocolBindingMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if plan.adapter_plan().authentication() != &authentication_adapter {
            return Err(receipt.fail(
                InterfaceInvocationError::AuthenticationAdapterMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if plan.adapter_plan().authorization() != &self.authorization.adapter_reference() {
            return Err(receipt.fail(
                InterfaceInvocationError::AuthorizationAdapterMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        let admission_reference = self
            .target_admission
            .as_ref()
            .map(|admission| admission.adapter_reference());
        if plan.adapter_plan().admission() != admission_reference.as_ref() {
            return Err(receipt.fail(
                InterfaceInvocationError::AdmissionAdapterMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if definition.principal_profile() != P::PROFILE {
            return Err(receipt.fail(
                InterfaceInvocationError::PrincipalProfileMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        receipt.stage(InterfaceInvocationStage::PrincipalEstablished);
        if definition.input_contract() != &contract_identity::<I>()
            || definition.stream_event_contract() != Some(&contract_identity::<S>())
            || definition.output_contract() != &contract_identity::<O>()
            || definition.target_error_contract() != &contract_identity::<E>()
        {
            return Err(receipt.fail(
                InterfaceInvocationError::ContractMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        let authorization = await_in_flight(
            &controls,
            self.authorization
                .authorize(InterfaceAuthorizationRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
        )
        .await;
        match authorization {
            Err(interruption) => {
                return Err(receipt.fail(
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                ));
            }
            Ok(Err(error)) => {
                return Err(receipt.fail(
                    InterfaceInvocationError::AuthorizationRejected(error),
                    InterfaceInvocationTerminal::Rejected,
                ));
            }
            Ok(Ok(())) => receipt.stage(InterfaceInvocationStage::Authorized),
        }
        if let Some(target_admission) = &self.target_admission {
            let admission = await_in_flight(
                &controls,
                target_admission.admit(InterfaceTargetAdmissionRequest::new(
                    principal.summary(),
                    definition.clone(),
                    binding,
                    protocol,
                )),
            )
            .await;
            match admission {
                Err(interruption) => {
                    return Err(receipt.fail(
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    ));
                }
                Ok(Err(error)) => {
                    return Err(receipt.fail(
                        InterfaceInvocationError::AdmissionRejected(error),
                        InterfaceInvocationTerminal::Rejected,
                    ));
                }
                Ok(Ok(())) => {}
            }
        }
        receipt.stage(InterfaceInvocationStage::Admitted);
        receipt.stage(InterfaceInvocationStage::Prepared);
        let Some(handler) = snapshot.stream_handler::<I, S, O, E, P>(definition.interface_id())
        else {
            return Err(receipt.fail(
                InterfaceInvocationError::ContractMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        };
        let attempt = ExecutionAttempt::first(target);
        receipt.dispatch(attempt.clone());
        receipt.stage(InterfaceInvocationStage::Executing);
        let context = InterfaceHandlerContext::new(
            principal,
            attempt,
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
        let stream = match await_in_flight(&controls, handler.invoke_stream(context, input)).await {
            Err(interruption) => {
                return Err(receipt.fail(
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                ));
            }
            Ok(Err(error)) => {
                return Err(receipt.fail(target_error(error), InterfaceInvocationTerminal::Failed));
            }
            Ok(Ok(stream)) => stream,
        };
        let completion = InterfaceStreamCompletion {
            completion: Box::pin(async move {
                let terminal = match await_in_flight(&controls, stream.terminal).await {
                    Err(interruption) => {
                        return Err(receipt.fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        ));
                    }
                    Ok(Err(_)) => {
                        return Err(receipt.fail(
                            InterfaceInvocationError::TargetFailed(
                                InterfaceTargetError::classified("stream-terminal-missing"),
                            ),
                            InterfaceInvocationTerminal::Failed,
                        ));
                    }
                    Ok(Ok(terminal)) => terminal,
                };
                match terminal {
                    InterfaceStreamTerminal::Completed(_) => {
                        receipt.stage(InterfaceInvocationStage::PostProcessed);
                        Ok(InterfaceStreamTerminalOutcome {
                            terminal,
                            receipt: receipt.complete(),
                        })
                    }
                    InterfaceStreamTerminal::Failed(error) => {
                        Err(receipt.fail(target_error(error), InterfaceInvocationTerminal::Failed))
                    }
                    InterfaceStreamTerminal::Rejected { ref classification } => Err(receipt.fail(
                        InterfaceInvocationError::AuthorizationRejected(
                            InterfaceAuthorizationError::classified(classification),
                        ),
                        InterfaceInvocationTerminal::Rejected,
                    )),
                    InterfaceStreamTerminal::Cancelled => Err(receipt.fail(
                        InterfaceInvocationError::Cancelled,
                        InterfaceInvocationTerminal::Cancelled,
                    )),
                }
            }),
        };
        Ok(InterfaceStreamInvocation {
            events: stream.events,
            completion,
        })
    }

    async fn invoke_internal<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        hook_plan: Option<&TypedInterfaceHookPlan<I, O>>,
        dispatch_target: Option<ExecutionTargetPin>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        let InvocationEnvelope {
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            principal,
            controls,
            mut input,
        } = envelope;
        let mut receipt = ReceiptBuilder::new(
            &snapshot,
            lineage.invocation_id(),
            lineage.parent_invocation_id(),
            protocol,
            principal.summary(),
            &controls,
        );
        let hook_context = InterfaceHookContext::new(
            principal.summary(),
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
        let initial_interruption = if controls.cancellation.is_cancelled() {
            Some(InvocationInterruption::Cancelled)
        } else if controls
            .deadline
            .is_some_and(|deadline| deadline <= SystemTime::now())
        {
            Some(InvocationInterruption::DeadlineElapsed)
        } else {
            None
        };
        if let Some(interruption) = initial_interruption {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Cancelled,
            )
            .await;
            return Err(receipt.fail(
                interruption_error(interruption),
                InterfaceInvocationTerminal::Cancelled,
            ));
        }
        if hook_plan.is_some_and(|plan| plan.graph_fingerprint() != snapshot.graph_fingerprint()) {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::HookPlanFingerprintMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        let Some(plan) = snapshot.plan(&binding_id).cloned() else {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::UnknownBinding,
                InterfaceInvocationTerminal::Rejected,
            ));
        };
        let definition = plan.definition().clone();
        let binding = plan.binding().clone();
        receipt.resolve(&plan);
        receipt.stage(InterfaceInvocationStage::Resolved);
        if binding.projection().protocol() != protocol {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::ProtocolBindingMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if plan.adapter_plan().authentication() != &authentication_adapter {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::AuthenticationAdapterMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if plan.adapter_plan().authorization() != &self.authorization.adapter_reference() {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::AuthorizationAdapterMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        let admission_reference = self
            .target_admission
            .as_ref()
            .map(|admission| admission.adapter_reference());
        if plan.adapter_plan().admission() != admission_reference.as_ref() {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::AdmissionAdapterMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if hook_plan.is_some_and(|hooks| {
            hooks.extension_plan_fingerprint() != plan.extension_plan().fingerprint()
        }) {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::HookPlanFingerprintMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if definition.principal_profile() != P::PROFILE {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::PrincipalProfileMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        receipt.stage(InterfaceInvocationStage::PrincipalEstablished);
        if definition.input_contract() != &contract_identity::<I>()
            || definition.output_contract() != &contract_identity::<O>()
            || definition.target_error_contract() != &contract_identity::<E>()
        {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::ContractMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        let authorization = await_in_flight(
            &controls,
            self.authorization
                .authorize(InterfaceAuthorizationRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
        )
        .await;
        let authorization = match authorization {
            Ok(authorization) => authorization,
            Err(interruption) => {
                run_completion_hooks(
                    &controls,
                    hook_plan,
                    &hook_context,
                    InterfaceInvocationTerminal::Cancelled,
                )
                .await;
                return Err(receipt.fail(
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                ));
            }
        };
        if let Err(error) = authorization {
            run_failure_hooks(&controls, hook_plan, &hook_context, error.classification()).await;
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::AuthorizationRejected(error),
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        receipt.stage(InterfaceInvocationStage::Authorized);
        if let Some(target_admission) = &self.target_admission {
            let admission = await_in_flight(
                &controls,
                target_admission.admit(InterfaceTargetAdmissionRequest::new(
                    principal.summary(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
            )
            .await;
            let admission = match admission {
                Ok(admission) => admission,
                Err(interruption) => {
                    run_completion_hooks(
                        &controls,
                        hook_plan,
                        &hook_context,
                        InterfaceInvocationTerminal::Cancelled,
                    )
                    .await;
                    return Err(receipt.fail(
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    ));
                }
            };
            if let Err(error) = admission {
                run_failure_hooks(&controls, hook_plan, &hook_context, error.classification())
                    .await;
                run_completion_hooks(
                    &controls,
                    hook_plan,
                    &hook_context,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await;
                return Err(receipt.fail(
                    InterfaceInvocationError::AdmissionRejected(error),
                    InterfaceInvocationTerminal::Rejected,
                ));
            }
        }
        receipt.stage(InterfaceInvocationStage::Admitted);
        if let Some(hook_plan) = hook_plan {
            let before =
                await_in_flight(&controls, hook_plan.run_before(&hook_context, &mut input)).await;
            match before {
                Err(interruption) => {
                    run_completion_hooks(
                        &controls,
                        Some(hook_plan),
                        &hook_context,
                        InterfaceInvocationTerminal::Cancelled,
                    )
                    .await;
                    return Err(receipt.fail(
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    ));
                }
                Ok(Err(error)) => {
                    run_failure_hooks(
                        &controls,
                        Some(hook_plan),
                        &hook_context,
                        error.classification(),
                    )
                    .await;
                    run_completion_hooks(
                        &controls,
                        Some(hook_plan),
                        &hook_context,
                        InterfaceInvocationTerminal::Rejected,
                    )
                    .await;
                    return Err(receipt.fail(
                        InterfaceInvocationError::BeforeHookRejected(error),
                        InterfaceInvocationTerminal::Rejected,
                    ));
                }
                Ok(Ok(())) => {}
            }
        }
        receipt.stage(InterfaceInvocationStage::Prepared);
        let Some(handler) = snapshot.handler::<I, O, E, P>(definition.interface_id()) else {
            run_completion_hooks(
                &controls,
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::ContractMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        };
        let attempt = ExecutionAttempt::first(dispatch_target.unwrap_or_else(|| {
            ExecutionTargetPin::BuiltIn {
                handler: definition.handler_reference().clone(),
                target: definition.target_reference().clone(),
            }
        }));
        receipt.dispatch(attempt.clone());
        receipt.stage(InterfaceInvocationStage::Executing);
        let context = InterfaceHandlerContext::new(
            principal,
            attempt,
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
        let target = await_in_flight(&controls, handler.invoke(context, input)).await;
        let target = match target {
            Ok(target) => target,
            Err(interruption) => {
                run_completion_hooks(
                    &controls,
                    hook_plan,
                    &hook_context,
                    InterfaceInvocationTerminal::Cancelled,
                )
                .await;
                return Err(receipt.fail(
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                ));
            }
        };
        match target {
            Ok(value) => {
                if let Some(hook_plan) = hook_plan {
                    if let Err(interruption) =
                        await_in_flight(&controls, hook_plan.run_after(&hook_context, &value)).await
                    {
                        run_completion_hooks(
                            &controls,
                            Some(hook_plan),
                            &hook_context,
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await;
                        return Err(receipt.fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        ));
                    }
                    run_completion_hooks(
                        &controls,
                        Some(hook_plan),
                        &hook_context,
                        InterfaceInvocationTerminal::Completed,
                    )
                    .await;
                }
                receipt.stage(InterfaceInvocationStage::PostProcessed);
                Ok(InterfaceInvocationOutcome {
                    value,
                    receipt: receipt.complete(),
                })
            }
            Err(error) => {
                if let Some(hook_plan) = hook_plan {
                    run_failure_hooks(
                        &controls,
                        Some(hook_plan),
                        &hook_context,
                        error.classification(),
                    )
                    .await;
                    run_completion_hooks(
                        &controls,
                        Some(hook_plan),
                        &hook_context,
                        InterfaceInvocationTerminal::Failed,
                    )
                    .await;
                }
                Err(receipt.fail(target_error(error), InterfaceInvocationTerminal::Failed))
            }
        }
    }
}

async fn run_failure_hooks<I, O>(
    controls: &InvocationControls,
    hook_plan: Option<&TypedInterfaceHookPlan<I, O>>,
    context: &InterfaceHookContext,
    classification: &str,
) where
    I: InterfaceContract,
    O: InterfaceContract,
{
    if let Some(hook_plan) = hook_plan {
        let _ = await_in_flight(controls, hook_plan.run_failure(context, classification)).await;
    }
}

async fn run_completion_hooks<I, O>(
    controls: &InvocationControls,
    hook_plan: Option<&TypedInterfaceHookPlan<I, O>>,
    context: &InterfaceHookContext,
    terminal: InterfaceInvocationTerminal,
) where
    I: InterfaceContract,
    O: InterfaceContract,
{
    if let Some(hook_plan) = hook_plan {
        let _ = await_in_flight(controls, hook_plan.run_completion(context, terminal)).await;
    }
}

struct ReceiptBuilder {
    invocation_id: InvocationId,
    parent_invocation_id: Option<InvocationId>,
    interface_id: Option<InterfaceId>,
    resolved: Option<ResolvedInvocationPin>,
    protocol: InterfaceProtocol,
    principal: PrincipalSummary,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
    stages: Vec<InterfaceStageRecord>,
    attempt: Option<ExecutionAttempt>,
    idempotency_key: Option<IdempotencyKey>,
}

impl ReceiptBuilder {
    fn new(
        snapshot: &CompiledInterfaceRegistry,
        invocation_id: InvocationId,
        parent_invocation_id: Option<InvocationId>,
        protocol: InterfaceProtocol,
        principal: PrincipalSummary,
        controls: &InvocationControls,
    ) -> Self {
        Self {
            invocation_id,
            parent_invocation_id,
            interface_id: None,
            resolved: None,
            protocol,
            principal,
            graph_fingerprint: snapshot.graph_fingerprint().clone(),
            registry_fingerprint: snapshot.fingerprint().clone(),
            stages: vec![InterfaceStageRecord {
                stage: InterfaceInvocationStage::Received,
                at: SystemTime::now(),
            }],
            attempt: None,
            idempotency_key: controls.idempotency_key.clone(),
        }
    }

    fn stage(&mut self, stage: InterfaceInvocationStage) {
        self.stages.push(InterfaceStageRecord {
            stage,
            at: SystemTime::now(),
        });
    }

    fn resolve(&mut self, plan: &crate::CompiledInvocationPlan) {
        self.interface_id = Some(plan.definition().interface_id().clone());
        self.resolved = Some(ResolvedInvocationPin {
            interface_version: plan.definition().version().clone(),
            binding_id: plan.binding().binding_id().clone(),
            binding_fingerprint: plan.binding_fingerprint().clone(),
            plan_fingerprint: plan.fingerprint().clone(),
        });
    }

    fn dispatch(&mut self, attempt: ExecutionAttempt) {
        self.attempt = Some(attempt);
        self.stage(InterfaceInvocationStage::Dispatched);
    }

    fn complete(self) -> InterfaceInvocationReceipt {
        self.receipt(InterfaceInvocationTerminal::Completed)
    }

    fn fail(
        self,
        error: InterfaceInvocationError,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceInvocationFailure {
        InterfaceInvocationFailure {
            error,
            receipt: self.receipt(terminal),
        }
    }

    fn receipt(self, terminal: InterfaceInvocationTerminal) -> InterfaceInvocationReceipt {
        InterfaceInvocationReceipt {
            invocation_id: self.invocation_id,
            parent_invocation_id: self.parent_invocation_id,
            interface_id: self.interface_id,
            resolved: self.resolved,
            protocol: self.protocol,
            principal: self.principal,
            graph_fingerprint: self.graph_fingerprint,
            registry_fingerprint: self.registry_fingerprint,
            stages: self.stages,
            attempt: self.attempt,
            idempotency_key: self.idempotency_key,
            terminal,
            terminal_at: SystemTime::now(),
        }
    }
}

fn contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("typed interface contract constants must be valid identities")
}

fn target_error<E>(error: InterfaceTargetFailure<E>) -> InterfaceInvocationError
where
    E: InterfaceContract,
{
    let InterfaceTargetFailure {
        classification,
        error,
    } = error;
    InterfaceInvocationError::TargetFailed(InterfaceTargetError::with_source(
        classification.as_ref(),
        error,
    ))
}

fn interruption_error(interruption: InvocationInterruption) -> InterfaceInvocationError {
    match interruption {
        InvocationInterruption::DeadlineElapsed => InterfaceInvocationError::DeadlineElapsed,
        InvocationInterruption::Cancelled => InterfaceInvocationError::Cancelled,
    }
}
