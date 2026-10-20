use super::*;

impl<I, P> InvocationEnvelope<I, P>
where
    I: InterfaceContract,
    P: InvocationPrincipal,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "the stable envelope constructor mirrors the sealed invocation boundary"
    )]
    pub fn with_principal(
        lineage: InvocationLineage,
        binding_id: BindingId,
        protocol: InterfaceProtocol,
        authentication_adapter: AuthenticationAdapterReference,
        authentication_activation: AuthenticationActivationIdentity,
        principal: P,
        deadline: Option<SystemTime>,
        input: I,
    ) -> Self {
        Self::with_principal_and_controls(
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            authentication_activation,
            principal,
            InvocationControls::new(deadline, InvocationCancellation::new(), None),
            input,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the stable envelope constructor mirrors the sealed invocation boundary"
    )]
    pub fn with_principal_and_controls(
        lineage: InvocationLineage,
        binding_id: BindingId,
        protocol: InterfaceProtocol,
        authentication_adapter: AuthenticationAdapterReference,
        authentication_activation: AuthenticationActivationIdentity,
        principal: P,
        controls: InvocationControls,
        input: I,
    ) -> Self {
        Self {
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            authentication_activation,
            principal,
            controls,
            extension_context: None,
            input,
        }
    }

    pub fn input(&self) -> &I {
        &self.input
    }

    /// The trusted adapter freezes one typed context before Kernel resolve. A second freeze fails.
    pub fn freeze_extension_context<T: std::any::Any + Send + Sync>(
        mut self,
        context: Arc<T>,
    ) -> Result<Self, InterfaceInvocationError> {
        if self.extension_context.is_some() {
            return Err(InterfaceInvocationError::ContractMismatch);
        }
        self.extension_context = Some(crate::hook::InvocationExtensionContext(context));
        Ok(self)
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

    pub fn authentication_activation(&self) -> &AuthenticationActivationIdentity {
        &self.authentication_activation
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
    #[expect(
        clippy::too_many_arguments,
        reason = "the stable user envelope constructor mirrors the sealed invocation boundary"
    )]
    pub fn new(
        lineage: InvocationLineage,
        binding_id: BindingId,
        protocol: InterfaceProtocol,
        authentication_adapter: AuthenticationAdapterReference,
        authentication_activation: AuthenticationActivationIdentity,
        actor: domain::ActorContext,
        deadline: Option<SystemTime>,
        input: I,
    ) -> Self {
        Self::with_principal(
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            authentication_activation,
            UserPrincipal::server_delegation(actor),
            deadline,
            input,
        )
    }
}
