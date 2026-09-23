use extension_contracts::provider_contract::ProviderTransportClosureEvidence;
use std::{error::Error, fmt, time::Duration};

const MAX_OPAQUE_ID_BYTES: usize = 256;

macro_rules! opaque_id {
    ($name:ident, $label:literal) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(IdentityError::Empty($label));
                }
                if value.len() > MAX_OPAQUE_ID_BYTES {
                    return Err(IdentityError::TooLong($label));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

opaque_id!(TransportSessionId, "transport_session_id");
opaque_id!(TransportOwnerId, "transport_owner_id");
opaque_id!(TransportProviderId, "transport_provider_id");
opaque_id!(TransportRuntimeTargetId, "transport_runtime_target_id");

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransportInstant(u64);

impl TransportInstant {
    pub const fn from_millis(millis: u64) -> Self {
        Self(millis)
    }

    pub const fn as_millis(self) -> u64 {
        self.0
    }

    pub(crate) fn saturating_add(self, duration: Duration) -> Self {
        let millis = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(millis))
    }

    pub(crate) fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}

pub type TransportDeadline = TransportInstant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransportGeneration(u64);

impl TransportGeneration {
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransportFence {
    pub session_id: TransportSessionId,
    pub generation: TransportGeneration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportFenceStatus {
    Current,
    Stale {
        current: TransportGeneration,
        received: TransportGeneration,
    },
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportSessionState {
    Opening,
    Active,
    WaitingTool,
    IdleAffinity,
    /// Affinity resource was released; the fixed logical lifetime remains.
    IdleReleased,
    Orphaned,
    Draining,
    Faulted,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadlineKind {
    Task,
    LogicalAbsolute,
    StateLease,
    PhysicalHard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationKind {
    DeadlineExceeded(DeadlineKind),
    CapacityEvicted,
    ProviderFault,
    ProviderHardMax,
    OwnerOrphaned,
    OwnerClosed,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminationReceipt {
    pub fence: TransportFence,
    pub owner_id: TransportOwnerId,
    pub provider_id: TransportProviderId,
    pub runtime_target_id: TransportRuntimeTargetId,
    pub previous_state: TransportSessionState,
    /// The invocation that still owned this generation at termination. A
    /// successor waits for this lease to finish or reach its own deadline.
    pub unsettled_invocation: Option<InvocationLease>,
    pub kind: TerminationKind,
    pub terminated_at: TransportInstant,
    pub connection_age: Duration,
    /// Result of the downstream close command. `None` means that the command is still pending.
    pub close_acknowledged: Option<bool>,
    pub closure_evidence: Option<ProviderTransportClosureEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    StateChanged {
        fence: TransportFence,
        from: TransportSessionState,
        to: TransportSessionState,
        at: TransportInstant,
    },
    Terminated(TerminationReceipt),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmissionRequest {
    pub session_id: TransportSessionId,
    pub owner_id: TransportOwnerId,
    /// Opaque Provider instance that owns the logical session.
    pub provider_id: TransportProviderId,
    /// Opaque Runtime Backend target used only for lifecycle control dispatch.
    pub runtime_target_id: TransportRuntimeTargetId,
    /// A Provider-advertised hard deadline, additionally capped by `physical_max_age`.
    pub provider_hard_deadline: Option<TransportDeadline>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvocationRequest {
    /// The current invocation's absolute deadline. When absent, the configured default is used.
    pub deadline: Option<TransportDeadline>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvocationLease {
    pub fence: TransportFence,
    sequence: u64,
    deadline: TransportDeadline,
}

impl InvocationLease {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn deadline(&self) -> TransportDeadline {
        self.deadline
    }

    pub(crate) const fn new(
        fence: TransportFence,
        sequence: u64,
        deadline: TransportDeadline,
    ) -> Self {
        Self {
            fence,
            sequence,
            deadline,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationCompletion {
    Active,
    WaitingTool,
    IdleAffinity,
    Orphaned,
    Faulted,
    Closing,
}

impl InvocationCompletion {
    pub(crate) const fn state(self) -> TransportSessionState {
        match self {
            Self::Active => TransportSessionState::Active,
            Self::WaitingTool => TransportSessionState::WaitingTool,
            Self::IdleAffinity => TransportSessionState::IdleAffinity,
            Self::Orphaned => TransportSessionState::Orphaned,
            Self::Faulted => TransportSessionState::Faulted,
            Self::Closing => TransportSessionState::Closing,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeSessionSnapshot {
    pub closure_evidence: Option<ProviderTransportClosureEvidence>,
    pub fence: TransportFence,
    pub owner_id: TransportOwnerId,
    pub provider_id: TransportProviderId,
    pub runtime_target_id: TransportRuntimeTargetId,
    pub state: TransportSessionState,
    pub inflight: bool,
    pub age: Duration,
    pub state_age: Duration,
    pub state_ttl: Duration,
    pub logical_ttl: Duration,
    pub invocation_deadline: Option<TransportDeadline>,
    pub invocation_ttl: Option<Duration>,
    pub physical_ttl: Duration,
    pub deadline_kind: DeadlineKind,
    pub eviction_priority: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SafeRegistrySnapshot {
    pub observed_at: TransportInstant,
    pub capacity: usize,
    pub tombstone_ttl: Duration,
    pub sessions: Vec<SafeSessionSnapshot>,
    pub tombstones: Vec<TerminationReceipt>,
}

#[derive(Clone, Debug)]
pub struct TransportRegistryConfig {
    pub capacity: usize,
    pub tombstone_capacity: usize,
    pub tombstone_ttl: Duration,
    pub event_capacity: usize,
    pub logical_max_age: Duration,
    pub invocation_default: Duration,
    pub waiting_tool_lease: Duration,
    pub orphan_grace: Duration,
    pub idle_affinity_lease: Duration,
    pub physical_soft_drain_age: Duration,
    pub physical_max_age: Duration,
    pub fault_grace: Duration,
    pub closing_grace: Duration,
}

impl Default for TransportRegistryConfig {
    fn default() -> Self {
        Self {
            capacity: 128,
            tombstone_capacity: 256,
            tombstone_ttl: Duration::from_secs(5 * 60),
            event_capacity: 512,
            logical_max_age: Duration::from_secs(2 * 60 * 60),
            invocation_default: Duration::from_secs(30 * 60),
            waiting_tool_lease: Duration::from_secs(55 * 60),
            orphan_grace: Duration::from_secs(60),
            idle_affinity_lease: Duration::from_secs(90),
            physical_soft_drain_age: Duration::from_secs(50 * 60),
            physical_max_age: Duration::from_secs(58 * 60),
            fault_grace: Duration::from_secs(60),
            closing_grace: Duration::from_secs(10),
        }
    }
}

impl TransportRegistryConfig {
    pub(crate) fn validate(&self) -> Result<(), RegistryError> {
        let durations = [
            self.logical_max_age,
            self.tombstone_ttl,
            self.invocation_default,
            self.waiting_tool_lease,
            self.orphan_grace,
            self.idle_affinity_lease,
            self.physical_soft_drain_age,
            self.physical_max_age,
            self.fault_grace,
            self.closing_grace,
        ];
        if self.capacity == 0 || self.tombstone_capacity == 0 || self.event_capacity == 0 {
            return Err(RegistryError::InvalidConfig);
        }
        if durations.iter().any(Duration::is_zero)
            || self.physical_soft_drain_age >= self.physical_max_age
        {
            return Err(RegistryError::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityError {
    Empty(&'static str),
    TooLong(&'static str),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(label) => write!(formatter, "{label} must not be empty"),
            Self::TooLong(label) => write!(formatter, "{label} exceeds the opaque identity limit"),
        }
    }
}

impl Error for IdentityError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapacityRejection {
    pub capacity: usize,
    pub active: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    InvalidConfig,
    InvalidClosureEvidence,
    DeadlineInPast,
    AlreadyExists,
    NotFound,
    StaleGeneration {
        expected: TransportGeneration,
        received: TransportGeneration,
    },
    InvalidTransition {
        from: TransportSessionState,
        to: TransportSessionState,
    },
    InflightExists,
    NoInflight,
    StaleInvocation,
    GenerationExhausted,
    InvocationSequenceExhausted,
    Capacity(CapacityRejection),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "transport session registry error: {self:?}")
    }
}

impl Error for RegistryError {}
