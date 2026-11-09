//! Typed projections and handler operations; no common lifecycle driver.
use super::*;

// The result boxes reuse the allocations returned by erase_target. I/P are
// moved into the handler before O/E enter this state, without cloning values.
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
    Result(Result<Box<O>, Box<InterfaceTargetFailure<E>>>),
    Invoking,
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
        let Phase::Result(Ok(output)) = &self.phase else {
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
        Box::pin(async move {
            receipt.observe_event_before(self.input()).await;
            if let Some(session) = &receipt.document_operation {
                if session.is_static() {
                    let _ = session.observe_before(self.input()).await;
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
        let Phase::Input { principal, input } = std::mem::replace(&mut self.phase, Phase::Invoking)
        else {
            unreachable!("private unary target preparation consumes its input once");
        };
        let mut context =
            InterfaceHandlerContext::new(principal, attempt, invocation_id, graph, registry);
        context.document_operation = document;
        self.phase = Phase::Prepared { context, input };
    }

    fn begin_target(&mut self) -> UnaryTargetFuture {
        let Phase::Prepared { context, input } =
            std::mem::replace(&mut self.phase, Phase::Invoking)
        else {
            unreachable!("private unary execution consumes its prepared input once");
        };
        let handler = self
            .handler
            .as_ref()
            .expect("validated typed unary handler");
        // Invoke synchronously; the wrapper only polls the resulting target.
        erase_target(handler.invoke(context, input))
    }

    fn finish_target(&mut self, result: UnaryTargetResult) -> bool {
        let result = match result {
            Ok(output) => Ok(output
                .downcast::<O>()
                .expect("private unary target output type")),
            Err(error) => Err(error
                .downcast::<InterfaceTargetFailure<E>>()
                .expect("private unary target error type")),
        };
        let success = result.is_ok();
        self.phase = Phase::Result(result);
        success
    }

    fn observe_success<'a>(
        &'a self,
        plan: &'a crate::CompiledInvocationPlan,
        receipt: &'a mut ReceiptBuilder,
        context: &'a InterfaceHookContext,
    ) -> UnaryStageFuture<'a> {
        Box::pin(async move {
            receipt.project_event_output(self.output()).await;
            if let Some(hooks) = plan.hook_plan::<I, O>() {
                hooks
                    .run_after(&mut receipt.finalization, context, self.output())
                    .await;
            }
            if let Some(session) = &receipt.document_operation {
                session
                    .project_output(self.output(), &mut receipt.finalization)
                    .await;
            }
        })
    }

    fn error_classification(&self) -> &str {
        let Phase::Result(Err(error)) = &self.phase else {
            unreachable!("private unary failure owns its concrete error");
        };
        error.classification()
    }

    fn take_error(&mut self) -> InterfaceInvocationError {
        let Phase::Result(Err(error)) = std::mem::replace(&mut self.phase, Phase::Invoking) else {
            unreachable!("private unary failure consumes its concrete error once");
        };
        target_error(*error)
    }

    fn take_output(&mut self) -> UnaryValue {
        let Phase::Result(Ok(output)) = std::mem::replace(&mut self.phase, Phase::Invoking) else {
            unreachable!("private unary success consumes its output once");
        };
        output
    }
}

// This leaf is generic only in target O/E. It preserves the original concrete
// source E until the driver finishes Failure and calls take_error.
fn erase_target<O: InterfaceContract, E: InterfaceContract>(
    target: crate::InterfaceHandlerFuture<O, E>,
) -> UnaryTargetFuture {
    Box::pin(async move {
        match target.await {
            Ok(output) => Ok(Box::new(output) as UnaryValue),
            Err(error) => Err(Box::new(error) as UnaryValue),
        }
    })
}
