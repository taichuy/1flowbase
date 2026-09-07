//! Host owner for authentication attempts. Credentials and original errors stay here;
//! only frozen metadata and a finite rejection classification reach lifecycle observers.
use super::AuthenticationAdapterFactoryRegistry;
use interface_runtime::{
    BindingId, CompiledInterfaceRegistry, InterfaceAuthenticationAttempt,
    InterfaceAuthenticationRejectionClass, InterfaceContract, InterfaceProtocol,
    InvocationEnvelope, InvocationId, InvocationLineage, InvocationPrincipal,
};
use std::{any::Any, sync::Arc};

pub(crate) struct AuthenticatedInvocation<P: InvocationPrincipal> {
    attempt: InterfaceAuthenticationAttempt,
    principal: P,
}
impl<P: InvocationPrincipal> AuthenticatedInvocation<P> {
    pub(crate) fn principal(&self) -> &P {
        &self.principal
    }
    #[cfg(test)]
    pub(crate) fn lineage(&self) -> &InvocationLineage {
        self.attempt.lineage()
    }
    pub(crate) fn into_envelope<I: InterfaceContract>(self, input: I) -> InvocationEnvelope<I, P> {
        self.attempt.into_envelope(self.principal, input)
    }

    pub(crate) fn into_same_http_carrier_envelope<I: InterfaceContract>(
        self,
        binding: &BindingId,
        input: I,
    ) -> Result<InvocationEnvelope<I, P>, interface_runtime::HttpSiblingAuthenticationError> {
        self.attempt
            .into_same_http_carrier_envelope(binding, self.principal, input)
    }
}

impl AuthenticationAdapterFactoryRegistry {
    pub(crate) async fn authenticate_invocation<C, P>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        binding: &BindingId,
        protocol: InterfaceProtocol,
        credential: C,
    ) -> anyhow::Result<AuthenticatedInvocation<P>>
    where
        C: Any + Send + 'static,
        P: InvocationPrincipal,
    {
        let lineage = InvocationLineage::root(InvocationId::now_v7());
        let Some(attempt) =
            InterfaceAuthenticationAttempt::resolve(snapshot, binding, protocol, lineage.clone())
        else {
            tracing::warn!(target: "interface_lifecycle", invocation_id = %lineage.invocation_id().value(),
                classification = "unresolved-authentication-binding", stage = "ingress",
                "interface authentication ingress rejected");
            return Err(control_plane::errors::ControlPlaneError::NotFound(
                "authentication_activation",
            )
            .into());
        };
        // A missing/mismatched factory is an ingress assembly error, not a credential rejection.
        if let Err(error) = self.validate_activation(attempt.activation()) {
            tracing::warn!(target: "interface_lifecycle", invocation_id = %attempt.lineage().invocation_id().value(),
                classification = "unresolved-authentication-activation", stage = "ingress",
                "interface authentication ingress rejected");
            return Err(error);
        }
        match self.authenticate(attempt.activation(), credential).await {
            Ok(principal) => Ok(AuthenticatedInvocation { attempt, principal }),
            Err(error) => {
                let classification =
                    match error.downcast_ref::<control_plane::errors::ControlPlaneError>() {
                        Some(
                            control_plane::errors::ControlPlaneError::NotAuthenticated
                            | control_plane::errors::ControlPlaneError::PermissionDenied(_),
                        ) => InterfaceAuthenticationRejectionClass::CredentialRejected,
                        _ => InterfaceAuthenticationRejectionClass::AuthenticationFailed,
                    };
                let receipt = attempt.reject(classification).await;
                tracing::warn!(target: "interface_lifecycle",
                    invocation_id = %receipt.invocation_id().value(),
                    parent_invocation_id = ?receipt.parent_invocation_id(),
                    binding_id = receipt.binding_id().as_str(), interface_id = receipt.interface_id().as_str(),
                    plan_fingerprint = receipt.plan_fingerprint().as_str(),
                    graph_fingerprint = receipt.graph_fingerprint().as_str(), registry_fingerprint = receipt.registry_fingerprint().as_str(),
                    authentication_adapter = receipt.activation().adapter().as_str(), authentication_activation = receipt.activation().activation().as_str(),
                    authentication_plugin = receipt.activation().plugin().as_str(),
                    principal = "unestablished", stage = receipt.stage(), terminal = ?receipt.terminal(),
                    protocol = ?receipt.protocol(), received_at = ?receipt.received_at(), rejected_at = ?receipt.rejected_at(),
                    classification = receipt.classification().as_str(), observers = ?receipt.observer_records(),
                    "interface authentication rejected");
                Err(error)
            }
        }
    }
}
