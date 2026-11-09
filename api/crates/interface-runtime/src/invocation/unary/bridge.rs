//! Typed projections and handler operations; no common lifecycle driver.
use super::*;

// Native target/result pointer slots keep O/E boxed. I/P are moved into
// the handler before its result enters this state, without cloning values.
enum Phase<I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    Input {
        principal: P,
        input: I,
    },
    Prepared {
        context: InterfaceHandlerContext<P>,
        input: I,
    },
    Invoking {
        target: Option<crate::InterfaceHandlerFuture<O, E>>,
        result: Option<Result<Box<O>, Box<InterfaceTargetFailure<E>>>>,
    },
}

pub(super) struct TypedUnaryBridge<'a, I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    authorization: &'a dyn InterfaceAuthorizationPort<P>,
    phase: Phase<I, O, E, P>,
    handler: Option<Arc<dyn crate::InterfaceHandler<I, O, E, P>>>,
}

impl<'a, I, O, E, P> TypedUnaryBridge<'a, I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    pub(super) fn new(
        authorization: &'a dyn InterfaceAuthorizationPort<P>,
        principal: P,
        input: I,
    ) -> Self {
        Self {
            authorization,
            phase: Phase::Input { principal, input },
            handler: None,
        }
    }

    fn principal(&self) -> &P {
        let Phase::Input { principal, .. } = &self.phase else {
            unreachable!("private unary preparation owns the full principal");
        };
        principal
    }

    fn input(&self) -> &I {
        let Phase::Input { input, .. } = &self.phase else {
            unreachable!("private unary preparation owns the input");
        };
        input
    }

    fn output(&self) -> &O {
        let Phase::Invoking {
            result: Some(Ok(output)),
            ..
        } = &self.phase
        else {
            unreachable!("private unary success owns the output");
        };
        output
    }
}

impl<I, O, E, P> UnaryBridge for TypedUnaryBridge<'_, I, O, E, P>
where
    I: InterfaceContract,
    O: InterfaceContract,
    E: InterfaceContract,
    P: InvocationPrincipal,
{
    fn summary(&self) -> PrincipalSummary {
        self.principal().summary()
    }

    fn event_principal(&self) -> Option<crate::InterfaceEventPrincipal> {
        crate::InterfaceEventPrincipal::from_principal(self.principal())
    }

    fn authorization_reference(&self) -> AuthorizationAdapterReference {
        self.authorization.adapter_reference()
    }

    fn principal_profile(&self) -> crate::PrincipalProfile {
        P::PROFILE
    }

    fn contracts_match(&self, definition: &InterfaceDefinition) -> bool {
        definition.input_contract() == &contract_identity::<I>()
            && definition.output_contract() == &contract_identity::<O>()
            && definition.target_error_contract() == &contract_identity::<E>()
    }

    fn has_hooks(&self, plan: &crate::CompiledInvocationPlan) -> bool {
        plan.hook_plan::<I, O>().is_some()
    }

    fn authorization<'p>(
        &self,
        plan: &'p crate::CompiledInvocationPlan,
    ) -> Option<AuthorizationContributions<'p>> {
        plan.authorization_plan::<I, O>()
            .map(|plan| plan.contributions())
    }

    fn admission<'p>(
        &self,
        plan: &'p crate::CompiledInvocationPlan,
    ) -> Option<AdmissionContributions<'p>> {
        plan.admission_plan::<I, O>()
            .map(|plan| plan.contributions())
    }

    fn failure<'p>(&self, plan: &'p crate::CompiledInvocationPlan) -> Option<FailureObservers<'p>> {
        plan.hook_plan::<I, O>()
            .map(|plan| plan.failure_observers())
    }

    fn project_input(&self) -> Option<crate::ManagedInterfaceProjection> {
        crate::ManagedInterfaceProjection::from_contract(self.input())
    }

    fn authorize(
        &self,
        definition: InterfaceDefinition,
        binding: ProtocolBinding,
        protocol: InterfaceProtocol,
    ) -> InterfaceAuthorizationFuture<'_> {
        self.authorization
            .authorize(InterfaceAuthorizationRequest::new(
                self.principal().clone(),
                definition,
                binding,
                protocol,
            ))
    }

    fn before<'a>(
        &'a mut self,
        plan: &'a crate::CompiledInvocationPlan,
        context: &'a InterfaceHookContext,
    ) -> crate::InterfaceBeforeHookFuture<'a> {
        let Phase::Input { input, .. } = &mut self.phase else {
            unreachable!("private unary Before owns the mutable input");
        };
        let hooks = plan
            .hook_plan::<I, O>()
            .expect("validated typed unary hooks");
        Box::pin(hooks.run_before(context, input))
    }

    fn observe_before<'a>(&'a self, receipt: &'a mut ReceiptBuilder) -> UnaryStageFuture<'a> {
        let input = self.input();
        Box::pin(async move {
            receipt.observe_event_before(input).await;
            if let Some(session) = &receipt.document_operation {
                if session.is_static() {
                    let _ = session.observe_before(input).await;
                }
            }
        })
    }

    fn bind_handler(&mut self, snapshot: &CompiledInterfaceRegistry, id: &InterfaceId) -> bool {
        self.handler = snapshot.handler::<I, O, E, P>(id);
        self.handler.is_some()
    }

    fn prepare_target(
        &mut self,
        attempt: ExecutionAttempt,
        invocation_id: InvocationId,
        graph: GraphFingerprint,
        registry: RegistryFingerprint,
        document: Option<crate::DocumentOperationSession>,
    ) {
        let Phase::Input { principal, input } = std::mem::replace(
            &mut self.phase,
            Phase::Invoking {
                target: None,
                result: None,
            },
        ) else {
            unreachable!("private unary target preparation consumes its input once");
        };
        let mut context =
            InterfaceHandlerContext::new(principal, attempt, invocation_id, graph, registry);
        context.document_operation = document;
        self.phase = Phase::Prepared { context, input };
    }

    fn begin_target(&mut self) {
        let Phase::Prepared { context, input } = std::mem::replace(
            &mut self.phase,
            Phase::Invoking {
                target: None,
                result: None,
            },
        ) else {
            unreachable!("private unary execution consumes its prepared input once");
        };
        let handler = self
            .handler
            .as_ref()
            .expect("validated typed unary handler");
        let future = handler.invoke(context, input);
        let Phase::Invoking { target, .. } = &mut self.phase else {
            unreachable!("private unary execution initialized its target slot");
        };
        *target = Some(future);
    }

    fn poll_target(&mut self, cx: &mut Context<'_>) -> Poll<bool> {
        let Phase::Invoking { target, result } = &mut self.phase else {
            unreachable!("private unary execution owns its target slot");
        };
        assert!(
            result.is_none(),
            "private unary target and result slots cannot overlap"
        );
        let outcome = std::task::ready!(target
            .as_mut()
            .expect("private unary target is active")
            .as_mut()
            .poll(cx));
        drop(target.take());
        let success = outcome.is_ok();
        *result = Some(match outcome {
            Ok(output) => Ok(Box::new(output)),
            Err(error) => Err(Box::new(error)),
        });
        Poll::Ready(success)
    }

    fn abort_target(&mut self) {
        let Phase::Invoking { target, result } = &mut self.phase else {
            unreachable!("private unary execution owns its abort slots");
        };
        drop(target.take());
        drop(result.take());
    }

    fn observe_success<'a>(
        &'a self,
        plan: &'a crate::CompiledInvocationPlan,
        receipt: &'a mut ReceiptBuilder,
        context: &'a InterfaceHookContext,
    ) -> UnaryStageFuture<'a> {
        let output = self.output();
        Box::pin(async move {
            receipt.project_event_output(output).await;
            if let Some(hooks) = plan.hook_plan::<I, O>() {
                hooks
                    .run_after(&mut receipt.finalization, context, output)
                    .await;
            }
            if let Some(session) = &receipt.document_operation {
                session
                    .project_output(output, &mut receipt.finalization)
                    .await;
            }
        })
    }

    fn error_classification(&self) -> &str {
        let Phase::Invoking {
            result: Some(Err(error)),
            ..
        } = &self.phase
        else {
            unreachable!("private unary failure owns its concrete error");
        };
        error.classification()
    }

    fn take_error(&mut self) -> InterfaceInvocationError {
        let Phase::Invoking { result, .. } = &mut self.phase else {
            unreachable!("private unary failure owns its result slot");
        };
        let Some(Err(error)) = result.take() else {
            unreachable!("private unary failure consumes its concrete error once");
        };
        target_error(*error)
    }

    fn take_output(&mut self) -> UnaryValue {
        let Phase::Invoking { result, .. } = &mut self.phase else {
            unreachable!("private unary success owns its result slot");
        };
        let Some(Ok(output)) = result.take() else {
            unreachable!("private unary success consumes its output once");
        };
        output
    }
}

#[cfg(test)]
#[path = "_tests/runtime_revision_tests.rs"]
mod runtime_revision_tests;
