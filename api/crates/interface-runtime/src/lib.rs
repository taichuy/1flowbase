//! Protocol-independent active interface definitions and invocation contracts.

mod identity;
mod invocation;
mod registry;

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
    CompiledInterfaceRegistry, DynamicInterfaceRegistry, InterfaceContract, InterfaceDefinition,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceTargetError,
    RegistryCompilationError, RegistryCompiler,
};

#[cfg(test)]
mod _tests;
