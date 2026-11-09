use std::{any::Any, future::Future, pin::Pin, sync::Arc};

use crate::finalization::InvocationFinalization;
use thiserror::Error;

use crate::{
    ContractIdentity, GraphFingerprint, InterfaceContract, InterfaceExtensionPoint,
    InterfaceInvocationTerminal, InvocationId, PluginIdentity, PrincipalSummary,
    RegistryFingerprint,
};

/// Opaque host-only context retained by the invocation through Completion. Never serialized.
#[derive(Clone)]
pub(crate) struct InvocationExtensionContext(pub(crate) Arc<dyn Any + Send + Sync>);
impl std::fmt::Debug for InvocationExtensionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InvocationExtensionContext(<host-owned>)")
    }
}

#[derive(Clone, Debug)]
pub struct InterfaceHookContext {
    principal: Option<PrincipalSummary>,
    invocation_id: InvocationId,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
    extension_context: Option<InvocationExtensionContext>,
    pub(crate) managed_invocation: Option<ManagedInvocationContext>,
    observer_failed: Arc<std::sync::atomic::AtomicBool>,
}

impl InterfaceHookContext {
    pub(crate) fn new(
        principal: PrincipalSummary,
        invocation_id: InvocationId,
        graph_fingerprint: GraphFingerprint,
        registry_fingerprint: RegistryFingerprint,
    ) -> Self {
        Self {
            principal: Some(principal),
            invocation_id,
            graph_fingerprint,
            registry_fingerprint,
            extension_context: None,
            managed_invocation: None,
            observer_failed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub(crate) fn unestablished(
        invocation_id: InvocationId,
        graph_fingerprint: GraphFingerprint,
        registry_fingerprint: RegistryFingerprint,
    ) -> Self {
        Self {
            principal: None,
            invocation_id,
            graph_fingerprint,
            registry_fingerprint,
            extension_context: None,
            managed_invocation: None,
            observer_failed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub(crate) fn with_extension_context(
        mut self,
        context: Option<InvocationExtensionContext>,
    ) -> Self {
        self.extension_context = context;
        self
    }

    pub fn extension_context<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.extension_context.as_ref()?.0.downcast_ref()
    }

    /// Diagnostic only: an observer's failure must not replace the invocation's main result.
    pub fn report_observer_failure(&self) {
        self.observer_failed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn observer_context(&self) -> Self {
        let mut context = self.clone();
        context.observer_failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        context
    }

    pub fn principal(&self) -> Option<&PrincipalSummary> {
        self.principal.as_ref()
    }

    pub fn invocation_id(&self) -> InvocationId {
        self.invocation_id
    }

    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn registry_fingerprint(&self) -> &RegistryFingerprint {
        &self.registry_fingerprint
    }
}

#[derive(Debug, Error)]
#[error("interface before hook rejected with {classification}")]
pub struct InterfaceBeforeHookError {
    classification: Arc<str>,
}

impl InterfaceBeforeHookError {
    pub fn classified(classification: impl AsRef<str>) -> Self {
        Self {
            classification: Arc::from(classification.as_ref()),
        }
    }

    pub fn classification(&self) -> &str {
        self.classification.as_ref()
    }
}

pub type InterfaceBeforeHookFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), InterfaceBeforeHookError>> + Send + 'a>>;

pub trait InterfaceBeforeHook<I>: Send + Sync + 'static
where
    I: InterfaceContract,
{
    fn before<'a>(
        &'a self,
        context: InterfaceHookContext,
        input: &'a mut I,
    ) -> InterfaceBeforeHookFuture<'a>;
}

pub type InterfaceAfterHookFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait InterfaceAfterHook<O>: Send + Sync + 'static
where
    O: InterfaceContract,
{
    fn after<'a>(
        &'a self,
        context: InterfaceHookContext,
        output: &'a O,
    ) -> InterfaceAfterHookFuture<'a>;
}

pub type InterfaceFailureHookFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait InterfaceFailureHook: Send + Sync + 'static {
    fn failed<'a>(
        &'a self,
        context: InterfaceHookContext,
        classification: &'a str,
    ) -> InterfaceFailureHookFuture<'a>;
}

pub type InterfaceCompletionHookFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait InterfaceCompletionHook: Send + Sync + 'static {
    fn completed(
        &self,
        context: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_>;
}

pub struct TypedInterfaceHookPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    graph_fingerprint: GraphFingerprint,
    input_contract: ContractIdentity,
    output_contract: ContractIdentity,
    bindings: Vec<(PluginIdentity, InterfaceExtensionPoint)>,
    before: Vec<Arc<dyn InterfaceBeforeHook<I>>>,
    after: Vec<Arc<dyn InterfaceAfterHook<O>>>,
    failure: Vec<Arc<dyn InterfaceFailureHook>>,
    completion: Vec<Arc<dyn InterfaceCompletionHook>>,
}

impl<I, O> Clone for TypedInterfaceHookPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    fn clone(&self) -> Self {
        Self {
            graph_fingerprint: self.graph_fingerprint.clone(),
            input_contract: self.input_contract.clone(),
            output_contract: self.output_contract.clone(),
            bindings: self.bindings.clone(),
            before: self.before.clone(),
            after: self.after.clone(),
            failure: self.failure.clone(),
            completion: self.completion.clone(),
        }
    }
}

impl<I, O> TypedInterfaceHookPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    pub fn new(graph_fingerprint: GraphFingerprint) -> Self {
        Self {
            graph_fingerprint,
            input_contract: typed_contract_identity::<I>(),
            output_contract: typed_contract_identity::<O>(),
            bindings: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            failure: Vec::new(),
            completion: Vec::new(),
        }
    }

    pub fn bind_before(
        mut self,
        plugin: PluginIdentity,
        hook: Arc<dyn InterfaceBeforeHook<I>>,
    ) -> Self {
        self.bindings
            .push((plugin, InterfaceExtensionPoint::Before));
        self.before.push(hook);
        self
    }

    pub fn bind_after(
        mut self,
        plugin: PluginIdentity,
        hook: Arc<dyn InterfaceAfterHook<O>>,
    ) -> Self {
        self.bindings.push((plugin, InterfaceExtensionPoint::After));
        self.after.push(hook);
        self
    }

    pub fn bind_failure(
        mut self,
        plugin: PluginIdentity,
        hook: Arc<dyn InterfaceFailureHook>,
    ) -> Self {
        self.bindings
            .push((plugin, InterfaceExtensionPoint::Failure));
        self.failure.push(hook);
        self
    }

    pub fn bind_completion(
        mut self,
        plugin: PluginIdentity,
        hook: Arc<dyn InterfaceCompletionHook>,
    ) -> Self {
        self.bindings
            .push((plugin, InterfaceExtensionPoint::Completion));
        self.completion.push(hook);
        self
    }

    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn input_contract(&self) -> &ContractIdentity {
        &self.input_contract
    }

    pub fn output_contract(&self) -> &ContractIdentity {
        &self.output_contract
    }

    pub(crate) fn bindings(&self) -> &[(PluginIdentity, InterfaceExtensionPoint)] {
        &self.bindings
    }

    pub(crate) async fn run_before(
        &self,
        context: &InterfaceHookContext,
        input: &mut I,
    ) -> Result<(), InterfaceBeforeHookError> {
        for hook in &self.before {
            hook.before(context.clone(), input).await?;
        }
        Ok(())
    }

    pub(crate) async fn run_after(
        &self,
        budget: &mut InvocationFinalization,
        context: &InterfaceHookContext,
        output: &O,
    ) {
        for (plugin, hook) in self
            .observer_plugins(InterfaceExtensionPoint::After)
            .into_iter()
            .zip(&self.after)
            .rev()
        {
            let observed = context.observer_context();
            let record = budget
                .observe(plugin, InterfaceExtensionPoint::After, || {
                    hook.after(observed.clone(), output)
                })
                .await;
            if observed
                .observer_failed
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                budget.record_reported_failure(record);
            }
        }
    }

    pub(crate) async fn run_failure(
        &self,
        budget: &mut InvocationFinalization,
        context: &InterfaceHookContext,
        classification: &str,
    ) {
        for (plugin, hook) in self
            .observer_plugins(InterfaceExtensionPoint::Failure)
            .into_iter()
            .zip(&self.failure)
            .rev()
        {
            let observed = context.observer_context();
            let record = budget
                .observe(plugin, InterfaceExtensionPoint::Failure, || {
                    hook.failed(observed.clone(), classification)
                })
                .await;
            if observed
                .observer_failed
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                budget.record_reported_failure(record);
            }
        }
    }

    fn observer_plugins(&self, point: InterfaceExtensionPoint) -> Vec<&PluginIdentity> {
        self.bindings
            .iter()
            .filter(|(_, p)| *p == point)
            .map(|(plugin, _)| plugin)
            .collect()
    }

    pub(crate) async fn run_completion(
        &self,
        budget: &mut InvocationFinalization,
        context: &InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) {
        for (plugin, hook) in self
            .observer_plugins(InterfaceExtensionPoint::Completion)
            .into_iter()
            .zip(&self.completion)
            .rev()
        {
            let observed = context.observer_context();
            let record = budget
                .observe(plugin, InterfaceExtensionPoint::Completion, || {
                    hook.completed(observed.clone(), terminal)
                })
                .await;
            if observed
                .observer_failed
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                budget.record_reported_failure(record);
            }
        }
    }
}

pub(crate) trait ErasedInterfaceHookPlan: Send + Sync {
    fn graph_fingerprint(&self) -> &GraphFingerprint;
    fn input_contract(&self) -> &ContractIdentity;
    fn output_contract(&self) -> &ContractIdentity;
    fn bindings(&self) -> &[(PluginIdentity, InterfaceExtensionPoint)];
    fn run_completion<'a>(
        &'a self,
        budget: &'a mut InvocationFinalization,
        context: &'a InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'a>;
    fn as_any(&self) -> &dyn Any;
}

impl<I, O> ErasedInterfaceHookPlan for TypedInterfaceHookPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    fn graph_fingerprint(&self) -> &GraphFingerprint {
        self.graph_fingerprint()
    }

    fn input_contract(&self) -> &ContractIdentity {
        self.input_contract()
    }

    fn output_contract(&self) -> &ContractIdentity {
        self.output_contract()
    }

    fn bindings(&self) -> &[(PluginIdentity, InterfaceExtensionPoint)] {
        self.bindings()
    }

    fn run_completion<'a>(
        &'a self,
        budget: &'a mut InvocationFinalization,
        context: &'a InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'a> {
        Box::pin(self.run_completion(budget, context, terminal))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn typed_contract_identity<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("typed hook contract constants must be valid identities")
}

/// Protocol-independent, immutable safe-value calls; no mutation or recovery response exists.
pub enum ManagedInterfaceCall {
    Authorization,
    Admission,
    Before,
    After(Option<crate::ManagedInterfaceProjection>),
    Failure,
    Completion(InterfaceInvocationTerminal),
}

pub struct ManagedInterfaceFreezeRequest {
    pub definition: crate::InterfaceDefinition,
    pub context: InterfaceHookContext,
    pub input: Option<crate::ManagedInterfaceProjection>,
    pub deadline: Option<std::time::SystemTime>,
}

pub type ManagedInterfaceFreezeFuture<'a> = Pin<
    Box<dyn Future<Output = Result<Arc<dyn ManagedInterfaceInvocation>, &'static str>> + Send + 'a>,
>;
pub type ManagedInterfaceCallFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send + 'a>>;

pub trait ManagedInterfaceInvocationFactory: Send + Sync + 'static {
    fn freeze(&self, request: ManagedInterfaceFreezeRequest) -> ManagedInterfaceFreezeFuture<'_>;
}
pub trait ManagedInterfaceInvocation: Send + Sync + 'static {
    fn run(
        &self,
        context: InterfaceHookContext,
        call: ManagedInterfaceCall,
    ) -> ManagedInterfaceCallFuture<'_>;
}

#[derive(Clone)]
pub(crate) struct ManagedInvocationContext(pub(crate) Arc<dyn ManagedInterfaceInvocation>);
impl std::fmt::Debug for ManagedInvocationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ManagedInvocationContext(<frozen-host-owned>)")
    }
}

pub(crate) fn managed_bridge_identity(point: InterfaceExtensionPoint) -> PluginIdentity {
    PluginIdentity::new(format!("interface-runtime.managed.{point:?}").to_lowercase())
        .expect("static managed bridge identity")
}

pub(crate) struct ManagedLifecycleBridge;
impl ManagedLifecycleBridge {
    async fn run(
        context: InterfaceHookContext,
        call: ManagedInterfaceCall,
    ) -> Result<(), &'static str> {
        // Invalid/unestablished attempts have no workspace execution authority to freeze.
        let Some(invocation) = context.managed_invocation.clone() else {
            return Ok(());
        };
        invocation.0.run(context, call).await
    }
}
impl crate::InterfaceAuthorizationContribution for ManagedLifecycleBridge {
    fn authorize(
        &self,
        request: crate::InterfaceAuthorizationContributionRequest,
    ) -> crate::InterfaceAuthorizationContributionFuture<'_> {
        Box::pin(async move {
            Self::run(
                request.context().clone(),
                ManagedInterfaceCall::Authorization,
            )
            .await
            .map_err(crate::InterfaceAuthorizationContributionError::classified)
        })
    }
}
impl crate::InterfaceAdmissionContribution for ManagedLifecycleBridge {
    fn admit(
        &self,
        request: crate::InterfaceAdmissionContributionRequest,
    ) -> crate::InterfaceAdmissionContributionFuture<'_> {
        Box::pin(async move {
            Self::run(request.context().clone(), ManagedInterfaceCall::Admission)
                .await
                .map_err(crate::InterfaceAdmissionContributionError::classified)
        })
    }
}
impl<I: InterfaceContract> InterfaceBeforeHook<I> for ManagedLifecycleBridge {
    fn before<'a>(
        &'a self,
        context: InterfaceHookContext,
        _input: &'a mut I,
    ) -> InterfaceBeforeHookFuture<'a> {
        Box::pin(async move {
            Self::run(context, ManagedInterfaceCall::Before)
                .await
                .map_err(InterfaceBeforeHookError::classified)
        })
    }
}
impl<O: InterfaceContract> InterfaceAfterHook<O> for ManagedLifecycleBridge {
    fn after<'a>(
        &'a self,
        context: InterfaceHookContext,
        output: &'a O,
    ) -> InterfaceAfterHookFuture<'a> {
        Box::pin(async move {
            if Self::run(
                context.clone(),
                ManagedInterfaceCall::After(crate::ManagedInterfaceProjection::from_contract(
                    output,
                )),
            )
            .await
            .is_err()
            {
                context.report_observer_failure();
            }
        })
    }
}
impl InterfaceFailureHook for ManagedLifecycleBridge {
    fn failed<'a>(
        &'a self,
        context: InterfaceHookContext,
        _classification: &'a str,
    ) -> InterfaceFailureHookFuture<'a> {
        Box::pin(async move {
            if Self::run(context.clone(), ManagedInterfaceCall::Failure)
                .await
                .is_err()
            {
                context.report_observer_failure();
            }
        })
    }
}
impl InterfaceCompletionHook for ManagedLifecycleBridge {
    fn completed(
        &self,
        context: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            if Self::run(context.clone(), ManagedInterfaceCall::Completion(terminal))
                .await
                .is_err()
            {
                context.report_observer_failure();
            }
        })
    }
}
