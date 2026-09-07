use crate::{
    AuthenticationActivationIdentity, AuthenticationAdapterReference, InterfaceExtensionTier,
    PluginIdentity, PrincipalProfile,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivatedAuthenticationAdapter {
    plugin: PluginIdentity,
    tier: InterfaceExtensionTier,
    adapter: AuthenticationAdapterReference,
    activation: AuthenticationActivationIdentity,
    principal_profile: PrincipalProfile,
}

impl ActivatedAuthenticationAdapter {
    pub fn new(
        plugin: PluginIdentity,
        tier: InterfaceExtensionTier,
        adapter: AuthenticationAdapterReference,
        activation: AuthenticationActivationIdentity,
        principal_profile: PrincipalProfile,
    ) -> Self {
        Self {
            plugin,
            tier,
            adapter,
            activation,
            principal_profile,
        }
    }

    pub fn plugin(&self) -> &PluginIdentity {
        &self.plugin
    }

    pub fn tier(&self) -> InterfaceExtensionTier {
        self.tier
    }

    pub fn adapter(&self) -> &AuthenticationAdapterReference {
        &self.adapter
    }

    pub fn activation(&self) -> &AuthenticationActivationIdentity {
        &self.activation
    }

    pub fn principal_profile(&self) -> PrincipalProfile {
        self.principal_profile
    }
}

use crate::finalization::InvocationFinalization;
use crate::{
    BindingId, CompiledInterfaceRegistry, CompiledInvocationPlan, GraphFingerprint,
    InterfaceContract, InterfaceHookContext, InterfaceId, InterfaceInvocationTerminal,
    InterfaceObserverRecord, InterfaceProtocol, InvocationEnvelope, InvocationId,
    InvocationLineage, InvocationPrincipal, PlanFingerprint, PrincipalSummary, RegistryFingerprint,
};
use std::{sync::Arc, time::SystemTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterfaceAuthenticationRejectionClass {
    CredentialRejected,
    AuthenticationFailed,
}
impl InterfaceAuthenticationRejectionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CredentialRejected => "credential-rejected",
            Self::AuthenticationFailed => "authentication-failed",
        }
    }
}

/// An authentication attempt pinned before credentials enter the adapter. Contains no credentials.
pub struct InterfaceAuthenticationAttempt {
    registry: Arc<CompiledInterfaceRegistry>,
    plan: CompiledInvocationPlan,
    lineage: InvocationLineage,
    protocol: InterfaceProtocol,
    received_at: SystemTime,
}
impl InterfaceAuthenticationAttempt {
    pub fn resolve(
        registry: Arc<CompiledInterfaceRegistry>,
        binding: &BindingId,
        protocol: InterfaceProtocol,
        lineage: InvocationLineage,
    ) -> Option<Self> {
        let plan = registry.plan(binding)?.clone();
        Some(Self {
            registry,
            plan,
            lineage,
            protocol,
            received_at: SystemTime::now(),
        })
    }
    pub fn activation(&self) -> &ActivatedAuthenticationAdapter {
        self.plan.authentication()
    }
    pub fn lineage(&self) -> &InvocationLineage {
        &self.lineage
    }
    pub fn into_envelope<I: InterfaceContract, P: InvocationPrincipal>(
        self,
        principal: P,
        input: I,
    ) -> InvocationEnvelope<I, P> {
        InvocationEnvelope::with_principal(
            self.lineage,
            self.plan.binding().binding_id().clone(),
            self.protocol,
            self.plan.authentication().adapter().clone(),
            self.plan.authentication().activation().clone(),
            principal,
            None,
            input,
        )
    }
    pub async fn reject(
        self,
        classification: InterfaceAuthenticationRejectionClass,
    ) -> InterfaceAuthenticationRejectionReceipt {
        let rejected_at = SystemTime::now();
        let context = InterfaceHookContext::unestablished(
            self.lineage.invocation_id(),
            self.registry.graph_fingerprint().clone(),
            self.registry.fingerprint().clone(),
        );
        let mut finalization = InvocationFinalization::default();
        if let Some(hooks) = self.plan.erased_hook_plan() {
            hooks
                .run_completion(
                    &mut finalization,
                    &context,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await;
        }
        InterfaceAuthenticationRejectionReceipt {
            invocation_id: self.lineage.invocation_id(),
            parent_invocation_id: self.lineage.parent_invocation_id(),
            binding_id: self.plan.binding().binding_id().clone(),
            interface_id: self.plan.definition().interface_id().clone(),
            plan_fingerprint: self.plan.fingerprint().clone(),
            activation: self.plan.authentication().clone(),
            graph_fingerprint: self.registry.graph_fingerprint().clone(),
            registry_fingerprint: self.registry.fingerprint().clone(),
            protocol: self.protocol,
            received_at: self.received_at,
            rejected_at,
            classification,
            observers: finalization.records,
        }
    }
}

/// Safe observation only; the original authentication error remains in its host adapter.
#[derive(Clone, Debug)]
pub struct InterfaceAuthenticationRejectionReceipt {
    invocation_id: InvocationId,
    parent_invocation_id: Option<InvocationId>,
    binding_id: BindingId,
    interface_id: InterfaceId,
    plan_fingerprint: PlanFingerprint,
    activation: ActivatedAuthenticationAdapter,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
    protocol: InterfaceProtocol,
    received_at: SystemTime,
    rejected_at: SystemTime,
    classification: InterfaceAuthenticationRejectionClass,
    observers: Vec<InterfaceObserverRecord>,
}
impl InterfaceAuthenticationRejectionReceipt {
    pub fn invocation_id(&self) -> InvocationId {
        self.invocation_id
    }
    pub fn parent_invocation_id(&self) -> Option<InvocationId> {
        self.parent_invocation_id
    }
    pub fn binding_id(&self) -> &BindingId {
        &self.binding_id
    }
    pub fn interface_id(&self) -> &InterfaceId {
        &self.interface_id
    }
    pub fn plan_fingerprint(&self) -> &PlanFingerprint {
        &self.plan_fingerprint
    }
    pub fn activation(&self) -> &ActivatedAuthenticationAdapter {
        &self.activation
    }
    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }
    pub fn registry_fingerprint(&self) -> &RegistryFingerprint {
        &self.registry_fingerprint
    }
    pub fn protocol(&self) -> InterfaceProtocol {
        self.protocol
    }
    pub fn received_at(&self) -> SystemTime {
        self.received_at
    }
    pub fn rejected_at(&self) -> SystemTime {
        self.rejected_at
    }
    pub fn principal(&self) -> Option<&PrincipalSummary> {
        None
    }
    pub fn stage(&self) -> &'static str {
        "authentication"
    }
    pub fn terminal(&self) -> InterfaceInvocationTerminal {
        InterfaceInvocationTerminal::Rejected
    }
    pub fn classification(&self) -> InterfaceAuthenticationRejectionClass {
        self.classification
    }
    pub fn observer_records(&self) -> &[InterfaceObserverRecord] {
        &self.observers
    }
}
