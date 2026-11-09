use super::*;
use crate::decision::{AdmissionContributions, AuthorizationContributions};
use crate::hook::FailureObservers;

pub(super) struct InvocationPolicy<'a> {
    pub controls: &'a InvocationControls,
    pub plan: &'a crate::CompiledInvocationPlan,
    pub principal: PrincipalSummary,
    pub context: &'a InterfaceHookContext,
    pub target_admission: Option<&'a dyn InterfaceTargetAdmissionPort>,
    pub authorization: Option<AuthorizationContributions<'a>>,
    pub admission: Option<AdmissionContributions<'a>>,
    pub failure: Option<FailureObservers<'a>>,
}

impl InvocationPolicy<'_> {
    pub(super) async fn run(
        self,
        authorization: InterfaceAuthorizationFuture<'_>,
        receipt: &mut ReceiptBuilder,
    ) -> Result<(), (InterfaceInvocationError, InterfaceInvocationTerminal)> {
        let authorization = await_in_flight(self.controls, authorization)
            .await
            .map_err(|interruption| {
                (
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                )
            })?;
        if let Err(error) = authorization {
            return Err(self
                .reject(
                    receipt,
                    InterfaceInvocationError::AuthorizationRejected(error),
                )
                .await);
        }
        let definition = self.plan.definition();
        let binding = self.plan.binding();
        let protocol = binding.projection().protocol();
        if let Some(extensions) = self.authorization {
            let authorization = await_in_flight(
                self.controls,
                extensions.run(InterfaceAuthorizationContributionRequest::new(
                    self.principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                    self.context.clone(),
                )),
            )
            .await
            .map_err(|interruption| {
                (
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                )
            })?;
            if let Err(error) = authorization {
                return Err(self
                    .reject(
                        receipt,
                        InterfaceInvocationError::AuthorizationContributionRejected(error),
                    )
                    .await);
            }
        }
        let authorization_decision = authorization_decision_fingerprint(self.plan);
        receipt.authorized(authorization_decision.clone());
        if let Some(target_admission) = self.target_admission {
            let admission = await_in_flight(
                self.controls,
                target_admission.admit(InterfaceTargetAdmissionRequest::new(
                    self.principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
            )
            .await
            .map_err(|interruption| {
                (
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                )
            })?;
            if let Err(error) = admission {
                return Err(self
                    .reject(receipt, InterfaceInvocationError::AdmissionRejected(error))
                    .await);
            }
        }
        if let Some(extensions) = self.admission {
            let admission = await_in_flight(
                self.controls,
                extensions.run(InterfaceAdmissionContributionRequest::new(
                    self.principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                    authorization_decision,
                    self.context.clone(),
                )),
            )
            .await
            .map_err(|interruption| {
                (
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                )
            })?;
            if let Err(error) = admission {
                return Err(self
                    .reject(
                        receipt,
                        InterfaceInvocationError::AdmissionContributionRejected(error),
                    )
                    .await);
            }
        }
        receipt.stage(InterfaceInvocationStage::Admitted);
        Ok(())
    }

    async fn reject(
        &self,
        receipt: &mut ReceiptBuilder,
        error: InterfaceInvocationError,
    ) -> (InterfaceInvocationError, InterfaceInvocationTerminal) {
        if let Some(failure) = self.failure {
            failure
                .run(
                    &mut receipt.finalization,
                    self.context,
                    error.classification(),
                )
                .await;
        }
        (error, InterfaceInvocationTerminal::Rejected)
    }
}
