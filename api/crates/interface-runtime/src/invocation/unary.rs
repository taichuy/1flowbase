//! Unary lifecycle polling is owned here, independently of the typed leaves.
use super::*;
use crate::decision::{AdmissionContributions, AuthorizationContributions};
use crate::hook::FailureObservers;
use std::any::Any;

mod bridge;
use bridge::TypedUnaryBridge;

type UnaryValue = Box<dyn Any + Send + Sync>;
type UnaryTargetResult = Result<UnaryValue, UnaryValue>;
type UnaryTargetFuture = Pin<Box<dyn Future<Output = UnaryTargetResult> + Send>>;
type UnaryStageFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
type UnaryFuture<'a> =
    Pin<Box<dyn Future<Output = Result<UnaryOutcome, InterfaceInvocationFailure>> + Send + 'a>>;
type UnaryPreparation<'a> = Result<Box<PreparedUnary<'a>>, InterfaceInvocationFailure>;

// Only this module constructs erased values, from the matching typed handler.
// The downcasts in the bridge and O-only mapper are private type invariants.
trait UnaryBridge: Send + Sync {
    fn summary(&self) -> PrincipalSummary;
    fn event_principal(&self) -> Option<crate::InterfaceEventPrincipal>;
    fn authorization_reference(&self) -> AuthorizationAdapterReference;
    fn principal_profile(&self) -> crate::PrincipalProfile;
    fn contracts_match(&self, definition: &InterfaceDefinition) -> bool;
    fn has_hooks(&self, plan: &crate::CompiledInvocationPlan) -> bool;
    fn authorization<'p>(
        &self,
        plan: &'p crate::CompiledInvocationPlan,
    ) -> Option<AuthorizationContributions<'p>>;
    fn admission<'p>(
        &self,
        plan: &'p crate::CompiledInvocationPlan,
    ) -> Option<AdmissionContributions<'p>>;
    fn failure<'p>(&self, plan: &'p crate::CompiledInvocationPlan) -> Option<FailureObservers<'p>>;
    fn project_input(&self) -> Option<crate::ManagedInterfaceProjection>;
    fn authorize(
        &self,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
    ) -> InterfaceAuthorizationFuture<'_>;
    fn before<'a>(
        &'a mut self,
        plan: &'a crate::CompiledInvocationPlan,
        context: &'a InterfaceHookContext,
    ) -> crate::InterfaceBeforeHookFuture<'a>;
    fn observe_before<'a>(&'a self, receipt: &'a mut ReceiptBuilder) -> UnaryStageFuture<'a>;
    fn bind_handler(&mut self, snapshot: &CompiledInterfaceRegistry, id: &InterfaceId) -> bool;
    fn prepare_target(
        &mut self,
        attempt: ExecutionAttempt,
        invocation_id: InvocationId,
        graph: GraphFingerprint,
        registry: RegistryFingerprint,
        document: Option<crate::DocumentOperationSession>,
    );
    fn begin_target(&mut self) -> UnaryTargetFuture;
    fn finish_target(&mut self, result: UnaryTargetResult) -> bool;
    fn observe_success<'a>(
        &'a self,
        plan: &'a crate::CompiledInvocationPlan,
        receipt: &'a mut ReceiptBuilder,
        context: &'a InterfaceHookContext,
    ) -> UnaryStageFuture<'a>;
    fn error_classification(&self) -> &str;
    fn take_error(&mut self) -> InterfaceInvocationError;
    fn take_output(&mut self) -> UnaryValue;
}

struct UnaryRequest<'a> {
    snapshot: Arc<CompiledInterfaceRegistry>,
    lineage: InvocationLineage,
    binding_id: BindingId,
    protocol: InterfaceProtocol,
    authentication_adapter: AuthenticationAdapterReference,
    authentication_activation: AuthenticationActivationIdentity,
    controls: InvocationControls,
    extension_context: Option<crate::hook::InvocationExtensionContext>,
    dispatch_target: Option<ExecutionTargetPin>,
    target_admission: Option<&'a dyn InterfaceTargetAdmissionPort>,
}

struct UnaryOutcome {
    value: UnaryValue,
    receipt: InterfaceInvocationReceipt,
}

// Synchronous typed construction only: no projection, port callback or polling.
pub(super) fn invoke_typed<I, O, E, P>(
    kernel: &InterfaceInvocationKernel<P>,
    snapshot: Arc<CompiledInterfaceRegistry>,
    envelope: InvocationEnvelope<I, P>,
    dispatch_target: Option<ExecutionTargetPin>,
) -> Pin<Box<dyn Future<Output = InterfaceInvocationResult<O>> + Send + '_>>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    let InvocationEnvelope {
        lineage,
        binding_id,
        protocol,
        authentication_adapter,
        authentication_activation,
        principal,
        controls,
        extension_context,
        input,
    } = envelope;
    let request = UnaryRequest {
        snapshot,
        lineage,
        binding_id,
        protocol,
        authentication_adapter,
        authentication_activation,
        controls,
        extension_context,
        dispatch_target,
        target_admission: kernel.target_admission.as_deref(),
    };
    let typed = Box::new(TypedUnaryBridge::<I, O, E, P>::new(
        kernel.authorization.as_ref(),
        principal,
        input,
    ));
    map_output::<O>(start_unary(request, typed))
}

fn map_output<O: InterfaceContract>(
    driver: UnaryFuture<'_>,
) -> Pin<Box<dyn Future<Output = InterfaceInvocationResult<O>> + Send + '_>> {
    Box::pin(async move {
        let UnaryOutcome { value, receipt } = driver.await?;
        let value = *value
            .downcast::<O>()
            .expect("private unary bridge returns its declared output type");
        Ok(InterfaceInvocationOutcome { value, receipt })
    })
}

// The shared poll/drop types are created inside these non-generic factories.
fn start_unary<'a>(request: UnaryRequest<'a>, typed: Box<dyn UnaryBridge + 'a>) -> UnaryFuture<'a> {
    Box::pin(async move {
        // This awaited future must exit and drop before execute constructs a
        // handler future. Nested same-task calls depend on these poll frames.
        let prepared = prepare(request, typed).await?;
        execute(prepared).await
    })
}

fn prepare<'a>(
    request: UnaryRequest<'a>,
    mut typed: Box<dyn UnaryBridge + 'a>,
) -> Pin<Box<dyn Future<Output = UnaryPreparation<'a>> + Send + 'a>> {
    Box::pin(async move {
        let UnaryRequest {
            snapshot,
            lineage,
            binding_id,
            protocol,
            authentication_adapter,
            authentication_activation,
            controls,
            extension_context,
            dispatch_target,
            target_admission,
        } = request;
        let mut receipt = ReceiptBuilder::new(
            &snapshot,
            lineage.invocation_id(),
            lineage.parent_invocation_id(),
            protocol,
            typed.summary(),
            &controls,
        );
        receipt.extension_context = extension_context.clone();
        let mut hook_context = InterfaceHookContext::new(
            typed.summary(),
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        )
        .with_extension_context(extension_context);
        if let Some(plan) = snapshot.plan(&binding_id) {
            if plan.event_observer.is_some()
                || (plan.document_observer.is_some() && plan.document_operation.is_some())
            {
                receipt.event_principal = typed.event_principal();
            }
            receipt.resolve(plan);
        }
        let initial_interruption = if controls.cancellation.is_cancelled() {
            Some(InvocationInterruption::Cancelled)
        } else if controls
            .deadline
            .is_some_and(|deadline| deadline <= SystemTime::now())
        {
            Some(InvocationInterruption::DeadlineElapsed)
        } else {
            None
        };
        if let Some(interruption) = initial_interruption {
            return Err(receipt
                .fail(
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                )
                .await);
        }
        let Some(plan) = snapshot.plan(&binding_id).cloned() else {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::UnknownBinding,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        };
        let definition = plan.definition().clone();
        let binding = plan.binding().clone();
        let has_hooks = typed.has_hooks(&plan);
        if plan.has_executable_extensions() && !has_hooks {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        receipt.resolve(&plan);
        receipt.stage(InterfaceInvocationStage::Resolved);
        if binding.projection().protocol() != protocol {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::ProtocolBindingMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if plan.adapter_plan().authentication() != &authentication_adapter {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AuthenticationAdapterMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if plan.authentication().activation() != &authentication_activation {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AuthenticationActivationMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if plan.adapter_plan().authorization() != &typed.authorization_reference() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AuthorizationAdapterMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        let admission_reference = target_admission.map(|admission| admission.adapter_reference());
        if plan.adapter_plan().admission() != admission_reference.as_ref() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AdmissionAdapterMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if definition.principal_profile() != typed.principal_profile() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::PrincipalProfileMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        receipt.stage(InterfaceInvocationStage::PrincipalEstablished);
        if !typed.contracts_match(&definition) {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if let Some(factory) = &plan.managed_factory {
            let frozen = await_in_flight(
                &controls,
                factory.freeze(crate::ManagedInterfaceFreezeRequest {
                    definition: definition.clone(),
                    context: hook_context.clone(),
                    input: typed.project_input(),
                    deadline: controls.deadline(),
                }),
            )
            .await;
            match frozen {
                Ok(Ok(invocation)) => {
                    let context = crate::hook::ManagedInvocationContext(invocation);
                    hook_context.managed_invocation = Some(context.clone());
                    receipt.managed_invocation = Some(context);
                }
                Ok(Err(classification)) => {
                    return Err(receipt
                        .fail(
                            InterfaceInvocationError::BeforeHookRejected(
                                crate::InterfaceBeforeHookError::classified(classification),
                            ),
                            InterfaceInvocationTerminal::Rejected,
                        )
                        .await);
                }
                Err(interruption) => {
                    return Err(receipt
                        .fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await);
                }
            }
        }

        // Construct with full sealed P, after freeze but before control polling.
        let authorization = typed.authorize(definition.clone(), binding.clone(), protocol);
        let principal = typed.summary();
        let authorization_extensions = typed.authorization(&plan);
        let admission_extensions = typed.admission(&plan);
        let failure = typed.failure(&plan);
        match await_in_flight(&controls, authorization).await {
            Err(interruption) => {
                return Err(receipt
                    .fail(
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    )
                    .await);
            }
            Ok(Err(error)) => {
                return Err(reject(
                    receipt,
                    failure,
                    &hook_context,
                    InterfaceInvocationError::AuthorizationRejected(error),
                )
                .await);
            }
            Ok(Ok(())) => {}
        }
        if let Some(extensions) = authorization_extensions {
            let authorization = await_in_flight(
                &controls,
                extensions.run(InterfaceAuthorizationContributionRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                    hook_context.clone(),
                )),
            )
            .await;
            match authorization {
                Err(interruption) => {
                    return Err(receipt
                        .fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await);
                }
                Ok(Err(error)) => {
                    return Err(reject(
                        receipt,
                        failure,
                        &hook_context,
                        InterfaceInvocationError::AuthorizationContributionRejected(error),
                    )
                    .await);
                }
                Ok(Ok(())) => {}
            }
        }
        let authorization_decision = authorization_decision_fingerprint(&plan);
        receipt.authorized(authorization_decision.clone());
        if let Some(admission) = target_admission {
            let admission = await_in_flight(
                &controls,
                admission.admit(InterfaceTargetAdmissionRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
            )
            .await;
            match admission {
                Err(interruption) => {
                    return Err(receipt
                        .fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await);
                }
                Ok(Err(error)) => {
                    return Err(reject(
                        receipt,
                        failure,
                        &hook_context,
                        InterfaceInvocationError::AdmissionRejected(error),
                    )
                    .await);
                }
                Ok(Ok(())) => {}
            }
        }
        if let Some(extensions) = admission_extensions {
            let admission = await_in_flight(
                &controls,
                extensions.run(InterfaceAdmissionContributionRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                    authorization_decision,
                    hook_context.clone(),
                )),
            )
            .await;
            match admission {
                Err(interruption) => {
                    return Err(receipt
                        .fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await);
                }
                Ok(Err(error)) => {
                    return Err(reject(
                        receipt,
                        failure,
                        &hook_context,
                        InterfaceInvocationError::AdmissionContributionRejected(error),
                    )
                    .await);
                }
                Ok(Ok(())) => {}
            }
        }
        receipt.stage(InterfaceInvocationStage::Admitted);
        if has_hooks {
            let before = await_in_flight(&controls, typed.before(&plan, &hook_context)).await;
            match before {
                Err(interruption) => {
                    return Err(receipt
                        .fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await);
                }
                Ok(Err(error)) => {
                    return Err(reject(
                        receipt,
                        failure,
                        &hook_context,
                        InterfaceInvocationError::BeforeHookRejected(error),
                    )
                    .await);
                }
                Ok(Ok(())) => {}
            }
        }
        if plan.event_observer.is_some()
            || receipt
                .document_operation
                .as_ref()
                .is_some_and(|session| session.is_static())
        {
            typed.observe_before(&mut receipt).await;
        }
        receipt.stage(InterfaceInvocationStage::Prepared);
        if !typed.bind_handler(&snapshot, definition.interface_id()) {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        let attempt = ExecutionAttempt::first(dispatch_target.unwrap_or_else(|| {
            if plan.effective_handler().plugin().as_str() == "builtin.interface-handler" {
                ExecutionTargetPin::BuiltIn {
                    handler: plan.effective_handler().handler().clone(),
                    target: plan.effective_handler().target().clone(),
                }
            } else {
                ExecutionTargetPin::HostExtension {
                    handler: plan.effective_handler().handler().clone(),
                    target: plan.effective_handler().target().clone(),
                    plugin: plan.effective_handler().plugin().clone(),
                }
            }
        }));
        receipt.dispatch(attempt.clone());
        receipt.stage(InterfaceInvocationStage::Executing);
        typed.prepare_target(
            attempt,
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
            receipt.document_operation.clone(),
        );
        Ok(Box::new(PreparedUnary {
            snapshot,
            plan,
            receipt,
            hook_context,
            controls,
            typed,
        }))
    })
}

async fn reject(
    mut receipt: ReceiptBuilder,
    failure: Option<FailureObservers<'_>>,
    context: &InterfaceHookContext,
    error: InterfaceInvocationError,
) -> InterfaceInvocationFailure {
    if let Some(failure) = failure {
        failure
            .run(&mut receipt.finalization, context, error.classification())
            .await;
    }
    receipt
        .fail(error, InterfaceInvocationTerminal::Rejected)
        .await
}

// No target future is present in the prepare/execute handoff.
struct PreparedUnary<'a> {
    snapshot: Arc<CompiledInterfaceRegistry>,
    plan: crate::CompiledInvocationPlan,
    receipt: ReceiptBuilder,
    hook_context: InterfaceHookContext,
    controls: InvocationControls,
    typed: Box<dyn UnaryBridge + 'a>,
}

#[expect(
    clippy::boxed_local,
    reason = "Keep the prepare/execute handoff pointer-sized in the driver poll frame."
)]
fn execute(prepared: Box<PreparedUnary<'_>>) -> UnaryFuture<'_> {
    let PreparedUnary {
        snapshot,
        plan,
        mut receipt,
        hook_context,
        controls,
        mut typed,
    } = *prepared;
    // Synchronous construction, after the prepare future has left the stack.
    let target = typed.begin_target();
    Box::pin(async move {
        let _snapshot = snapshot;
        let target = match await_in_flight(&controls, target).await {
            Ok(target) => target,
            Err(interruption) => {
                return Err(receipt
                    .fail(
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    )
                    .await);
            }
        };
        if typed.finish_target(target) {
            if plan.event_observer.is_some()
                || typed.has_hooks(&plan)
                || receipt.document_operation.is_some()
            {
                typed
                    .observe_success(&plan, &mut receipt, &hook_context)
                    .await;
            }
            receipt.stage(InterfaceInvocationStage::PostProcessed);
            let receipt = receipt.complete().await;
            Ok(UnaryOutcome {
                value: typed.take_output(),
                receipt,
            })
        } else {
            if let Some(failure) = typed.failure(&plan) {
                failure
                    .run(
                        &mut receipt.finalization,
                        &hook_context,
                        typed.error_classification(),
                    )
                    .await;
            }
            Err(receipt
                .fail(typed.take_error(), InterfaceInvocationTerminal::Failed)
                .await)
        }
    })
}
