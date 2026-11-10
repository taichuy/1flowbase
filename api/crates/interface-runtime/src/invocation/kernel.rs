use super::decisions::{
    admission_bindings, authorization_bindings, DecisionFailure, InvocationDecisions,
};
use super::metadata::InvocationMetadata;
use super::*;

impl<P> InterfaceInvocationKernel<P>
where
    P: InvocationPrincipal,
{
    pub fn new(authorization: Arc<dyn InterfaceAuthorizationPort<P>>) -> Self {
        Self {
            authorization,
            target_admission: None,
        }
    }

    pub fn with_target_admission(
        authorization: Arc<dyn InterfaceAuthorizationPort<P>>,
        target_admission: Arc<dyn InterfaceTargetAdmissionPort>,
    ) -> Self {
        Self {
            authorization,
            target_admission: Some(target_admission),
        }
    }

    pub fn invoke<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
    ) -> impl Future<Output = InterfaceInvocationResult<O>> + '_
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        // Finish constructing the boxed state before its first poll.
        Box::pin(self.invoke_internal::<I, O, E>(snapshot, envelope, None))
    }

    pub fn invoke_with_dispatch_target<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        target: ExecutionTargetPin,
    ) -> impl Future<Output = InterfaceInvocationResult<O>> + '_
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        Box::pin(self.invoke_internal::<I, O, E>(snapshot, envelope, Some(target)))
    }

    pub async fn invoke_server_stream_with_dispatch_target<I, S, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        target: ExecutionTargetPin,
    ) -> Result<InterfaceStreamInvocation<S, O, E>, InterfaceInvocationFailure>
    where
        I: InterfaceContract,
        S: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
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
            mut input,
        } = envelope;
        let mut receipt = ReceiptBuilder::new(
            &snapshot,
            lineage.invocation_id(),
            lineage.parent_invocation_id(),
            protocol,
            principal.summary(),
            &controls,
        );
        receipt.extension_context = extension_context.clone();
        if let Some(plan) = snapshot.plan(&binding_id) {
            if plan.event_observer.is_some()
                || (plan.document_observer.is_some() && plan.document_operation.is_some())
            {
                receipt.event_principal =
                    crate::InterfaceEventPrincipal::from_principal(&principal);
            }
            receipt.resolve(plan);
        }
        let interruption = if controls.cancellation.is_cancelled() {
            Some(InvocationInterruption::Cancelled)
        } else if controls
            .deadline
            .is_some_and(|deadline| deadline <= SystemTime::now())
        {
            Some(InvocationInterruption::DeadlineElapsed)
        } else {
            None
        };
        if let Some(interruption) = interruption {
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
        let hook_plan = plan.hook_plan::<I, O>().cloned();
        if plan.has_executable_extensions() && hook_plan.is_none() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        let mut hook_context = InterfaceHookContext::new(
            principal.summary(),
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        )
        .with_extension_context(extension_context);
        receipt.resolve(&plan);
        let mut admission_reference = None;
        let validation = InvocationMetadata {
            plan: &plan,
            definition: &definition,
            binding: &binding,
            protocol,
            authentication_adapter: &authentication_adapter,
            authentication_activation: &authentication_activation,
            authorization_reference: &|| self.authorization.adapter_reference(),
            target_admission: self.target_admission.as_deref(),
            principal_profile: P::PROFILE,
            contracts: [
                contract_identity::<I>,
                contract_identity::<O>,
                contract_identity::<E>,
            ],
            stream_event: Some(contract_identity::<S>),
        }
        .validate(&mut receipt, &mut admission_reference);
        if let Err(error) = validation {
            return Err(receipt
                .fail(error, InterfaceInvocationTerminal::Rejected)
                .await);
        }
        if let Some(factory) = &plan.managed_factory {
            let frozen = await_in_flight(
                &controls,
                factory.freeze(crate::ManagedInterfaceFreezeRequest {
                    definition: definition.clone(),
                    context: hook_context.clone(),
                    input: crate::ManagedInterfaceProjection::from_contract(&input),
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

        let mut authorization_decision = None;
        let authorization = self
            .authorization
            .authorize(InterfaceAuthorizationRequest::new(
                principal.clone(),
                definition.clone(),
                binding.clone(),
                protocol,
            ));
        let decisions = InvocationDecisions {
            principal: &principal,
            plan: &plan,
            definition: &definition,
            binding: &binding,
            protocol,
            target_admission: self.target_admission.as_deref(),
            authorization_bindings: authorization_bindings::<I, O>,
            admission_bindings: admission_bindings::<I, O>,
            controls: &controls,
            context: &hook_context,
            receipt: &mut receipt,
            authorization_decision: &mut authorization_decision,
        };
        if let Err(failure) = decisions.run(authorization).await {
            let (error, terminal) = match failure {
                DecisionFailure::Interrupted(interruption) => (
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                ),
                DecisionFailure::Rejected(error) => {
                    run_failure_hooks(
                        &mut receipt.finalization,
                        hook_plan.as_ref(),
                        &hook_context,
                        error.classification(),
                    )
                    .await;
                    (error, InterfaceInvocationTerminal::Rejected)
                }
            };
            return Err(receipt.fail(error, terminal).await);
        }
        if let Some(hooks) = hook_plan.as_ref() {
            match await_in_flight(&controls, hooks.run_before(&hook_context, &mut input)).await {
                Err(interruption) => {
                    return Err(receipt
                        .fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await);
                }
                Ok(Err(error)) => {
                    run_failure_hooks(
                        &mut receipt.finalization,
                        hook_plan.as_ref(),
                        &hook_context,
                        error.classification(),
                    )
                    .await;
                    return Err(receipt
                        .fail(
                            InterfaceInvocationError::BeforeHookRejected(error),
                            InterfaceInvocationTerminal::Rejected,
                        )
                        .await);
                }
                Ok(Ok(())) => {}
            }
        }
        receipt.observe_event_before(&input).await;
        if let Some(session) = &receipt.document_operation {
            if session.is_static() {
                let _ = session.observe_before(&input).await;
            }
        }
        receipt.stage(InterfaceInvocationStage::Prepared);
        let Some(handler) = snapshot.stream_handler::<I, S, O, E, P>(definition.interface_id())
        else {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        };
        let attempt = ExecutionAttempt::first(target);
        receipt.dispatch(attempt.clone());
        receipt.stage(InterfaceInvocationStage::Executing);
        let mut context = InterfaceHandlerContext::new(
            principal,
            attempt,
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
        context.document_operation = receipt.document_operation.clone();
        let stream = match await_in_flight(&controls, handler.invoke_stream(context, input)).await {
            Err(interruption) => {
                return Err(receipt
                    .fail(
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    )
                    .await);
            }
            Ok(Err(error)) => {
                run_failure_hooks(
                    &mut receipt.finalization,
                    hook_plan.as_ref(),
                    &hook_context,
                    error.classification(),
                )
                .await;
                return Err(receipt
                    .fail(target_error(error), InterfaceInvocationTerminal::Failed)
                    .await);
            }
            Ok(Ok(stream)) => stream,
        };
        let completion = InterfaceStreamCompletion {
            completion: Box::pin(async move {
                let terminal = match await_in_flight(&controls, stream.terminal).await {
                    Err(interruption) => {
                        return Err(receipt
                            .fail(
                                interruption_error(interruption),
                                InterfaceInvocationTerminal::Cancelled,
                            )
                            .await);
                    }
                    Ok(Err(_)) => {
                        run_failure_hooks(
                            &mut receipt.finalization,
                            hook_plan.as_ref(),
                            &hook_context,
                            "stream-terminal-missing",
                        )
                        .await;
                        return Err(receipt
                            .fail(
                                InterfaceInvocationError::TargetFailed(
                                    InterfaceTargetError::classified("stream-terminal-missing"),
                                ),
                                InterfaceInvocationTerminal::Failed,
                            )
                            .await);
                    }
                    Ok(Ok(terminal)) => terminal,
                };
                match terminal {
                    InterfaceStreamTerminal::Completed(ref output) => {
                        receipt.project_event_output(output).await;
                        if let Some(hooks) = hook_plan.as_ref() {
                            hooks
                                .run_after(&mut receipt.finalization, &hook_context, output)
                                .await;
                        }
                        if let Some(session) = &receipt.document_operation {
                            session
                                .project_output(output, &mut receipt.finalization)
                                .await;
                        }
                        receipt.stage(InterfaceInvocationStage::PostProcessed);
                        Ok(InterfaceStreamTerminalOutcome {
                            terminal,
                            receipt: receipt.complete().await,
                        })
                    }
                    InterfaceStreamTerminal::Failed(error) => {
                        run_failure_hooks(
                            &mut receipt.finalization,
                            hook_plan.as_ref(),
                            &hook_context,
                            error.classification(),
                        )
                        .await;
                        Err(receipt
                            .fail(target_error(error), InterfaceInvocationTerminal::Failed)
                            .await)
                    }
                    InterfaceStreamTerminal::Rejected { ref classification } => {
                        run_failure_hooks(
                            &mut receipt.finalization,
                            hook_plan.as_ref(),
                            &hook_context,
                            classification,
                        )
                        .await;
                        Err(receipt
                            .fail(
                                InterfaceInvocationError::AuthorizationRejected(
                                    InterfaceAuthorizationError::classified(classification),
                                ),
                                InterfaceInvocationTerminal::Rejected,
                            )
                            .await)
                    }
                    InterfaceStreamTerminal::Cancelled => Err(receipt
                        .fail(
                            InterfaceInvocationError::Cancelled,
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await),
                }
            }),
        };
        Ok(InterfaceStreamInvocation {
            events: stream.events,
            completion,
        })
    }

    async fn invoke_internal<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        dispatch_target: Option<ExecutionTargetPin>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        // Preparation owns the larger validation state machine. It must return
        // before a handler can poll a nested invocation on this same task.
        let prepared = self
            .prepare_unary::<I, O, E>(snapshot, envelope, dispatch_target)
            .await?;
        prepared.execute().await
    }

    fn prepare_unary<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        dispatch_target: Option<ExecutionTargetPin>,
    ) -> Pin<Box<impl Future<Output = UnaryPreparation<I, O, E, P>> + Send + '_>>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        Box::pin(self.prepare_unary_inner::<I, O, E>(snapshot, envelope, dispatch_target))
    }

    async fn prepare_unary_inner<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        dispatch_target: Option<ExecutionTargetPin>,
    ) -> UnaryPreparation<I, O, E, P>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
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
            mut input,
        } = envelope;
        let mut receipt = ReceiptBuilder::new(
            &snapshot,
            lineage.invocation_id(),
            lineage.parent_invocation_id(),
            protocol,
            principal.summary(),
            &controls,
        );
        receipt.extension_context = extension_context.clone();
        let mut hook_context = InterfaceHookContext::new(
            principal.summary(),
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        )
        .with_extension_context(extension_context);
        if let Some(plan) = snapshot.plan(&binding_id) {
            if plan.event_observer.is_some()
                || (plan.document_observer.is_some() && plan.document_operation.is_some())
            {
                receipt.event_principal =
                    crate::InterfaceEventPrincipal::from_principal(&principal);
            }
            receipt.resolve(plan);
        }
        // Preserve ownership through the shared failure finalization await.
        let plan;
        let definition;
        let binding;
        let hook_plan;
        let mut admission_reference = None;
        let mut authorization_decision = None;
        let (error, terminal) = 'preparation: {
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
                break 'preparation (
                    interruption_error(interruption),
                    InterfaceInvocationTerminal::Cancelled,
                );
            }
            plan = match snapshot.plan(&binding_id).cloned() {
                Some(plan) => plan,
                None => {
                    break 'preparation (
                        InterfaceInvocationError::UnknownBinding,
                        InterfaceInvocationTerminal::Rejected,
                    );
                }
            };
            definition = plan.definition().clone();
            binding = plan.binding().clone();
            hook_plan = plan.hook_plan::<I, O>();
            if plan.has_executable_extensions() && hook_plan.is_none() {
                break 'preparation (
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                );
            }
            receipt.resolve(&plan);
            let validation = InvocationMetadata {
                plan: &plan,
                definition: &definition,
                binding: &binding,
                protocol,
                authentication_adapter: &authentication_adapter,
                authentication_activation: &authentication_activation,
                authorization_reference: &|| self.authorization.adapter_reference(),
                target_admission: self.target_admission.as_deref(),
                principal_profile: P::PROFILE,
                contracts: [
                    contract_identity::<I>,
                    contract_identity::<O>,
                    contract_identity::<E>,
                ],
                stream_event: None,
            }
            .validate(&mut receipt, &mut admission_reference);
            if let Err(error) = validation {
                break 'preparation (error, InterfaceInvocationTerminal::Rejected);
            }
            if let Some(factory) = &plan.managed_factory {
                let frozen = await_in_flight(
                    &controls,
                    factory.freeze(crate::ManagedInterfaceFreezeRequest {
                        definition: definition.clone(),
                        context: hook_context.clone(),
                        input: crate::ManagedInterfaceProjection::from_contract(&input),
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
                        break 'preparation (
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        );
                    }
                }
            }

            let authorization = self
                .authorization
                .authorize(InterfaceAuthorizationRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                ));
            let decisions = InvocationDecisions {
                principal: &principal,
                plan: &plan,
                definition: &definition,
                binding: &binding,
                protocol,
                target_admission: self.target_admission.as_deref(),
                authorization_bindings: authorization_bindings::<I, O>,
                admission_bindings: admission_bindings::<I, O>,
                controls: &controls,
                context: &hook_context,
                receipt: &mut receipt,
                authorization_decision: &mut authorization_decision,
            };
            if let Err(failure) = decisions.run(authorization).await {
                break 'preparation match failure {
                    DecisionFailure::Interrupted(interruption) => (
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    ),
                    DecisionFailure::Rejected(error) => {
                        run_failure_hooks(
                            &mut receipt.finalization,
                            hook_plan,
                            &hook_context,
                            error.classification(),
                        )
                        .await;
                        (error, InterfaceInvocationTerminal::Rejected)
                    }
                };
            }
            if let Some(hook_plan) = hook_plan {
                let before =
                    await_in_flight(&controls, hook_plan.run_before(&hook_context, &mut input))
                        .await;
                match before {
                    Err(interruption) => {
                        break 'preparation (
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        );
                    }
                    Ok(Err(error)) => {
                        run_failure_hooks(
                            &mut receipt.finalization,
                            Some(hook_plan),
                            &hook_context,
                            error.classification(),
                        )
                        .await;
                        break 'preparation (
                            InterfaceInvocationError::BeforeHookRejected(error),
                            InterfaceInvocationTerminal::Rejected,
                        );
                    }
                    Ok(Ok(())) => {}
                }
            }
            receipt.observe_event_before(&input).await;
            if let Some(session) = &receipt.document_operation {
                if session.is_static() {
                    let _ = session.observe_before(&input).await;
                }
            }
            receipt.stage(InterfaceInvocationStage::Prepared);
            let Some(handler) = snapshot.handler::<I, O, E, P>(definition.interface_id()) else {
                break 'preparation (
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                );
            };
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
            let mut context = InterfaceHandlerContext::new(
                principal,
                attempt,
                lineage.invocation_id(),
                snapshot.graph_fingerprint().clone(),
                snapshot.fingerprint().clone(),
            );
            context.document_operation = receipt.document_operation.clone();
            return Ok(Box::new(PreparedUnaryInvocation {
                snapshot,
                plan,
                receipt,
                hook_context,
                controls,
                handler,
                context,
                input,
            }));
        };
        Err(receipt.fail(error, terminal).await)
    }
}

type UnaryPreparation<I, O, E, P> =
    Result<Box<PreparedUnaryInvocation<I, O, E, P>>, InterfaceInvocationFailure>;

// One invocation retains its frozen plan and completion budget across the phase
// boundary. Boxing the handoff also keeps the driver's returned state small.
struct PreparedUnaryInvocation<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    snapshot: Arc<CompiledInterfaceRegistry>,
    plan: crate::CompiledInvocationPlan,
    receipt: ReceiptBuilder,
    hook_context: InterfaceHookContext,
    controls: InvocationControls,
    handler: Arc<dyn crate::InterfaceHandler<I, O, E, P>>,
    context: InterfaceHandlerContext<P>,
    input: I,
}

impl<I, O, E, P> PreparedUnaryInvocation<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    #[expect(
        clippy::boxed_local,
        reason = "The prepare/execute handoff stays pointer-sized in the driver poll frame for unbounded input I."
    )]
    fn execute(
        self: Box<Self>,
    ) -> Pin<Box<impl Future<Output = InterfaceInvocationResult<O>> + Send>> {
        let PreparedUnaryInvocation {
            snapshot,
            plan,
            mut receipt,
            hook_context,
            controls,
            handler,
            context,
            input,
        } = *self;
        // Construct the handler future after preparation has exited, and outside
        // the driver's poll frame. The original controls still own its polling.
        let target = handler.invoke(context, input);
        Box::pin(async move {
            let _snapshot = snapshot;
            let _handler = handler;
            let hook_plan = plan.hook_plan::<I, O>();
            let target = await_in_flight(&controls, target).await;
            let target = match target {
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
            match target {
                Ok(value) => {
                    receipt.project_event_output(&value).await;
                    if let Some(hook_plan) = hook_plan {
                        hook_plan
                            .run_after(&mut receipt.finalization, &hook_context, &value)
                            .await;
                    }
                    if let Some(session) = &receipt.document_operation {
                        session
                            .project_output(&value, &mut receipt.finalization)
                            .await;
                    }
                    receipt.stage(InterfaceInvocationStage::PostProcessed);
                    Ok(InterfaceInvocationOutcome {
                        value,
                        receipt: receipt.complete().await,
                    })
                }
                Err(error) => {
                    if let Some(hook_plan) = hook_plan {
                        run_failure_hooks(
                            &mut receipt.finalization,
                            Some(hook_plan),
                            &hook_context,
                            error.classification(),
                        )
                        .await;
                    }
                    Err(receipt
                        .fail(target_error(error), InterfaceInvocationTerminal::Failed)
                        .await)
                }
            }
        })
    }
}
