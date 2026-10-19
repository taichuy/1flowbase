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

    pub async fn invoke<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        // Keep the frozen invocation state out of the caller's inline future.
        // Nested protocol dispatch still uses this same lifecycle and controls.
        Box::pin(self.invoke_internal::<I, O, E>(snapshot, envelope, None)).await
    }

    pub async fn invoke_with_dispatch_target<I, O, E>(
        &self,
        snapshot: Arc<CompiledInterfaceRegistry>,
        envelope: InvocationEnvelope<I, P>,
        target: ExecutionTargetPin,
    ) -> InterfaceInvocationResult<O>
    where
        I: InterfaceContract,
        O: InterfaceContract,
        E: InterfaceContract,
    {
        Box::pin(self.invoke_internal::<I, O, E>(snapshot, envelope, Some(target))).await
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
        if plan.adapter_plan().authorization() != &self.authorization.adapter_reference() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AuthorizationAdapterMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        let admission_reference = self
            .target_admission
            .as_ref()
            .map(|admission| admission.adapter_reference());
        if plan.adapter_plan().admission() != admission_reference.as_ref() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AdmissionAdapterMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if definition.principal_profile() != P::PROFILE {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::PrincipalProfileMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        receipt.stage(InterfaceInvocationStage::PrincipalEstablished);
        if definition.input_contract() != &contract_identity::<I>()
            || definition.stream_event_contract() != Some(&contract_identity::<S>())
            || definition.output_contract() != &contract_identity::<O>()
            || definition.target_error_contract() != &contract_identity::<E>()
        {
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

        let authorization = await_in_flight(
            &controls,
            self.authorization
                .authorize(InterfaceAuthorizationRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
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
                run_failure_hooks(
                    &mut receipt.finalization,
                    hook_plan.as_ref(),
                    &hook_context,
                    error.classification(),
                )
                .await;
                return Err(receipt
                    .fail(
                        InterfaceInvocationError::AuthorizationRejected(error),
                        InterfaceInvocationTerminal::Rejected,
                    )
                    .await);
            }
            Ok(Ok(())) => {}
        }
        if let Some(extensions) = plan.authorization_plan::<I, O>() {
            match await_in_flight(
                &controls,
                extensions.run(InterfaceAuthorizationContributionRequest::new(
                    principal.summary(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                    hook_context.clone(),
                )),
            )
            .await
            {
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
                            InterfaceInvocationError::AuthorizationContributionRejected(error),
                            InterfaceInvocationTerminal::Rejected,
                        )
                        .await);
                }
                Ok(Ok(())) => {}
            }
        }
        let authorization_decision = authorization_decision_fingerprint(&plan);
        receipt.authorized(authorization_decision.clone());
        if let Some(target_admission) = &self.target_admission {
            let admission = await_in_flight(
                &controls,
                target_admission.admit(InterfaceTargetAdmissionRequest::new(
                    principal.summary(),
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
                    run_failure_hooks(
                        &mut receipt.finalization,
                        hook_plan.as_ref(),
                        &hook_context,
                        error.classification(),
                    )
                    .await;
                    return Err(receipt
                        .fail(
                            InterfaceInvocationError::AdmissionRejected(error),
                            InterfaceInvocationTerminal::Rejected,
                        )
                        .await);
                }
                Ok(Ok(())) => {}
            }
        }
        if let Some(extensions) = plan.admission_plan::<I, O>() {
            match await_in_flight(
                &controls,
                extensions.run(InterfaceAdmissionContributionRequest::new(
                    principal.summary(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                    authorization_decision,
                    hook_context.clone(),
                )),
            )
            .await
            {
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
                            InterfaceInvocationError::AdmissionContributionRejected(error),
                            InterfaceInvocationTerminal::Rejected,
                        )
                        .await);
                }
                Ok(Ok(())) => {}
            }
        }
        receipt.stage(InterfaceInvocationStage::Admitted);
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
        let context = InterfaceHandlerContext::new(
            principal,
            attempt,
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
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
                        if let Some(hooks) = hook_plan.as_ref() {
                            hooks
                                .run_after(&mut receipt.finalization, &hook_context, output)
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
        let hook_plan = plan.hook_plan::<I, O>();
        if plan.has_executable_extensions() && hook_plan.is_none() {
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
        if plan.adapter_plan().authorization() != &self.authorization.adapter_reference() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AuthorizationAdapterMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        let admission_reference = self
            .target_admission
            .as_ref()
            .map(|admission| admission.adapter_reference());
        if plan.adapter_plan().admission() != admission_reference.as_ref() {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AdmissionAdapterMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if definition.principal_profile() != P::PROFILE {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::PrincipalProfileMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        receipt.stage(InterfaceInvocationStage::PrincipalEstablished);
        if definition.input_contract() != &contract_identity::<I>()
            || definition.output_contract() != &contract_identity::<O>()
            || definition.target_error_contract() != &contract_identity::<E>()
        {
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

        let authorization = await_in_flight(
            &controls,
            self.authorization
                .authorize(InterfaceAuthorizationRequest::new(
                    principal.clone(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
        )
        .await;
        let authorization = match authorization {
            Ok(authorization) => authorization,
            Err(interruption) => {
                return Err(receipt
                    .fail(
                        interruption_error(interruption),
                        InterfaceInvocationTerminal::Cancelled,
                    )
                    .await);
            }
        };
        if let Err(error) = authorization {
            run_failure_hooks(
                &mut receipt.finalization,
                hook_plan,
                &hook_context,
                error.classification(),
            )
            .await;
            return Err(receipt
                .fail(
                    InterfaceInvocationError::AuthorizationRejected(error),
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
        }
        if let Some(extensions) = plan.authorization_plan::<I, O>() {
            let authorization = await_in_flight(
                &controls,
                extensions.run(InterfaceAuthorizationContributionRequest::new(
                    principal.summary(),
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
                    run_failure_hooks(
                        &mut receipt.finalization,
                        hook_plan,
                        &hook_context,
                        error.classification(),
                    )
                    .await;
                    return Err(receipt
                        .fail(
                            InterfaceInvocationError::AuthorizationContributionRejected(error),
                            InterfaceInvocationTerminal::Rejected,
                        )
                        .await);
                }
                Ok(Ok(())) => {}
            }
        }
        let authorization_decision = authorization_decision_fingerprint(&plan);
        receipt.authorized(authorization_decision.clone());
        if let Some(target_admission) = &self.target_admission {
            let admission = await_in_flight(
                &controls,
                target_admission.admit(InterfaceTargetAdmissionRequest::new(
                    principal.summary(),
                    definition.clone(),
                    binding.clone(),
                    protocol,
                )),
            )
            .await;
            let admission = match admission {
                Ok(admission) => admission,
                Err(interruption) => {
                    return Err(receipt
                        .fail(
                            interruption_error(interruption),
                            InterfaceInvocationTerminal::Cancelled,
                        )
                        .await);
                }
            };
            if let Err(error) = admission {
                run_failure_hooks(
                    &mut receipt.finalization,
                    hook_plan,
                    &hook_context,
                    error.classification(),
                )
                .await;
                return Err(receipt
                    .fail(
                        InterfaceInvocationError::AdmissionRejected(error),
                        InterfaceInvocationTerminal::Rejected,
                    )
                    .await);
            }
        }
        if let Some(extensions) = plan.admission_plan::<I, O>() {
            let admission = await_in_flight(
                &controls,
                extensions.run(InterfaceAdmissionContributionRequest::new(
                    principal.summary(),
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
                    run_failure_hooks(
                        &mut receipt.finalization,
                        hook_plan,
                        &hook_context,
                        error.classification(),
                    )
                    .await;
                    return Err(receipt
                        .fail(
                            InterfaceInvocationError::AdmissionContributionRejected(error),
                            InterfaceInvocationTerminal::Rejected,
                        )
                        .await);
                }
                Ok(Ok(())) => {}
            }
        }
        receipt.stage(InterfaceInvocationStage::Admitted);
        if let Some(hook_plan) = hook_plan {
            let before =
                await_in_flight(&controls, hook_plan.run_before(&hook_context, &mut input)).await;
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
                    run_failure_hooks(
                        &mut receipt.finalization,
                        Some(hook_plan),
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
        receipt.stage(InterfaceInvocationStage::Prepared);
        let Some(handler) = snapshot.handler::<I, O, E, P>(definition.interface_id()) else {
            return Err(receipt
                .fail(
                    InterfaceInvocationError::ContractMismatch,
                    InterfaceInvocationTerminal::Rejected,
                )
                .await);
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
        let context = InterfaceHandlerContext::new(
            principal,
            attempt,
            lineage.invocation_id(),
            snapshot.graph_fingerprint().clone(),
            snapshot.fingerprint().clone(),
        );
        let target = await_in_flight(&controls, handler.invoke(context, input)).await;
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
                if let Some(hook_plan) = hook_plan {
                    hook_plan
                        .run_after(&mut receipt.finalization, &hook_context, &value)
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
    }
}
