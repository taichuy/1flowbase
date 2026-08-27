use std::{future::Future, pin::Pin, sync::Arc, time::SystemTime};

use domain::ActorContext;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    CompiledInterfaceRegistry, ContractIdentity, GraphFingerprint, InterfaceContract,
    InterfaceDefinition, InterfaceHandlerContext, InterfaceId, InterfaceTargetError,
    RegistryFingerprint,
};

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
    protocol: InterfaceProtocol,
}

impl InterfaceAuthorizationRequest {
    fn new(
        actor: ActorContext,
        definition: InterfaceDefinition,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            actor,
            definition,
            protocol,
        }
    }

    pub fn actor(&self) -> &ActorContext {
        &self.actor
    }

    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
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
    protocol: InterfaceProtocol,
}

impl InterfaceTargetAdmissionRequest {
    fn new(
        actor: ActorContext,
        definition: InterfaceDefinition,
        protocol: InterfaceProtocol,
    ) -> Self {
        Self {
            actor,
            definition,
            protocol,
        }
    }

    pub fn actor(&self) -> &ActorContext {
        &self.actor
    }

    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
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
    Invoking,
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
    #[error(transparent)]
    AuthorizationRejected(InterfaceAuthorizationError),
    #[error(transparent)]
    AdmissionRejected(InterfaceTargetAdmissionError),
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
        let InvocationEnvelope {
            lineage,
            interface_id,
            protocol,
            actor,
            deadline,
            input,
        } = envelope;
        let mut receipt = ReceiptBuilder::new(
            &snapshot,
            lineage.invocation_id(),
            lineage.parent_invocation_id(),
            interface_id.clone(),
            protocol,
        );
        if deadline.is_some_and(|deadline| deadline <= SystemTime::now()) {
            return Err(receipt.fail(
                InterfaceInvocationError::DeadlineElapsed,
                InterfaceInvocationTerminal::Cancelled,
            ));
        }
        let Some(definition) = snapshot.definition(&interface_id).cloned() else {
            return Err(receipt.fail(
                InterfaceInvocationError::UnknownInterface,
                InterfaceInvocationTerminal::Rejected,
            ));
        };
        receipt.stage(InterfaceInvocationStage::Resolved);
        if definition.input_contract() != &contract_identity::<I>()
            || definition.output_contract() != &contract_identity::<O>()
        {
            return Err(receipt.fail(
                InterfaceInvocationError::ContractMismatch,
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        if let Err(error) = self
            .authorization
            .authorize(InterfaceAuthorizationRequest::new(
                actor.clone(),
                definition.clone(),
                protocol,
            ))
            .await
        {
            return Err(receipt.fail(
                InterfaceInvocationError::AuthorizationRejected(error),
                InterfaceInvocationTerminal::Rejected,
            ));
        }
        receipt.stage(InterfaceInvocationStage::Authorized);
        if let Some(target_admission) = &self.target_admission {
            if let Err(error) = target_admission
                .admit(InterfaceTargetAdmissionRequest::new(
                    actor.clone(),
                    definition,
                    protocol,
                ))
                .await
            {
                return Err(receipt.fail(
                    InterfaceInvocationError::AdmissionRejected(error),
                    InterfaceInvocationTerminal::Rejected,
                ));
            }
            receipt.stage(InterfaceInvocationStage::Admitted);
        }
        if deadline.is_some_and(|deadline| deadline <= SystemTime::now()) {
            return Err(receipt.fail(
                InterfaceInvocationError::DeadlineElapsed,
                InterfaceInvocationTerminal::Cancelled,
            ));
        }
        let Some(handler) = snapshot.handler::<I, O>(&interface_id) else {
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
        match handler.invoke(context, input).await {
            Ok(value) => Ok(InterfaceInvocationOutcome {
                value,
                receipt: receipt.complete(),
            }),
            Err(error) => {
                Err(receipt.fail(target_error(error), InterfaceInvocationTerminal::Failed))
            }
        }
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
