use super::*;
use crate::{CompiledInvocationPlan, PrincipalProfile};

// These checks do not depend on the concrete input, output, error or principal.
// Callbacks keep adapter and contract identities lazy, in their original order.
pub(super) struct InvocationMetadata<'a> {
    pub plan: &'a CompiledInvocationPlan,
    pub definition: &'a InterfaceDefinition,
    pub binding: &'a ProtocolBinding,
    pub protocol: InterfaceProtocol,
    pub authentication_adapter: &'a AuthenticationAdapterReference,
    pub authentication_activation: &'a AuthenticationActivationIdentity,
    pub authorization_reference: &'a dyn Fn() -> AuthorizationAdapterReference,
    pub target_admission: Option<&'a dyn InterfaceTargetAdmissionPort>,
    pub principal_profile: PrincipalProfile,
    pub contracts: [fn() -> ContractIdentity; 3],
    pub stream_event: Option<fn() -> ContractIdentity>,
}

impl InvocationMetadata<'_> {
    pub fn validate(
        self,
        receipt: &mut ReceiptBuilder,
        admission_reference: &mut Option<AdmissionAdapterReference>,
    ) -> Result<(), InterfaceInvocationError> {
        receipt.stage(InterfaceInvocationStage::Resolved);
        if self.binding.projection().protocol() != self.protocol {
            return Err(InterfaceInvocationError::ProtocolBindingMismatch);
        }
        if self.plan.adapter_plan().authentication() != self.authentication_adapter {
            return Err(InterfaceInvocationError::AuthenticationAdapterMismatch);
        }
        if self.plan.authentication().activation() != self.authentication_activation {
            return Err(InterfaceInvocationError::AuthenticationActivationMismatch);
        }
        if self.plan.adapter_plan().authorization() != &(self.authorization_reference)() {
            return Err(InterfaceInvocationError::AuthorizationAdapterMismatch);
        }
        *admission_reference = self
            .target_admission
            .map(|admission| admission.adapter_reference());
        if self.plan.adapter_plan().admission() != admission_reference.as_ref() {
            return Err(InterfaceInvocationError::AdmissionAdapterMismatch);
        }
        if self.definition.principal_profile() != self.principal_profile {
            return Err(InterfaceInvocationError::PrincipalProfileMismatch);
        }
        receipt.stage(InterfaceInvocationStage::PrincipalEstablished);
        if self.definition.input_contract() != &(self.contracts[0])()
            || self.stream_event.is_some_and(|identity| {
                self.definition.stream_event_contract() != Some(&identity())
            })
            || self.definition.output_contract() != &(self.contracts[1])()
            || self.definition.target_error_contract() != &(self.contracts[2])()
        {
            return Err(InterfaceInvocationError::ContractMismatch);
        }
        Ok(())
    }
}
