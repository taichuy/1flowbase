//! Trusted host observation, independent of managed plugin hooks and subscriptions.
use std::{any::Any, future::Future, pin::Pin, sync::Arc};

use serde_json::Value;

use crate::{
    ApplicationPrincipal, ExecutionAttempt, GraphFingerprint, InterfaceDefinition,
    InterfaceInvocationTerminal, InvocationId, InvocationPrincipal, PrincipalSummary,
    ProtocolBinding, PublicPrincipal, RegistryFingerprint, ResolvedInvocationPin, UserPrincipal,
};

/// A declared operation capability. Schemas are provided by typed owners, never samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceEventCapability {
    definition: InterfaceDefinition,
    input_schema: Value,
    output_schema: Value,
}
impl InterfaceEventCapability {
    pub(crate) fn new(
        definition: InterfaceDefinition,
        input_schema: Value,
        output_schema: Value,
    ) -> Self {
        Self {
            definition,
            input_schema,
            output_schema,
        }
    }
    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }
    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }
    pub fn output_schema(&self) -> &Value {
        &self.output_schema
    }
}

/// Sealed identities retained only inside the trusted host. Never serialized to event payloads.
#[derive(Clone, Debug)]
pub enum InterfaceEventPrincipal {
    Public(PublicPrincipal),
    User(UserPrincipal),
    Application(ApplicationPrincipal),
}
impl InterfaceEventPrincipal {
    pub(crate) fn from_principal<P: InvocationPrincipal>(principal: &P) -> Option<Self> {
        let principal = principal as &dyn Any;
        if let Some(principal) = principal.downcast_ref::<PublicPrincipal>() {
            Some(Self::Public(*principal))
        } else if let Some(principal) = principal.downcast_ref::<UserPrincipal>() {
            Some(Self::User(principal.clone()))
        } else {
            principal
                .downcast_ref::<ApplicationPrincipal>()
                .cloned()
                .map(Self::Application)
        }
    }
    pub fn summary(&self) -> PrincipalSummary {
        match self {
            Self::Public(principal) => principal.summary(),
            Self::User(principal) => principal.summary(),
            Self::Application(principal) => principal.summary(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct InterfaceEventContext {
    pub(crate) principal: Option<InterfaceEventPrincipal>,
    pub(crate) definition: InterfaceDefinition,
    pub(crate) binding: ProtocolBinding,
    pub(crate) resolved: ResolvedInvocationPin,
    pub(crate) invocation_id: InvocationId,
    pub(crate) parent_invocation_id: Option<InvocationId>,
    pub(crate) graph_fingerprint: GraphFingerprint,
    pub(crate) registry_fingerprint: RegistryFingerprint,
    pub(crate) attempt: Option<ExecutionAttempt>,
    pub(crate) extension_context: Option<crate::hook::InvocationExtensionContext>,
    pub(crate) observer_context: Option<crate::hook::InvocationExtensionContext>,
}
impl InterfaceEventContext {
    /// None denotes rejection before authentication established a principal.
    pub fn principal(&self) -> Option<&InterfaceEventPrincipal> {
        self.principal.as_ref()
    }
    pub fn definition(&self) -> &InterfaceDefinition {
        &self.definition
    }
    pub fn binding(&self) -> &ProtocolBinding {
        &self.binding
    }
    pub fn resolved(&self) -> &ResolvedInvocationPin {
        &self.resolved
    }
    pub fn invocation_id(&self) -> InvocationId {
        self.invocation_id
    }
    pub fn parent_invocation_id(&self) -> Option<InvocationId> {
        self.parent_invocation_id
    }
    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }
    pub fn registry_fingerprint(&self) -> &RegistryFingerprint {
        &self.registry_fingerprint
    }
    pub fn attempt(&self) -> Option<&ExecutionAttempt> {
        self.attempt.as_ref()
    }
    /// Invocation-owned snapshot of host observation metadata, retained through stream terminal.
    pub fn observer_context<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.observer_context.as_ref()?.0.downcast_ref()
    }
    pub fn extension_context<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.extension_context.as_ref()?.0.downcast_ref()
    }
}

#[derive(Clone, Debug)]
pub enum InterfaceEvent {
    Before {
        context: InterfaceEventContext,
        input: Value,
    },
    Completion {
        context: InterfaceEventContext,
        /// Available only after the invocation passed authorization/admission and typed Before.
        input: Option<Value>,
        /// Only a real successful typed result can provide output, including for streams.
        output: Option<Value>,
        terminal: InterfaceInvocationTerminal,
        classification: Option<Arc<str>>,
    },
}
impl InterfaceEvent {
    pub fn context(&self) -> &InterfaceEventContext {
        match self {
            Self::Before { context, .. } | Self::Completion { context, .. } => context,
        }
    }
}

pub type InterfaceEventObserverFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), Arc<str>>> + Send + 'a>>;

/// Host-only read observation. It cannot mutate input, veto, or replace the primary result.
pub trait InterfaceEventObserver: Send + Sync + 'static {
    /// Captured once when resolving an invocation, before task-local context can be lost.
    /// The kernel treats it as opaque host metadata; it is never serialized.
    fn capture_context(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }

    fn observe(&self, event: InterfaceEvent) -> InterfaceEventObserverFuture<'_>;
}

pub(crate) async fn observe_event(
    budget: &mut crate::finalization::InvocationFinalization,
    observer: &dyn InterfaceEventObserver,
    event: InterfaceEvent,
) {
    let identity = crate::PluginIdentity::new("interface-runtime.host-events")
        .expect("static host observer identity");
    let point = match &event {
        InterfaceEvent::Before { .. } => crate::InterfaceExtensionPoint::Before,
        InterfaceEvent::Completion { .. } => crate::InterfaceExtensionPoint::Completion,
    };
    let mut failed = false;
    let token = budget
        .observe(&identity, point, || async {
            failed = observer.observe(event).await.is_err();
        })
        .await;
    if failed {
        budget.record_reported_failure(token);
    }
}

pub(crate) mod document;
