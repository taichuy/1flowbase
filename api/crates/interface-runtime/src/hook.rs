use std::{future::Future, pin::Pin, sync::Arc};

use thiserror::Error;

use crate::{
    ExtensionPlanFingerprint, GraphFingerprint, InterfaceContract, InterfaceInvocationTerminal,
    InvocationId, PrincipalSummary, RegistryFingerprint,
};

#[derive(Clone, Debug)]
pub struct InterfaceHookContext {
    principal: PrincipalSummary,
    invocation_id: InvocationId,
    graph_fingerprint: GraphFingerprint,
    registry_fingerprint: RegistryFingerprint,
}

impl InterfaceHookContext {
    pub(crate) fn new(
        principal: PrincipalSummary,
        invocation_id: InvocationId,
        graph_fingerprint: GraphFingerprint,
        registry_fingerprint: RegistryFingerprint,
    ) -> Self {
        Self {
            principal,
            invocation_id,
            graph_fingerprint,
            registry_fingerprint,
        }
    }

    pub fn principal(&self) -> &PrincipalSummary {
        &self.principal
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
    extension_plan_fingerprint: ExtensionPlanFingerprint,
    before: Vec<Arc<dyn InterfaceBeforeHook<I>>>,
    after: Vec<Arc<dyn InterfaceAfterHook<O>>>,
    failure: Vec<Arc<dyn InterfaceFailureHook>>,
    completion: Vec<Arc<dyn InterfaceCompletionHook>>,
}

impl<I, O> TypedInterfaceHookPlan<I, O>
where
    I: InterfaceContract,
    O: InterfaceContract,
{
    pub fn new(
        graph_fingerprint: GraphFingerprint,
        extension_plan_fingerprint: ExtensionPlanFingerprint,
    ) -> Self {
        Self {
            graph_fingerprint,
            extension_plan_fingerprint,
            before: Vec::new(),
            after: Vec::new(),
            failure: Vec::new(),
            completion: Vec::new(),
        }
    }

    pub fn bind_before(mut self, hook: Arc<dyn InterfaceBeforeHook<I>>) -> Self {
        self.before.push(hook);
        self
    }

    pub fn bind_after(mut self, hook: Arc<dyn InterfaceAfterHook<O>>) -> Self {
        self.after.push(hook);
        self
    }

    pub fn bind_failure(mut self, hook: Arc<dyn InterfaceFailureHook>) -> Self {
        self.failure.push(hook);
        self
    }

    pub fn bind_completion(mut self, hook: Arc<dyn InterfaceCompletionHook>) -> Self {
        self.completion.push(hook);
        self
    }

    pub fn graph_fingerprint(&self) -> &GraphFingerprint {
        &self.graph_fingerprint
    }

    pub fn extension_plan_fingerprint(&self) -> &ExtensionPlanFingerprint {
        &self.extension_plan_fingerprint
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

    pub(crate) async fn run_after(&self, context: &InterfaceHookContext, output: &O) {
        for hook in self.after.iter().rev() {
            hook.after(context.clone(), output).await;
        }
    }

    pub(crate) async fn run_failure(&self, context: &InterfaceHookContext, classification: &str) {
        for hook in self.failure.iter().rev() {
            hook.failed(context.clone(), classification).await;
        }
    }

    pub(crate) async fn run_completion(
        &self,
        context: &InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) {
        for hook in self.completion.iter().rev() {
            hook.completed(context.clone(), terminal).await;
        }
    }
}
