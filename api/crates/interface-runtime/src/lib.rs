//! Protocol-independent active interface definitions and invocation contracts.

mod hook;
mod identity;
mod invocation;
mod principal;
mod registry;

pub use hook::{
    InterfaceAfterHook, InterfaceAfterHookFuture, InterfaceBeforeHook, InterfaceBeforeHookError,
    InterfaceBeforeHookFuture, InterfaceCompletionHook, InterfaceCompletionHookFuture,
    InterfaceFailureHook, InterfaceFailureHookFuture, InterfaceHookContext, TypedInterfaceHookPlan,
};
pub use identity::{
    AdmissionAdapterReference, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingFingerprint, BindingId, ContractIdentity,
    ExtensionPlanFingerprint, GraphFingerprint, HandlerReference, IdentityError, InterfaceId,
    InterfaceOwner, InterfaceVersion, PlanFingerprint, RegistryFingerprint, RouteIdentity,
    TargetReference,
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
pub use principal::{
    ApplicationPrincipal, ApplicationPrincipalError, InvocationPrincipal, PrincipalProfile,
    PrincipalSummary, PublicPrincipal, UserCredentialKind, UserPrincipal,
};
pub use registry::{
    CompiledInterfaceRegistry, CompiledInvocationPlan, DynamicInterfaceRegistry, InterfaceAccess,
    InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceContract, InterfaceContracts,
    InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution, InterfaceHandler,
    InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceIdentity, InterfaceLifecycle,
    InterfaceScope, InterfaceTargetError, InvocationAdapterPlan, ProtocolBinding,
    ProtocolProjection, RegistryCompilationError, RegistryCompiler,
};

#[cfg(test)]
mod _tests;
