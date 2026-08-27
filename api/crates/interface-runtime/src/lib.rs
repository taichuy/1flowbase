//! Protocol-independent active interface definitions and invocation contracts.

mod identity;
mod registry;

pub use identity::{
    ContractIdentity, GraphFingerprint, HandlerReference, IdentityError, InterfaceId,
    InterfaceOwner, PermissionIdentity, RegistryFingerprint, RouteIdentity, TargetReference,
};
pub use registry::{
    CompiledInterfaceRegistry, DynamicInterfaceRegistry, InterfaceContract, InterfaceDefinition,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceTargetError,
    RegistryCompilationError, RegistryCompiler,
};

#[cfg(test)]
mod _tests;
