//! Protocol-independent active interface definitions and invocation contracts.

mod extension;
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
    AdmissionAdapterReference, ArtifactIdentity, AuthenticationAdapterReference,
    AuthorizationAdapterReference, AuthorizationOperation, BindingFingerprint, BindingId,
    ContractIdentity, ExtensionPlanFingerprint, GraphFingerprint, HandlerReference, IdentityError,
    InterfaceId, InterfaceOwner, InterfaceVersion, PlanFingerprint, PluginIdentity,
    RegistryFingerprint, RouteIdentity, RuntimeGeneration, RuntimeTargetIdentity, TargetReference,
    WorkerGeneration,
};
pub use invocation::{
    CanonicalInvocationResult, ExecutionAttempt, ExecutionAttemptId, ExecutionTargetPin,
    IdempotencyKey, IdempotencyKeyError, InterfaceAuthorizationError, InterfaceAuthorizationFuture,
    InterfaceAuthorizationPort, InterfaceAuthorizationRequest, InterfaceInvocationError,
    InterfaceInvocationFailure, InterfaceInvocationKernel, InterfaceInvocationOutcome,
    InterfaceInvocationReceipt, InterfaceInvocationResult, InterfaceInvocationStage,
    InterfaceInvocationTerminal, InterfaceProtocol, InterfaceServerStream, InterfaceStageRecord,
    InterfaceStreamAccumulator, InterfaceStreamStateError, InterfaceStreamTerminal,
    InterfaceTargetAdmissionError, InterfaceTargetAdmissionFuture, InterfaceTargetAdmissionPort,
    InterfaceTargetAdmissionRequest, InterfaceTargetFailure, InvocationCancellation,
    InvocationControls, InvocationEnvelope, InvocationId, InvocationLineage,
    InvocationLineageError, ResolvedInvocationPin,
};
pub use principal::{
    ApplicationPrincipal, ApplicationPrincipalError, InvocationPrincipal, PrincipalProfile,
    PrincipalSummary, PublicPrincipal, UserCredentialKind, UserPrincipal,
};
pub use registry::{
    CompiledInterfaceRegistry, CompiledInvocationPlan, DynamicInterfaceRegistry, InterfaceAccess,
    InterfaceAuditPolicy, InterfaceAuthenticationPolicy, InterfaceContract, InterfaceContracts,
    InterfaceDefinition, InterfaceErrorPolicy, InterfaceExecution, InterfaceExecutionMode,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceIdentity,
    InterfaceLifecycle, InterfaceResultContracts, InterfaceScope, InterfaceTargetError,
    InvocationAdapterPlan, ProtocolBinding, ProtocolProjection, RegistryCompilationError,
    RegistryCompiler,
};

#[cfg(test)]
mod _tests;
pub use extension::{
    compile_effective_handler, InterfaceExtensionCompilationError, InterfaceExtensionFact,
    InterfaceExtensionIsolation, InterfaceExtensionPermission, InterfaceExtensionPoint,
    InterfaceExtensionRegistration, InterfaceExtensionTier, InterfaceHandlerCandidate,
};
