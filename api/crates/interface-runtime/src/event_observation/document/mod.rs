//! Explicit host codecs for document operations. Independent of legacy contract projections.
use crate::{
    BindingId, InterfaceContract, InterfaceEventContext, InterfaceIdentity,
    InterfaceInvocationTerminal,
};
use serde_json::Value;
use std::{any::Any, future::Future, pin::Pin, sync::Arc};

mod registration;
mod session;
pub use registration::DocumentOperationRegistration;
pub(crate) use registration::{validate_descriptor, ErasedDocumentOperationRegistration};
pub use session::DocumentOperationSession;

/// Owner-declared document schemas, already filtered by the owner's field policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentOperationSchemas {
    pub request: Option<Value>,
    pub result: Option<Value>,
    pub event: Option<Value>,
}

/// Document identity is separate from the canonical interface and protocol binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentOperationDescriptor {
    pub operation_id: Arc<str>,
    pub binding_id: BindingId,
    pub interface_identity: InterfaceIdentity,
    /// Must match the generation of the compiled host registration.
    pub registration_generation: Arc<str>,
    /// Stable owner identity, including the actual model/application where applicable.
    pub target_identity: Arc<str>,
    /// Generation of the exact owner snapshot used to execute this operation.
    pub target_generation: Arc<str>,
    pub schemas: DocumentOperationSchemas,
}

/// Read-only metadata borrowed from the exact compiled binding. No callable selector or codec.
#[derive(Clone, Copy, Debug)]
pub struct DocumentOperationCapability<'a> {
    pub binding_id: &'a BindingId,
    pub interface_identity: &'a InterfaceIdentity,
    pub registration_generation: &'a str,
    pub dynamic: bool,
    pub observer_enabled: bool,
    pub static_descriptor: Option<&'a DocumentOperationDescriptor>,
}

/// An explicit codec over the real handler types; never a Serialize fallback.
pub trait DocumentOperationCodec<I: InterfaceContract, O: InterfaceContract>:
    Send + Sync + 'static
{
    fn project_input(&self, input: &I) -> Result<Value, Arc<str>>;
    fn project_output(&self, output: &O) -> Result<Value, Arc<str>>;
}

pub struct DocumentOperationProjection<I: InterfaceContract, O: InterfaceContract> {
    pub descriptor: DocumentOperationDescriptor,
    pub codec: Arc<dyn DocumentOperationCodec<I, O>>,
}
impl<I: InterfaceContract, O: InterfaceContract> Clone for DocumentOperationProjection<I, O> {
    fn clone(&self) -> Self {
        Self {
            descriptor: self.descriptor.clone(),
            codec: self.codec.clone(),
        }
    }
}

/// Trusted compiled owner, called with the actually resolved and authorized snapshot.
/// The owner validates target generation/scope and executes this same immutable snapshot.
/// Returning a descriptor does not change authorization, admission, handler or execution target.
pub trait DocumentOperationSelector<I: InterfaceContract, O: InterfaceContract>:
    Send + Sync + 'static
{
    fn select(
        &self,
        snapshot: &(dyn Any + Send + Sync),
    ) -> Result<DocumentOperationProjection<I, O>, Arc<str>>;
}

#[derive(Clone, Debug)]
pub enum DocumentOperationEvent {
    Before {
        context: InterfaceEventContext,
        descriptor: DocumentOperationDescriptor,
        input: Value,
    },
    Completion {
        context: InterfaceEventContext,
        descriptor: DocumentOperationDescriptor,
        input: Option<Value>,
        output: Option<Value>,
        terminal: InterfaceInvocationTerminal,
        classification: Option<Arc<str>>,
    },
}
impl DocumentOperationEvent {
    /// Includes frozen registry/graph/plan proof, sealed principal and opaque ancestry.
    pub fn context(&self) -> &InterfaceEventContext {
        match self {
            Self::Before { context, .. } | Self::Completion { context, .. } => context,
        }
    }
    pub fn descriptor(&self) -> &DocumentOperationDescriptor {
        match self {
            Self::Before { descriptor, .. } | Self::Completion { descriptor, .. } => descriptor,
        }
    }
}
pub type DocumentOperationObserverFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), Arc<str>>> + Send + 'a>>;
pub type DocumentOperationInterestFuture<'a> =
    Pin<Box<dyn Future<Output = Result<bool, Arc<str>>> + Send + 'a>>;
pub trait DocumentOperationObserver: Send + Sync + 'static {
    fn capture_context(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
    /// Invocation-local interest snapshot. Completion-only subscriptions count as interest.
    /// Called before any input/output codec; false skips projections and publications.
    fn interested<'a>(
        &'a self,
        _context: &'a InterfaceEventContext,
        _descriptor: &'a DocumentOperationDescriptor,
    ) -> DocumentOperationInterestFuture<'a> {
        Box::pin(async { Ok(true) })
    }
    fn observe(&self, event: DocumentOperationEvent) -> DocumentOperationObserverFuture<'_>;
}
