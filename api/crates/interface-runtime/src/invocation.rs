use crate::finalization::InvocationFinalization;
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
    AdmissionAdapterReference, ArtifactIdentity, AuthenticationActivationIdentity,
    AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationDecisionFingerprint, BindingFingerprint, BindingId, CompiledInterfaceRegistry,
    ContractIdentity, GraphFingerprint, HandlerReference, InterfaceAdmissionContributionError,
    InterfaceAdmissionContributionRequest, InterfaceAuthorizationContributionError,
    InterfaceAuthorizationContributionRequest, InterfaceContract, InterfaceDefinition,
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
    HostExtension {
        handler: HandlerReference,
        target: TargetReference,
        plugin: PluginIdentity,
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
    authentication_activation: AuthenticationActivationIdentity,
    principal: P,
    controls: InvocationControls,
    extension_context: Option<crate::hook::InvocationExtensionContext>,
    input: I,
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
    authorization_decision: Option<AuthorizationDecisionFingerprint>,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
    stages: Vec<InterfaceStageRecord>,
    attempt: Option<ExecutionAttempt>,
    idempotency_key: Option<IdempotencyKey>,
    terminal: InterfaceInvocationTerminal,
    terminal_at: SystemTime,
    observers: Vec<crate::InterfaceObserverRecord>,
}

impl InterfaceInvocationReceipt {
    pub fn observer_records(&self) -> &[crate::InterfaceObserverRecord] {
        &self.observers
    }

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

    pub fn authorization_decision(&self) -> Option<&AuthorizationDecisionFingerprint> {
        self.authorization_decision.as_ref()
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
    #[error("authentication activation does not match the compiled invocation plan")]
    AuthenticationActivationMismatch,
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
    AuthorizationContributionRejected(InterfaceAuthorizationContributionError),
    #[error(transparent)]
    AdmissionRejected(InterfaceTargetAdmissionError),
    #[error(transparent)]
    AdmissionContributionRejected(InterfaceAdmissionContributionError),
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
    error: Box<InterfaceInvocationError>,
    receipt: Box<InterfaceInvocationReceipt>,
}

impl InterfaceInvocationFailure {
    pub fn error(&self) -> &InterfaceInvocationError {
        &self.error
    }

    pub fn receipt(&self) -> &InterfaceInvocationReceipt {
        &self.receipt
    }

    pub fn into_error(self) -> InterfaceInvocationError {
        *self.error
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

async fn run_failure_hooks<I, O>(
    budget: &mut InvocationFinalization,
    hook_plan: Option<&TypedInterfaceHookPlan<I, O>>,
    context: &InterfaceHookContext,
    classification: &str,
) where
    I: InterfaceContract,
    O: InterfaceContract,
{
    if let Some(hook_plan) = hook_plan {
        hook_plan.run_failure(budget, context, classification).await;
    }
}

struct ReceiptBuilder {
    managed_invocation: Option<crate::hook::ManagedInvocationContext>,
    extension_context: Option<crate::hook::InvocationExtensionContext>,
    finalization: InvocationFinalization,
    frozen_plan: Option<crate::CompiledInvocationPlan>,
    invocation_id: InvocationId,
    parent_invocation_id: Option<InvocationId>,
    interface_id: Option<InterfaceId>,
    resolved: Option<ResolvedInvocationPin>,
    protocol: InterfaceProtocol,
    principal: PrincipalSummary,
    authorization_decision: Option<AuthorizationDecisionFingerprint>,
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
            managed_invocation: None,
            extension_context: None,
            finalization: InvocationFinalization::default(),
            frozen_plan: None,
            invocation_id,
            parent_invocation_id,
            interface_id: None,
            resolved: None,
            protocol,
            principal,
            authorization_decision: None,
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

    fn authorized(&mut self, fingerprint: AuthorizationDecisionFingerprint) {
        self.authorization_decision = Some(fingerprint);
        self.stage(InterfaceInvocationStage::Authorized);
    }

    fn resolve(&mut self, plan: &crate::CompiledInvocationPlan) {
        self.frozen_plan = Some(plan.clone());
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

    async fn complete(mut self) -> InterfaceInvocationReceipt {
        self.finalize(InterfaceInvocationTerminal::Completed).await;
        self.receipt(InterfaceInvocationTerminal::Completed)
    }

    fn fail(
        mut self,
        error: InterfaceInvocationError,
        terminal: InterfaceInvocationTerminal,
    ) -> Pin<Box<impl Future<Output = InterfaceInvocationFailure> + Send>> {
        // Unary preparation has many mutually exclusive rejection branches.
        // Keep their finalization state out of its poll frame, including when
        // preparation is reached through a same-task MCP callable invocation.
        Box::pin(async move {
            self.finalize(terminal).await;
            InterfaceInvocationFailure {
                error: Box::new(error),
                receipt: Box::new(self.receipt(terminal)),
            }
        })
    }

    async fn finalize(&mut self, terminal: InterfaceInvocationTerminal) {
        if let Some(hooks) = self
            .frozen_plan
            .as_ref()
            .and_then(|plan| plan.erased_hook_plan())
        {
            let mut context = InterfaceHookContext::new(
                self.principal.clone(),
                self.invocation_id,
                self.graph_fingerprint.clone(),
                self.registry_fingerprint.clone(),
            )
            .with_extension_context(self.extension_context.clone());
            context.managed_invocation = self.managed_invocation.clone();
            hooks
                .run_completion(&mut self.finalization, &context, terminal)
                .await;
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
            authorization_decision: self.authorization_decision,
            graph_fingerprint: self.graph_fingerprint,
            registry_fingerprint: self.registry_fingerprint,
            stages: self.stages,
            attempt: self.attempt,
            idempotency_key: self.idempotency_key,
            terminal,
            terminal_at: SystemTime::now(),
            observers: self.finalization.records,
        }
    }
}

fn contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("typed interface contract constants must be valid identities")
}

fn authorization_decision_fingerprint(
    plan: &crate::CompiledInvocationPlan,
) -> AuthorizationDecisionFingerprint {
    AuthorizationDecisionFingerprint::new(format!("{}:allow", plan.fingerprint().as_str()))
        .expect("compiled plan fingerprint must produce a valid authorization decision identity")
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

mod kernel;

#[path = "invocation/invocation_envelope.rs"]
mod invocation_envelope;
