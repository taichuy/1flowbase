use super::*;
use crate::{
    decision::{run_admission, run_authorization},
    CompiledInvocationPlan, InterfaceAdmissionContribution, InterfaceAuthorizationContribution,
};

type AuthorizationBindings = Vec<(PluginIdentity, Arc<dyn InterfaceAuthorizationContribution>)>;
type AdmissionBindings = Vec<(PluginIdentity, Arc<dyn InterfaceAdmissionContribution>)>;

// Borrow the principal and obtain each summary at its original stage.
pub(super) trait PrincipalSummarySource: Send + Sync {
    fn principal_summary(&self) -> PrincipalSummary;
}

impl<P: InvocationPrincipal> PrincipalSummarySource for P {
    fn principal_summary(&self) -> PrincipalSummary {
        self.summary()
    }
}

pub(super) fn authorization_bindings<I: InterfaceContract, O: InterfaceContract>(
    plan: &CompiledInvocationPlan,
) -> Option<&AuthorizationBindings> {
    plan.authorization_plan::<I, O>()
        .map(|plan| plan.contributions())
}

pub(super) fn admission_bindings<I: InterfaceContract, O: InterfaceContract>(
    plan: &CompiledInvocationPlan,
) -> Option<&AdmissionBindings> {
    plan.admission_plan::<I, O>()
        .map(|plan| plan.contributions())
}

pub(super) enum DecisionFailure {
    Interrupted(InvocationInterruption),
    Rejected(InterfaceInvocationError),
}

// Only the two small lookup functions depend on the input/output contracts.
// This phase borrows all owners retained by the typed caller during finalization.
pub(super) struct InvocationDecisions<'a> {
    pub principal: &'a dyn PrincipalSummarySource,
    pub plan: &'a CompiledInvocationPlan,
    pub definition: &'a InterfaceDefinition,
    pub binding: &'a ProtocolBinding,
    pub protocol: InterfaceProtocol,
    pub target_admission: Option<&'a dyn InterfaceTargetAdmissionPort>,
    pub authorization_bindings: fn(&CompiledInvocationPlan) -> Option<&AuthorizationBindings>,
    pub admission_bindings: fn(&CompiledInvocationPlan) -> Option<&AdmissionBindings>,
    pub controls: &'a InvocationControls,
    pub context: &'a InterfaceHookContext,
    pub receipt: &'a mut ReceiptBuilder,
    pub authorization_decision: &'a mut Option<AuthorizationDecisionFingerprint>,
}

impl InvocationDecisions<'_> {
    pub async fn run(
        mut self,
        core_authorization: InterfaceAuthorizationFuture<'_>,
    ) -> Result<(), DecisionFailure> {
        await_in_flight(self.controls, core_authorization)
            .await
            .map_err(DecisionFailure::Interrupted)?
            .map_err(|error| {
                DecisionFailure::Rejected(InterfaceInvocationError::AuthorizationRejected(error))
            })?;
        if let Some(bindings) = (self.authorization_bindings)(self.plan) {
            self.authorization_future(bindings)
                .await
                .map_err(DecisionFailure::Interrupted)?
                .map_err(|error| {
                    DecisionFailure::Rejected(
                        InterfaceInvocationError::AuthorizationContributionRejected(error),
                    )
                })?;
        }
        let decision = self
            .authorization_decision
            .insert(authorization_decision_fingerprint(self.plan));
        self.receipt.authorized(decision.clone());
        if let Some(target_admission) = self.target_admission {
            await_in_flight(
                self.controls,
                target_admission.admit(InterfaceTargetAdmissionRequest::new(
                    self.principal.principal_summary(),
                    self.definition.clone(),
                    self.binding.clone(),
                    self.protocol,
                )),
            )
            .await
            .map_err(DecisionFailure::Interrupted)?
            .map_err(|error| {
                DecisionFailure::Rejected(InterfaceInvocationError::AdmissionRejected(error))
            })?;
        }
        if let Some(bindings) = (self.admission_bindings)(self.plan) {
            self.admission_future(bindings)
                .await
                .map_err(DecisionFailure::Interrupted)?
                .map_err(|error| {
                    DecisionFailure::Rejected(
                        InterfaceInvocationError::AdmissionContributionRejected(error),
                    )
                })?;
        }
        self.receipt.stage(InterfaceInvocationStage::Admitted);
        Ok(())
    }

    // Construct the existing concrete futures outside the phase's poll frame.
    // These ordinary functions add neither an async state nor an allocation.
    fn authorization_future<'a>(
        &'a self,
        bindings: &'a AuthorizationBindings,
    ) -> impl Future<
        Output = Result<
            Result<(), InterfaceAuthorizationContributionError>,
            InvocationInterruption,
        >,
    > + 'a {
        await_in_flight(
            self.controls,
            run_authorization(
                bindings,
                InterfaceAuthorizationContributionRequest::new(
                    self.principal.principal_summary(),
                    self.definition.clone(),
                    self.binding.clone(),
                    self.protocol,
                    self.context.clone(),
                ),
            ),
        )
    }

    fn admission_future<'a>(
        &'a mut self,
        bindings: &'a AdmissionBindings,
    ) -> impl Future<
        Output = Result<Result<(), InterfaceAdmissionContributionError>, InvocationInterruption>,
    > + 'a {
        await_in_flight(
            self.controls,
            run_admission(
                bindings,
                InterfaceAdmissionContributionRequest::new(
                    self.principal.principal_summary(),
                    self.definition.clone(),
                    self.binding.clone(),
                    self.protocol,
                    self.authorization_decision
                        .take()
                        .expect("authorization precedes admission"),
                    self.context.clone(),
                ),
            ),
        )
    }
}
