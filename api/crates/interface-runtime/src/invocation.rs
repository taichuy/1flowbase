use std::{future::Future, pin::Pin, sync::Arc, time::SystemTime};

use domain::ActorContext;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, InterfaceContract,
    InterfaceDefinition, InterfaceHandlerContext, InterfaceHookContext, InterfaceId,
    InterfaceTargetError, ProtocolBinding, RegistryFingerprint, TypedInterfaceHookPlan,
};

async fn await_before_deadline<T>(
    deadline: Option<SystemTime>,
    future: impl Future<Output = T>,
) -> Result<T, ()> {
    let Some(deadline) = deadline else {
        return Ok(future.await);
    };
    let remaining = deadline.duration_since(SystemTime::now()).map_err(|_| ())?;
    tokio::time::timeout(remaining, future)
        .await
        .map_err(|_| ())
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
}

pub struct InvocationEnvelope<I>
where
    I: InterfaceContract,
{
    lineage: InvocationLineage,
    interface_id: InterfaceId,
    protocol: InterfaceProtocol,
    actor: ActorContext,
    deadline: Option<SystemTime>,
    input: I,
}

impl<I> InvocationEnvelope<I>
where
    I: InterfaceContract,
{
    pub fn new(
        lineage: InvocationLineage,
        interface_id: InterfaceId,
        protocol: InterfaceProtocol,
        actor: ActorContext,
        deadline: Option<SystemTime>,
        input: I,
    ) -> Self {
        Self {
            lineage,
            interface_id,
            protocol,
            actor,
            deadline,
            input,
        }
    }

    pub fn lineage(&self) -> &InvocationLineage {
        &self.lineage
    }

    pub fn interface_id(&self) -> &InterfaceId {
        &self.interface_id
    }

    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }

    pub fn actor(&self) -> &ActorContext {
        &self.actor
    }
}

#[derive(Clone, Debug)]
pub struct InterfaceAuthorizationRequest {
    actor: ActorContext,
    definition: InterfaceDefinition,
    binding: ProtocolBinding,
    protocol: InterfaceProtocol,
}

impl InterfaceAuthorizationRequest {
    fn new(
        actor: ActorContext,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            actor,
            definition,
            binding,
            protocol,
        }
    }

    pub fn actor(&self) -> &ActorContext {
        &self.actor
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

pub trait InterfaceAuthorizationPort: Send + Sync + 'static {
    fn authorize(&self, request: InterfaceAuthorizationRequest)
        -> InterfaceAuthorizationFuture<'_>;
}

#[derive(Clone, Debug)]
pub struct InterfaceTargetAdmissionRequest {
    actor: ActorContext,
    definition: InterfaceDefinition,
    binding: ProtocolBinding,
    protocol: InterfaceProtocol,
}

impl InterfaceTargetAdmissionRequest {
    fn new(
        actor: ActorContext,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            actor,
            definition,
            binding,
            protocol,
        }
    }

    pub fn actor(&self) -> &ActorContext {
        &self.actor
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
    fn admit(&self, request: InterfaceTargetAdmissionRequest)
        -> InterfaceTargetAdmissionFuture<'_>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceInvocationStage {
    Resolved,
    Authorized,
    Admitted,
    BeforeHooksCompleted,
    Invoking,
    AfterHooksCompleted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceInvocationTerminal {
    Completed,
    Failed,
    Rejected,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceInvocationReceipt {
    invocation_id: InvocationId,
    parent_invocation_id: Option<InvocationId>,
    interface_id: InterfaceId,
    protocol: InterfaceProtocol,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
    stages: Vec<InterfaceInvocationStage>,
    terminal: InterfaceInvocationTerminal,
}

impl InterfaceInvocationReceipt {
    pub fn invocation_id(&self) -> InvocationId {
        self.invocation_id
    }

    pub fn parent_invocation_id(&self) -> Option<InvocationId> {
        self.parent_invocation_id
    }

    pub fn interface_id(&self) -> &InterfaceId {
        &self.interface_id
    }

    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }

    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn registry_fingerprint(&self) -> &RegistryFingerprint {
        &self.registry_fingerprint
    }

    pub fn stages(&self) -> &[InterfaceInvocationStage] {
        &self.stages
    }

    pub fn terminal(&self) -> InterfaceInvocationTerminal {
        self.terminal
    }
}

#[derive(Debug, Error)]
pub enum InterfaceInvocationError {
    #[error("interface is not registered")]
    UnknownInterface,
    #[error("invocation contract does not match the registered interface")]
    ContractMismatch,
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

pub struct InterfaceInvocationKernel {
    authorization: Arc<dyn InterfaceAuthorizationPort>,
    target_admission: Option<Arc<dyn InterfaceTargetAdmissionPort>>,
}

impl InterfaceInvocationKernel {
    pub fn new(authorization: Arc<dyn InterfaceAuthorizationPort>) -> Self {
        Self {
            authorization,
            target_admission: None,
        }
    }

    pub fn with_target_admission(
        authorization: Arc<dyn InterfaceAuthorizationPort>,
        target_admission: Arc<dyn InterfaceTargetAdmissionPort>,
    ) -> Self {
        Self {
            authorization,
            target_admission: Some(target_admission),
        }
    }

    pub async fn invoke<I, O>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        self.invoke_internal(snapshot, envelope, None).await
    }

    pub async fn invoke_with_hook_plan<I, O>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I>,
        hook_plan: &TypedInterfaceHookPlan<I, O>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        self.invoke_internal(snapshot, envelope, Some(hook_plan))
            .await
    }

    async fn invoke_internal<I, O>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I>,
        hook_plan: Option<&TypedInterfaceHookPlan<I, O>>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
    {
        let InvocationEnvelope {
            lineage,
            interface_id,
            protocol,
            actor,
            deadline,
            mut input,
        } = envelope;
        let mut receipt = ReceiptBuilder::new(
            &snapshot,
            lineage.invocation_id(),
            lineage.parent_invocation_id(),
            interface_id.clone(),
            protocol,
        );
        let hook_context = InterfaceHookContext::new(
            actor.clone(),
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
        if deadline.is_some_and(|deadline| deadline <= SystemTime::now()) {
            run_completion_hooks(
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Cancelled,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::DeadlineElapsed,
                InterfaceInvocationTerminal::Cancelled,
            ));
        }
        if hook_plan.is_some_and(|plan| plan.graph_fingerprint() != snapshot.graph_fingerprint()) {
            run_completion_hooks(
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
        let Some(plan) = snapshot.plan_for_interface(&interface_id).cloned() else {
            run_completion_hooks(
                hook_plan,
                &hook_context,
                InterfaceInvocationTerminal::Rejected,
            )
            .await;
            return Err(receipt.fail(
                InterfaceInvocationError::UnknownInterface,
                InterfaceInvocationTerminal::Rejected,
            ));
        };
        let definition = plan.definition().clone();
        let binding = plan.binding().clone();
        receipt.stage(InterfaceInvocationStage::Resolved);
        if definition.input_contract() != &contract_identity::<I>()
            || definition.output_contract() != &contract_identity::<O>()
        {
            run_completion_hooks(
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
        let authorization = await_before_deadline(
            deadline,
            self.authorization
                .authorize(InterfaceAuthorizationRequest::new(
                    actor.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
        )
        .await;
        let authorization = match authorization {
            Ok(authorization) => authorization,
            Err(()) => {
                run_completion_hooks(
                    hook_plan,
                    &hook_context,
                    InterfaceInvocationTerminal::Cancelled,
                )
                .await;
                return Err(receipt.fail(
                    InterfaceInvocationError::DeadlineElapsed,
                    InterfaceInvocationTerminal::Cancelled,
                ));
            }
        };
        if let Err(error) = authorization {
            run_failure_hooks(hook_plan, &hook_context, error.classification()).await;
            run_completion_hooks(
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
            let admission = await_before_deadline(
                deadline,
                target_admission.admit(InterfaceTargetAdmissionRequest::new(
                    actor.clone(),
                    definition,
                    binding,
                    protocol,
                )),
            )
            .await;
            let admission = match admission {
                Ok(admission) => admission,
                Err(()) => {
                    run_completion_hooks(
                        hook_plan,
                        &hook_context,
                        InterfaceInvocationTerminal::Cancelled,
                    )
                    .await;
                    return Err(receipt.fail(
                        InterfaceInvocationError::DeadlineElapsed,
                        InterfaceInvocationTerminal::Cancelled,
                    ));
                }
            };
            if let Err(error) = admission {
                run_failure_hooks(hook_plan, &hook_context, error.classification()).await;
                run_completion_hooks(
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
            receipt.stage(InterfaceInvocationStage::Admitted);
        }
        if let Some(hook_plan) = hook_plan {
            let before =
                await_before_deadline(deadline, hook_plan.run_before(&hook_context, &mut input))
                    .await;
            match before {
                Err(()) => {
                    hook_plan
                        .run_completion(&hook_context, InterfaceInvocationTerminal::Cancelled)
                        .await;
                    return Err(receipt.fail(
                        InterfaceInvocationError::DeadlineElapsed,
                        InterfaceInvocationTerminal::Cancelled,
                    ));
                }
                Ok(Err(error)) => {
                    hook_plan
                        .run_failure(&hook_context, error.classification())
                        .await;
                    hook_plan
                        .run_completion(&hook_context, InterfaceInvocationTerminal::Rejected)
                        .await;
                    return Err(receipt.fail(
                        InterfaceInvocationError::BeforeHookRejected(error),
                        InterfaceInvocationTerminal::Rejected,
                    ));
                }
                Ok(Ok(())) => receipt.stage(InterfaceInvocationStage::BeforeHooksCompleted),
            }
        }
        let Some(handler) = snapshot.handler::<I, O>(&interface_id) else {
            run_completion_hooks(
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
        receipt.stage(InterfaceInvocationStage::Invoking);
        let context = InterfaceHandlerContext::new(
            actor,
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
        let target = await_before_deadline(deadline, handler.invoke(context, input)).await;
        let target = match target {
            Ok(target) => target,
            Err(()) => {
                run_completion_hooks(
                    hook_plan,
                    &hook_context,
                    InterfaceInvocationTerminal::Cancelled,
                )
                .await;
                return Err(receipt.fail(
                    InterfaceInvocationError::DeadlineElapsed,
                    InterfaceInvocationTerminal::Cancelled,
                ));
            }
        };
        match target {
            Ok(value) => {
                if let Some(hook_plan) = hook_plan {
                    hook_plan.run_after(&hook_context, &value).await;
                    receipt.stage(InterfaceInvocationStage::AfterHooksCompleted);
                    hook_plan
                        .run_completion(&hook_context, InterfaceInvocationTerminal::Completed)
                        .await;
                }
                Ok(InterfaceInvocationOutcome {
                    value,
                    receipt: receipt.complete(),
                })
            }
            Err(error) => {
                if let Some(hook_plan) = hook_plan {
                    hook_plan
                        .run_failure(&hook_context, error.classification())
                        .await;
                    hook_plan
                        .run_completion(&hook_context, InterfaceInvocationTerminal::Failed)
                        .await;
                }
                Err(receipt.fail(target_error(error), InterfaceInvocationTerminal::Failed))
            }
        }
    }
}

async fn run_failure_hooks<I, O>(
    hook_plan: Option<&TypedInterfaceHookPlan<I, O>>,
    context: &InterfaceHookContext,
    classification: &str,
) where
    I: InterfaceContract,
    O: InterfaceContract,
{
    if let Some(hook_plan) = hook_plan {
        hook_plan.run_failure(context, classification).await;
    }
}

async fn run_completion_hooks<I, O>(
    hook_plan: Option<&TypedInterfaceHookPlan<I, O>>,
    context: &InterfaceHookContext,
    terminal: InterfaceInvocationTerminal,
) where
    I: InterfaceContract,
    O: InterfaceContract,
{
    if let Some(hook_plan) = hook_plan {
        hook_plan.run_completion(context, terminal).await;
    }
}

struct ReceiptBuilder {
    invocation_id: InvocationId,
    parent_invocation_id: Option<InvocationId>,
    interface_id: InterfaceId,
    protocol: InterfaceProtocol,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
    stages: Vec<InterfaceInvocationStage>,
}

impl ReceiptBuilder {
    fn new(
        snapshot: &CompiledInterfaceRegistry,
        invocation_id: InvocationId,
        parent_invocation_id: Option<InvocationId>,
        interface_id: InterfaceId,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            invocation_id,
            parent_invocation_id,
            interface_id,
            protocol,
            graph_fingerprint: snapshot.graph_fingerprint().clone(),
            registry_fingerprint: snapshot.fingerprint().clone(),
            stages: Vec::new(),
        }
    }

    fn stage(&mut self, stage: InterfaceInvocationStage) {
        self.stages.push(stage);
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
            protocol: self.protocol,
            graph_fingerprint: self.graph_fingerprint,
            registry_fingerprint: self.registry_fingerprint,
            stages: self.stages,
            terminal,
        }
    }
}

fn contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("typed interface contract constants must be valid identities")
}

fn target_error(error: InterfaceTargetError) -> InterfaceInvocationError {
    InterfaceInvocationError::TargetFailed(error)
}
