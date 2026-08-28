//! Protocol-independent active interface definitions and invocation contracts.

mod hook;
mod identity;
mod invocation;
mod registry;

pub use hook::{
    InterfaceAfterHook, InterfaceAfterHookFuture, InterfaceBeforeHook, InterfaceBeforeHookError,
    InterfaceBeforeHookFuture, InterfaceCompletionHook, InterfaceCompletionHookFuture,
    InterfaceFailureHook, InterfaceFailureHookFuture, InterfaceHookContext, TypedInterfaceHookPlan,
};
pub use identity::{
    ContractIdentity, GraphFingerprint, HandlerReference, IdentityError, InterfaceId,
    InterfaceOwner, PermissionIdentity, RegistryFingerprint, RouteIdentity, TargetReference,
};
pub use invocation::{
    InterfaceAuthorizationError, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceInvocationError, InterfaceInvocationFailure,
    InterfaceInvocationKernel, InterfaceInvocationOutcome, InterfaceInvocationReceipt,
    InterfaceInvocationResult, InterfaceInvocationStage, InterfaceInvocationTerminal,
    InterfaceProtocol, InterfaceTargetAdmissionError, InterfaceTargetAdmissionFuture,
    InterfaceTargetAdmissionPort, InterfaceTargetAdmissionRequest, InvocationEnvelope,
    InvocationId, InvocationLineage, InvocationLineageError,
};
pub use registry::{
    CompiledInterfaceRegistry, DynamicInterfaceRegistry, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceContract, InterfaceDefinition, InterfaceErrorPolicy,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceLifecycle,
    InterfaceScope, InterfaceTargetError, RegistryCompilationError, RegistryCompiler,
};

#[cfg(test)]
mod _tests;
