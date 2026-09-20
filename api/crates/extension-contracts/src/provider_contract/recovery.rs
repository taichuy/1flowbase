use super::*;

pub const PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY: &str = "provider_recovery";
pub const PROVIDER_RECOVERY_RECEIPT_METADATA_KEY: &str = "1flowbase_provider_recovery";
pub const MAX_RECOVERY_INNER_ATTEMPTS: u16 = 16;

/// AI Native-owned fencing identity. This is deliberately not a provider
/// socket generation or incarnation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TransportEpoch(u64);

impl TransportEpoch {
    pub fn new(value: u64) -> Result<Self, String> {
        (value > 0)
            .then_some(Self(value))
            .ok_or_else(|| "transport epoch must be positive".to_string())
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for TransportEpoch {
    type Error = String;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<TransportEpoch> for u64 {
    fn from(value: TransportEpoch) -> Self {
        value.get()
    }
}

/// Provider-local identity for one real socket. It must never be used as an
/// AI Native fencing epoch.
///
/// ```compile_fail
/// use extension_contracts::{SocketIncarnation, TransportEpoch};
///
/// let epoch = TransportEpoch::new(1).unwrap();
/// let _incarnation: SocketIncarnation = epoch;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct SocketIncarnation(u64);

impl SocketIncarnation {
    pub fn new(value: u64) -> Result<Self, String> {
        (value > 0)
            .then_some(Self(value))
            .ok_or_else(|| "socket incarnation must be positive".to_string())
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for SocketIncarnation {
    type Error = String;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SocketIncarnation> for u64 {
    fn from(value: SocketIncarnation) -> Self {
        value.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitLevel {
    /// Only lifecycle facts exist; no semantic result has been committed.
    LifecycleOnly,
    SemanticCommitted,
    Terminal,
}

/// Cursor provenance only. The cursor or provider turn state is intentionally
/// absent from this stable contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CursorBinding {
    Durable,
    ConnectionBound {
        transport_epoch: TransportEpoch,
        /// Provider-reported source socket incarnation that produced the cursor.
        socket_incarnation: SocketIncarnation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CursorProvenance {
    pub binding: CursorBinding,
}

impl CursorProvenance {
    pub const fn durable() -> Self {
        Self {
            binding: CursorBinding::Durable,
        }
    }

    pub const fn connection_bound(
        transport_epoch: TransportEpoch,
        socket_incarnation: SocketIncarnation,
    ) -> Self {
        Self {
            binding: CursorBinding::ConnectionBound {
                transport_epoch,
                socket_incarnation,
            },
        }
    }

    fn validate_epoch(self, transport_epoch: TransportEpoch) -> Result<(), String> {
        let CursorBinding::ConnectionBound {
            transport_epoch: bound_epoch,
            ..
        } = self.binding
        else {
            return Ok(());
        };

        if bound_epoch != transport_epoch {
            return Err("connection-bound cursor cannot cross a transport epoch".to_string());
        }
        Ok(())
    }

    fn validate_actual_receipt(self, receipt: &ProviderRecoveryReceipt) -> Result<(), String> {
        let CursorBinding::ConnectionBound {
            socket_incarnation: bound_incarnation,
            ..
        } = self.binding
        else {
            return Ok(());
        };

        match receipt.disposition {
            RecoveryDisposition::SameEpochReconnect
                if receipt
                    .socket_incarnation
                    .map_or(true, |actual_incarnation| {
                        actual_incarnation <= bound_incarnation
                    }) =>
            {
                Err(
                    "same-epoch reconnect must advance its connection-bound socket incarnation"
                        .to_string(),
                )
            }
            RecoveryDisposition::PreCommitHttpFallback => {
                Err("connection-bound cursor cannot use provider HTTP fallback".to_string())
            }
            _ => Ok(()),
        }
    }
}

/// Parse the typed recovery receipt a Provider may attach to an error or to a
/// successful result.
///
/// `container` is a free-form `provider_details` / `provider_metadata` object.
/// The single owner of the key lives here so the error path and the output path
/// cannot drift apart. A receipt that is present but malformed is an error
/// rather than `None`: a corrupted receipt must never be silently downgraded to
/// "no receipt", which would authorize an unbounded outer attempt.
pub fn recovery_receipt_from_details(
    container: &serde_json::Value,
) -> Result<Option<ProviderRecoveryReceipt>, String> {
    let Some(value) = container
        .as_object()
        .and_then(|object| object.get(PROVIDER_RECOVERY_RECEIPT_METADATA_KEY))
    else {
        return Ok(None);
    };
    let receipt: ProviderRecoveryReceipt = serde_json::from_value(value.clone())
        .map_err(|_| "provider recovery receipt is invalid".to_string())?;
    receipt.validate()?;
    Ok(Some(receipt))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryDisposition {
    SameEpochReconnect,
    PreCommitHttpFallback,
    OneFullContextRebuild,
    LogicalInvocationRetry,
    TerminalInterruption,
    SemanticTerminal,
}

impl RecoveryDisposition {
    pub const fn replays_semantic_input(self) -> bool {
        matches!(
            self,
            Self::PreCommitHttpFallback
                | Self::OneFullContextRebuild
                | Self::LogicalInvocationRetry
        )
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::TerminalInterruption | Self::SemanticTerminal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryBudget {
    /// Total authorized upstream attempts, including the initial attempt.
    /// One attempt starts before connection acquisition and includes its send/read;
    /// connection, send and read failures each consume that same attempt once.
    /// A reconnect or HTTP fallback starts another attempt; backoff consumes none.
    pub max_inner_attempts: u16,
    pub absolute_deadline_unix_ms: i64,
}

impl RecoveryBudget {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_inner_attempts == 0 || self.max_inner_attempts > MAX_RECOVERY_INNER_ATTEMPTS {
            return Err(format!(
                "recovery max_inner_attempts must contain 1 through {MAX_RECOVERY_INNER_ATTEMPTS} attempts"
            ));
        }
        if self.absolute_deadline_unix_ms <= 0 {
            return Err("recovery absolute_deadline_unix_ms must be positive".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryPolicy {
    SemanticMapped { budget: RecoveryBudget },
    NativeOpaque { budget: RecoveryBudget },
}

impl RecoveryPolicy {
    pub fn validate(&self) -> Result<(), String> {
        self.budget().validate()
    }

    fn validate_disposition(&self, disposition: RecoveryDisposition) -> Result<(), String> {
        self.validate()?;
        if matches!(self, Self::NativeOpaque { .. })
            && disposition == RecoveryDisposition::PreCommitHttpFallback
        {
            return Err("native-opaque recovery forbids provider HTTP fallback".to_string());
        }
        Ok(())
    }

    pub const fn budget(&self) -> &RecoveryBudget {
        match self {
            Self::SemanticMapped { budget } | Self::NativeOpaque { budget } => budget,
        }
    }
}

/// Host-owned recovery constraints and initial facts. The Provider chooses and
/// reports the actual transition in `ProviderRecoveryReceipt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRecoveryDirective {
    pub policy: RecoveryPolicy,
    pub transport_epoch: TransportEpoch,
    pub initial_commit_level: CommitLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_provenance: Option<CursorProvenance>,
}

impl ProviderRecoveryDirective {
    pub fn validate(&self) -> Result<(), String> {
        self.policy.validate()?;
        if let Some(provenance) = self.cursor_provenance {
            provenance.validate_epoch(self.transport_epoch)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryTransport {
    AiNativeWebSocket,
    ProviderHttp,
}

/// Closed reason taxonomy: recovery receipts cannot carry upstream errors,
/// prompts, tool output, cursor values, response ids, or other payload text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    TransportDisconnected,
    TransportRejected,
    ProtocolError,
    DeadlineExceeded,
    FencingRejected,
    BudgetExhausted,
    SemanticCompleted,
    SemanticFailed,
}

/// Provider-owned result for one recovery transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRecoveryReceipt {
    /// Zero-based index of the last actual upstream attempt (not retry count).
    /// Successful recovery must include the successful attempt in this index.
    pub attempt: u16,
    pub transport: RecoveryTransport,
    pub transport_epoch: TransportEpoch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socket_incarnation: Option<SocketIncarnation>,
    pub commit_level: CommitLevel,
    pub disposition: RecoveryDisposition,
    pub reason: RecoveryReason,
}

impl ProviderRecoveryReceipt {
    /// Actual consumed attempts represented by a valid receipt. The wire keeps
    /// its published zero-based index; accounting must use this count instead.
    pub fn consumed_attempts(&self) -> Result<u16, String> {
        self.validate()?;
        Ok(self.attempt + 1)
    }

    pub const fn is_pre_commit_http_fallback(&self) -> bool {
        matches!(
            (self.transport, self.disposition),
            (
                RecoveryTransport::ProviderHttp,
                RecoveryDisposition::PreCommitHttpFallback
            )
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_commit_disposition(self.commit_level, self.disposition)?;
        if self.attempt >= MAX_RECOVERY_INNER_ATTEMPTS {
            return Err(format!(
                "zero-based recovery receipt attempt must be below {MAX_RECOVERY_INNER_ATTEMPTS}"
            ));
        }
        match (self.transport, self.socket_incarnation) {
            // A terminal receipt claims no reconnect and no resumption, so a
            // failure observed before any socket existed may report the real
            // outcome without inventing an incarnation. Every non-terminal
            // WebSocket receipt still has to name the socket it intends to use.
            (RecoveryTransport::AiNativeWebSocket, None) if !self.disposition.is_terminal() => Err(
                "AI Native WebSocket recovery receipt requires a socket incarnation".to_string(),
            ),
            (RecoveryTransport::ProviderHttp, Some(_)) => {
                Err("provider HTTP recovery receipt cannot claim a socket incarnation".to_string())
            }
            _ => Ok(()),
        }?;
        match self.disposition {
            RecoveryDisposition::SameEpochReconnect
                if self.transport != RecoveryTransport::AiNativeWebSocket =>
            {
                Err("same-epoch reconnect requires AI Native WebSocket transport".to_string())
            }
            RecoveryDisposition::PreCommitHttpFallback
                if self.transport != RecoveryTransport::ProviderHttp =>
            {
                Err("pre-commit HTTP fallback requires provider HTTP transport".to_string())
            }
            _ => Ok(()),
        }
    }

    pub fn validate_against(&self, directive: &ProviderRecoveryDirective) -> Result<(), String> {
        self.validate()?;
        directive.validate()?;
        if self.transport_epoch != directive.transport_epoch {
            return Err("recovery receipt cannot cross its directive transport epoch".to_string());
        }
        if !directive
            .initial_commit_level
            .can_advance_to(self.commit_level)
        {
            return Err("recovery receipt commit level cannot move backwards".to_string());
        }
        if self.attempt >= directive.policy.budget().max_inner_attempts {
            return Err("recovery receipt attempt exceeds its directive budget".to_string());
        }
        directive.policy.validate_disposition(self.disposition)?;
        if let Some(provenance) = directive.cursor_provenance {
            provenance.validate_actual_receipt(self)?;
        }
        Ok(())
    }
}

impl CommitLevel {
    const fn can_advance_to(self, final_level: Self) -> bool {
        matches!(
            (self, final_level),
            (Self::LifecycleOnly, _)
                | (
                    Self::SemanticCommitted,
                    Self::SemanticCommitted | Self::Terminal
                )
                | (Self::Terminal, Self::Terminal)
        )
    }
}

fn validate_commit_disposition(
    commit_level: CommitLevel,
    disposition: RecoveryDisposition,
) -> Result<(), String> {
    if commit_level != CommitLevel::LifecycleOnly && disposition.replays_semantic_input() {
        return Err("committed recovery state cannot select a replay disposition".to_string());
    }
    if (commit_level == CommitLevel::Terminal) != disposition.is_terminal() {
        return Err(
            "terminal commit level and terminal recovery disposition must agree".to_string(),
        );
    }
    Ok(())
}
