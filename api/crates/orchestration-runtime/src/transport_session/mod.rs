//! Logical provider transport-session lifecycle.
//!
//! This module deliberately stores only opaque identities and lifecycle metadata. Provider
//! credentials, prompts, tool output, cursors, and physical socket handles belong to other
//! boundaries and cannot be inserted into this registry.

mod registry;
mod types;

pub use registry::{SystemTransportClock, TransportClock, TransportSessionRegistry};
pub use types::{
    AdmissionRequest, CapacityRejection, DeadlineKind, IdentityError, InvocationCompletion,
    InvocationLease, InvocationRequest, LifecycleEvent, RegistryError, SafeRegistrySnapshot,
    SafeSessionSnapshot, TerminationKind, TerminationReceipt, TransportDeadline, TransportFence,
    TransportGeneration, TransportInstant, TransportOwnerId, TransportProviderId,
    TransportRegistryConfig, TransportRuntimeTargetId, TransportSessionId, TransportSessionState,
};

#[cfg(test)]
mod _tests;
