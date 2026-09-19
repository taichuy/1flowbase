use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

use extension_contracts::provider_contract::{
    CommitLevel, ProviderRecoveryDirective, ProviderRecoveryReceipt, ProviderStreamEvent,
    RecoveryBudget, RecoveryDisposition, RecoveryPolicy, TransportEpoch,
};
use serde::Serialize;

const DEFAULT_BUCKET_CAPACITY: u16 = 4;
const DEFAULT_BUCKET_REFILL_TOKENS: u16 = 1;
const DEFAULT_BUCKET_REFILL_INTERVAL_MS: u64 = 30_000;
const DEFAULT_RETRY_COST: u16 = 1;
const DEFAULT_PROVIDER_INNER_ATTEMPTS: u16 = 2;

static NEXT_TRANSPORT_EPOCH: AtomicU64 = AtomicU64::new(1);
static DEFAULT_RETRY_BUCKET: OnceLock<Mutex<PartitionedRetryTokenBucket>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderRetryPartition {
    provider_instance_id: String,
    endpoint_identity: String,
    credential_owner_hash: String,
}

impl ProviderRetryPartition {
    pub fn new(
        provider_instance_id: impl Into<String>,
        endpoint_identity: impl Into<String>,
        credential_owner_hash: impl Into<String>,
    ) -> Result<Self, &'static str> {
        let partition = Self {
            provider_instance_id: provider_instance_id.into(),
            endpoint_identity: endpoint_identity.into(),
            credential_owner_hash: credential_owner_hash.into(),
        };
        if partition.provider_instance_id.is_empty()
            || partition.endpoint_identity.is_empty()
            || partition.credential_owner_hash.is_empty()
        {
            return Err("provider retry partition components must not be empty");
        }
        if [
            &partition.provider_instance_id,
            &partition.endpoint_identity,
            &partition.credential_owner_hash,
        ]
        .into_iter()
        .any(|value| value.len() > 256)
        {
            return Err("provider retry partition component exceeds the opaque identity limit");
        }
        Ok(partition)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryTokenBucketConfig {
    pub capacity: u16,
    pub refill_tokens: u16,
    pub refill_interval_ms: u64,
    pub retry_cost: u16,
}

impl Default for RetryTokenBucketConfig {
    fn default() -> Self {
        Self {
            capacity: DEFAULT_BUCKET_CAPACITY,
            refill_tokens: DEFAULT_BUCKET_REFILL_TOKENS,
            refill_interval_ms: DEFAULT_BUCKET_REFILL_INTERVAL_MS,
            retry_cost: DEFAULT_RETRY_COST,
        }
    }
}

impl RetryTokenBucketConfig {
    fn validate(self) -> Result<Self, &'static str> {
        if self.capacity == 0
            || self.refill_tokens == 0
            || self.refill_tokens > self.capacity
            || self.refill_interval_ms == 0
            || self.retry_cost == 0
            || self.retry_cost > self.capacity
        {
            return Err("provider retry token bucket bounds are invalid");
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug)]
struct RetryTokenState {
    tokens: u16,
    last_refill_ms: u64,
}

/// Process-local protection for logical replay storms. It deliberately stores only
/// opaque partition identities; credentials and Provider payloads cannot enter it.
pub struct PartitionedRetryTokenBucket {
    config: RetryTokenBucketConfig,
    partitions: BTreeMap<ProviderRetryPartition, RetryTokenState>,
}

impl PartitionedRetryTokenBucket {
    pub fn new(config: RetryTokenBucketConfig) -> Result<Self, &'static str> {
        Ok(Self {
            config: config.validate()?,
            partitions: BTreeMap::new(),
        })
    }

    pub fn try_consume(&mut self, partition: ProviderRetryPartition, now_ms: u64) -> bool {
        let state = self.partitions.entry(partition).or_insert(RetryTokenState {
            tokens: self.config.capacity,
            last_refill_ms: now_ms,
        });
        let elapsed = now_ms.saturating_sub(state.last_refill_ms);
        let intervals = elapsed / self.config.refill_interval_ms;
        if intervals > 0 {
            let replenished = u64::from(self.config.refill_tokens).saturating_mul(intervals);
            state.tokens = u16::try_from(
                u64::from(state.tokens)
                    .saturating_add(replenished)
                    .min(u64::from(self.config.capacity)),
            )
            .expect("bounded token count must fit u16");
            state.last_refill_ms = state
                .last_refill_ms
                .saturating_add(intervals.saturating_mul(self.config.refill_interval_ms));
        }
        if state.tokens < self.config.retry_cost {
            return false;
        }
        state.tokens -= self.config.retry_cost;
        true
    }
}

impl Default for PartitionedRetryTokenBucket {
    fn default() -> Self {
        Self::new(RetryTokenBucketConfig::default())
            .expect("internal provider retry token bucket defaults must be valid")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OuterReplayDecision {
    Retry,
    MissingTypedReceipt,
    StaleEpochNoop,
    ProviderDidNotRequestLogicalRetry,
    SemanticCommitted,
    SemanticTerminal,
    NonReproducible,
    AttemptBudgetExhausted,
    DeadlineExceeded,
    PartitionTokenExhausted,
    InvalidReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AiNativeRecoveryReceipt {
    pub outer_attempt: u16,
    pub transport_epoch: TransportEpoch,
    pub provider_directive: ProviderRecoveryDirective,
    pub provider_inner_receipt: Option<ProviderRecoveryReceipt>,
    pub provider_inner_attempt: Option<u16>,
    pub provider_final_commit: Option<CommitLevel>,
    pub provider_disposition: Option<RecoveryDisposition>,
    pub decision: OuterReplayDecision,
    pub original_terminal_preserved: bool,
}

/// Invocation-local semantic barrier and attempt ledger. The absolute deadline is
/// captured once and is never recomputed when a new outer attempt starts.
pub struct AiNativeRecoveryLedger {
    absolute_deadline_unix_ms: i64,
    outer_retry_budget: u16,
    outer_retries_used: u16,
    current_epoch: TransportEpoch,
    semantic_committed: bool,
    semantic_terminal: bool,
    reproducible: bool,
    input_mode: RecoveryInputMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryInputMode {
    SemanticMapped,
    NativeOpaque,
}

impl AiNativeRecoveryLedger {
    pub fn new(
        absolute_deadline_unix_ms: i64,
        outer_retry_budget: u16,
        reproducible: bool,
        input_mode: RecoveryInputMode,
    ) -> Result<Self, &'static str> {
        if absolute_deadline_unix_ms <= 0 {
            return Err("AI Native recovery deadline must be positive");
        }
        Ok(Self {
            absolute_deadline_unix_ms,
            outer_retry_budget,
            outer_retries_used: 0,
            current_epoch: allocate_transport_epoch()?,
            semantic_committed: false,
            semantic_terminal: false,
            reproducible,
            input_mode,
        })
    }

    pub const fn absolute_deadline_unix_ms(&self) -> i64 {
        self.absolute_deadline_unix_ms
    }

    pub const fn current_epoch(&self) -> TransportEpoch {
        self.current_epoch
    }

    pub const fn semantic_replay_blocked(&self) -> bool {
        self.semantic_committed || self.semantic_terminal
    }

    pub fn rotate_epoch_for_configured_retry(&mut self) -> Result<(), &'static str> {
        self.current_epoch = allocate_transport_epoch()?;
        Ok(())
    }

    pub fn provider_directive(&self) -> ProviderRecoveryDirective {
        let budget = RecoveryBudget {
            max_inner_attempts: DEFAULT_PROVIDER_INNER_ATTEMPTS,
            absolute_deadline_unix_ms: self.absolute_deadline_unix_ms,
        };
        let policy = match self.input_mode {
            RecoveryInputMode::SemanticMapped => RecoveryPolicy::SemanticMapped { budget },
            RecoveryInputMode::NativeOpaque => RecoveryPolicy::NativeOpaque { budget },
        };
        ProviderRecoveryDirective {
            policy,
            transport_epoch: self.current_epoch,
            // A directive is emitted before its Provider attempt. Once this ledger observes a
            // semantic commit/terminal it refuses another outer attempt instead of issuing a
            // directive that could replay committed input.
            initial_commit_level: CommitLevel::LifecycleOnly,
            cursor_provenance: None,
        }
    }

    pub fn observe_events(&mut self, events: &[ProviderStreamEvent]) {
        for event in events {
            match event {
                ProviderStreamEvent::NativeEvent { .. }
                | ProviderStreamEvent::UsageDelta { .. }
                | ProviderStreamEvent::UsageSnapshot { .. }
                | ProviderStreamEvent::Finish { .. }
                | ProviderStreamEvent::Error { .. } => {}
                ProviderStreamEvent::OutputProtocolFailure { .. } => {
                    self.semantic_terminal = true;
                }
                ProviderStreamEvent::TextDelta { .. }
                | ProviderStreamEvent::ReasoningDelta { .. }
                | ProviderStreamEvent::ReasoningSignatureDelta { .. }
                | ProviderStreamEvent::ToolCallDelta { .. }
                | ProviderStreamEvent::ToolCallCommit { .. }
                | ProviderStreamEvent::McpCallDelta { .. }
                | ProviderStreamEvent::McpCallCommit { .. }
                | ProviderStreamEvent::ResponsesOutputDelta { .. }
                | ProviderStreamEvent::OutputItem { .. } => {
                    self.semantic_committed = true;
                }
            }
        }
    }

    pub fn decide_outer_replay(
        &mut self,
        receipt: Option<&ProviderRecoveryReceipt>,
        partition: ProviderRetryPartition,
        now_unix_ms: i64,
        token_bucket: &mut PartitionedRetryTokenBucket,
        outer_attempt: u16,
    ) -> AiNativeRecoveryReceipt {
        let decision = self.replay_decision(receipt, partition, now_unix_ms, token_bucket);
        let provider_directive = self.provider_directive();
        let result = AiNativeRecoveryReceipt {
            outer_attempt,
            transport_epoch: self.current_epoch,
            provider_directive,
            provider_inner_receipt: receipt.cloned(),
            provider_inner_attempt: receipt.map(|receipt| receipt.attempt),
            provider_final_commit: receipt.map(|receipt| receipt.commit_level),
            provider_disposition: receipt.map(|receipt| receipt.disposition),
            decision,
            original_terminal_preserved: decision != OuterReplayDecision::Retry,
        };
        if decision == OuterReplayDecision::Retry {
            self.outer_retries_used = self.outer_retries_used.saturating_add(1);
            if let Ok(epoch) = allocate_transport_epoch() {
                self.current_epoch = epoch;
            }
        }
        result
    }

    fn replay_decision(
        &self,
        receipt: Option<&ProviderRecoveryReceipt>,
        partition: ProviderRetryPartition,
        now_unix_ms: i64,
        token_bucket: &mut PartitionedRetryTokenBucket,
    ) -> OuterReplayDecision {
        let Some(receipt) = receipt else {
            return OuterReplayDecision::MissingTypedReceipt;
        };
        if receipt.transport_epoch != self.current_epoch {
            return OuterReplayDecision::StaleEpochNoop;
        }
        if receipt
            .validate_against(&self.provider_directive())
            .is_err()
        {
            return OuterReplayDecision::InvalidReceipt;
        }
        if self.semantic_terminal || receipt.commit_level == CommitLevel::Terminal {
            return OuterReplayDecision::SemanticTerminal;
        }
        if self.semantic_committed || receipt.commit_level != CommitLevel::LifecycleOnly {
            return OuterReplayDecision::SemanticCommitted;
        }
        if receipt.disposition != RecoveryDisposition::LogicalInvocationRetry {
            return OuterReplayDecision::ProviderDidNotRequestLogicalRetry;
        }
        if !self.reproducible {
            return OuterReplayDecision::NonReproducible;
        }
        if self.outer_retries_used >= self.outer_retry_budget {
            return OuterReplayDecision::AttemptBudgetExhausted;
        }
        if now_unix_ms >= self.absolute_deadline_unix_ms {
            return OuterReplayDecision::DeadlineExceeded;
        }
        let now_ms = u64::try_from(now_unix_ms).unwrap_or(0);
        if !token_bucket.try_consume(partition, now_ms) {
            return OuterReplayDecision::PartitionTokenExhausted;
        }
        OuterReplayDecision::Retry
    }
}

pub fn decide_outer_replay_with_default_bucket(
    ledger: &mut AiNativeRecoveryLedger,
    receipt: Option<&ProviderRecoveryReceipt>,
    partition: ProviderRetryPartition,
    now_unix_ms: i64,
    outer_attempt: u16,
) -> AiNativeRecoveryReceipt {
    let bucket =
        DEFAULT_RETRY_BUCKET.get_or_init(|| Mutex::new(PartitionedRetryTokenBucket::default()));
    let Ok(mut bucket) = bucket.lock() else {
        return AiNativeRecoveryReceipt {
            outer_attempt,
            transport_epoch: ledger.current_epoch(),
            provider_directive: ledger.provider_directive(),
            provider_inner_receipt: receipt.cloned(),
            provider_inner_attempt: receipt.map(|receipt| receipt.attempt),
            provider_final_commit: receipt.map(|receipt| receipt.commit_level),
            provider_disposition: receipt.map(|receipt| receipt.disposition),
            decision: OuterReplayDecision::PartitionTokenExhausted,
            original_terminal_preserved: true,
        };
    };
    ledger.decide_outer_replay(receipt, partition, now_unix_ms, &mut bucket, outer_attempt)
}

fn allocate_transport_epoch() -> Result<TransportEpoch, &'static str> {
    let value = NEXT_TRANSPORT_EPOCH.fetch_add(1, Ordering::Relaxed);
    if value == 0 || value == u64::MAX {
        return Err("AI Native transport epoch space is exhausted");
    }
    TransportEpoch::new(value).map_err(|_| "AI Native transport epoch must be positive")
}

#[cfg(test)]
mod tests {
    use super::*;
    use extension_contracts::provider_contract::{
        RecoveryReason, RecoveryTransport, SocketIncarnation,
    };
    use rand::{rngs::StdRng, RngExt, SeedableRng};
    use serde_json::json;

    fn partition(name: &str) -> ProviderRetryPartition {
        ProviderRetryPartition::new(name, "endpoint", "credential-owner-hash").unwrap()
    }

    fn logical_retry(epoch: TransportEpoch) -> ProviderRecoveryReceipt {
        ProviderRecoveryReceipt {
            attempt: 1,
            transport: RecoveryTransport::AiNativeWebSocket,
            transport_epoch: epoch,
            socket_incarnation: Some(SocketIncarnation::new(7).unwrap()),
            commit_level: CommitLevel::LifecycleOnly,
            disposition: RecoveryDisposition::LogicalInvocationRetry,
            reason: RecoveryReason::TransportDisconnected,
        }
    }

    #[test]
    fn lifecycle_events_do_not_commit_but_first_semantic_event_does() {
        let mut ledger =
            AiNativeRecoveryLedger::new(10_000, 1, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        ledger.observe_events(&[
            ProviderStreamEvent::NativeEvent {
                protocol: "opaque".to_string(),
                event: json!({"lifecycle": "connected"}),
            },
            ProviderStreamEvent::UsageSnapshot {
                usage: Default::default(),
            },
        ]);
        let receipt = logical_retry(ledger.current_epoch());
        let mut bucket = PartitionedRetryTokenBucket::default();
        assert_eq!(
            ledger
                .decide_outer_replay(Some(&receipt), partition("a"), 1, &mut bucket, 0)
                .decision,
            OuterReplayDecision::Retry
        );

        let mut ledger =
            AiNativeRecoveryLedger::new(10_000, 1, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        ledger.observe_events(&[ProviderStreamEvent::ReasoningSignatureDelta {
            signature: "opaque".to_string(),
        }]);
        let receipt = logical_retry(ledger.current_epoch());
        assert_eq!(
            ledger
                .decide_outer_replay(Some(&receipt), partition("b"), 1, &mut bucket, 0)
                .decision,
            OuterReplayDecision::SemanticCommitted
        );
    }

    #[test]
    fn stale_epoch_is_a_noop_and_does_not_consume_budget() {
        let mut ledger =
            AiNativeRecoveryLedger::new(10_000, 1, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        let stale_epoch = allocate_transport_epoch().unwrap();
        let stale = logical_retry(stale_epoch);
        let current = logical_retry(ledger.current_epoch());
        let mut bucket = PartitionedRetryTokenBucket::default();
        assert_eq!(
            ledger
                .decide_outer_replay(Some(&stale), partition("a"), 1, &mut bucket, 0)
                .decision,
            OuterReplayDecision::StaleEpochNoop
        );
        assert_eq!(
            ledger
                .decide_outer_replay(Some(&current), partition("a"), 1, &mut bucket, 0)
                .decision,
            OuterReplayDecision::Retry
        );
    }

    #[test]
    fn outer_budget_and_deadline_are_not_reset_across_attempts() {
        let mut ledger =
            AiNativeRecoveryLedger::new(100, 1, true, RecoveryInputMode::SemanticMapped).unwrap();
        let first = logical_retry(ledger.current_epoch());
        let mut bucket = PartitionedRetryTokenBucket::default();
        assert_eq!(
            ledger
                .decide_outer_replay(Some(&first), partition("a"), 99, &mut bucket, 0)
                .decision,
            OuterReplayDecision::Retry
        );
        let second = logical_retry(ledger.current_epoch());
        assert_eq!(
            ledger
                .decide_outer_replay(Some(&second), partition("a"), 99, &mut bucket, 1)
                .decision,
            OuterReplayDecision::AttemptBudgetExhausted
        );

        let mut expired =
            AiNativeRecoveryLedger::new(100, 2, true, RecoveryInputMode::SemanticMapped).unwrap();
        let receipt = logical_retry(expired.current_epoch());
        assert_eq!(
            expired
                .decide_outer_replay(Some(&receipt), partition("b"), 100, &mut bucket, 0)
                .decision,
            OuterReplayDecision::DeadlineExceeded
        );
        assert_eq!(expired.absolute_deadline_unix_ms(), 100);
    }

    #[test]
    fn token_bucket_is_partitioned_bounded_and_uses_deterministic_time() {
        let config = RetryTokenBucketConfig {
            capacity: 2,
            refill_tokens: 1,
            refill_interval_ms: 10,
            retry_cost: 1,
        };
        let mut bucket = PartitionedRetryTokenBucket::new(config).unwrap();
        assert!(bucket.try_consume(partition("a"), 0));
        assert!(bucket.try_consume(partition("a"), 0));
        assert!(!bucket.try_consume(partition("a"), 0));
        assert!(bucket.try_consume(partition("b"), 0));
        assert!(!bucket.try_consume(partition("a"), 9));
        assert!(bucket.try_consume(partition("a"), 10));
        assert!(!bucket.try_consume(partition("a"), 10));
    }

    #[test]
    fn rejection_preserves_original_terminal_receipt() {
        let mut ledger =
            AiNativeRecoveryLedger::new(10_000, 0, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        let receipt = logical_retry(ledger.current_epoch());
        let mut bucket = PartitionedRetryTokenBucket::default();
        let result = ledger.decide_outer_replay(Some(&receipt), partition("a"), 1, &mut bucket, 0);
        assert_eq!(result.decision, OuterReplayDecision::AttemptBudgetExhausted);
        assert!(result.original_terminal_preserved);
        assert_eq!(result.provider_inner_attempt, Some(1));
    }

    #[test]
    fn seeded_event_sequences_never_replay_after_semantic_commit_or_terminal() {
        let mut rng = StdRng::seed_from_u64(0x0020_72D3);
        for sequence in 0..256_u16 {
            let mut ledger =
                AiNativeRecoveryLedger::new(10_000, 1, true, RecoveryInputMode::SemanticMapped)
                    .unwrap();
            let mut events = Vec::new();
            let mut semantic = false;
            let mut terminal = false;
            for _ in 0..rng.random_range(1..=12) {
                match rng.random_range(0..5) {
                    0 => events.push(ProviderStreamEvent::NativeEvent {
                        protocol: "opaque".to_string(),
                        event: json!({"lifecycle": true}),
                    }),
                    1 => events.push(ProviderStreamEvent::UsageDelta {
                        usage: Default::default(),
                    }),
                    2 => {
                        semantic = true;
                        events.push(ProviderStreamEvent::TextDelta {
                            delta: "x".to_string(),
                        });
                    }
                    3 => {
                        semantic = true;
                        events.push(ProviderStreamEvent::ToolCallDelta {
                            call_id: "call".to_string(),
                            delta: json!({}),
                        });
                    }
                    _ => {
                        terminal = true;
                        events.push(ProviderStreamEvent::OutputProtocolFailure {
                            failure: extension_contracts::provider_contract::ProviderOutputProtocolFailure {
                                protocol: "canonical".to_string(),
                                error_code: "semantic_terminal".to_string(),
                                message: "semantic terminal".to_string(),
                                retry_feedback: "none".to_string(),
                                provider_details: json!({}),
                            },
                        });
                    }
                }
            }
            ledger.observe_events(&events);
            let receipt = logical_retry(ledger.current_epoch());
            let mut bucket = PartitionedRetryTokenBucket::default();
            let decision = ledger
                .decide_outer_replay(
                    Some(&receipt),
                    partition(&format!("sequence-{sequence}")),
                    1,
                    &mut bucket,
                    0,
                )
                .decision;
            if terminal {
                assert_eq!(decision, OuterReplayDecision::SemanticTerminal);
            } else if semantic {
                assert_eq!(decision, OuterReplayDecision::SemanticCommitted);
            } else {
                assert_eq!(decision, OuterReplayDecision::Retry);
            }
        }
    }

    #[test]
    fn multi_provider_restart_and_out_of_order_receipts_remain_epoch_fenced() {
        let mut provider_a =
            AiNativeRecoveryLedger::new(10_000, 1, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        let mut provider_b =
            AiNativeRecoveryLedger::new(10_000, 1, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        let a_epoch_before_retry = provider_a.current_epoch();
        let b_epoch = provider_b.current_epoch();
        assert_ne!(a_epoch_before_retry, b_epoch);

        let mut bucket = PartitionedRetryTokenBucket::default();
        let worker_restart_receipt = logical_retry(b_epoch);
        let a_epoch_before_stale = provider_a.current_epoch();
        assert_eq!(
            provider_a
                .decide_outer_replay(
                    Some(&worker_restart_receipt),
                    partition("provider-a"),
                    1,
                    &mut bucket,
                    0,
                )
                .decision,
            OuterReplayDecision::StaleEpochNoop
        );
        assert_eq!(provider_a.current_epoch(), a_epoch_before_stale);

        let first_a = logical_retry(a_epoch_before_retry);
        assert_eq!(
            provider_a
                .decide_outer_replay(Some(&first_a), partition("provider-a"), 1, &mut bucket, 0,)
                .decision,
            OuterReplayDecision::Retry
        );
        let a_epoch_after_retry = provider_a.current_epoch();
        assert_ne!(a_epoch_after_retry, a_epoch_before_retry);

        // A delayed receipt from the prior socket arrives after the retry. It must not
        // mutate the current epoch or consume the remaining provider-local state.
        assert_eq!(
            provider_a
                .decide_outer_replay(Some(&first_a), partition("provider-a"), 2, &mut bucket, 1,)
                .decision,
            OuterReplayDecision::StaleEpochNoop
        );
        assert_eq!(provider_a.current_epoch(), a_epoch_after_retry);

        // Provider B has a distinct partition and remains independently eligible.
        let first_b = logical_retry(b_epoch);
        assert_eq!(
            provider_b
                .decide_outer_replay(Some(&first_b), partition("provider-b"), 2, &mut bucket, 0,)
                .decision,
            OuterReplayDecision::Retry
        );
    }

    #[test]
    fn provider_inner_attempts_precede_outer_attempt_and_do_not_multiply_budget() {
        let mut ledger =
            AiNativeRecoveryLedger::new(10_000, 1, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        let mut provider_finished_inner_budget = logical_retry(ledger.current_epoch());
        provider_finished_inner_budget.attempt = 1;
        let mut bucket = PartitionedRetryTokenBucket::default();

        let first = ledger.decide_outer_replay(
            Some(&provider_finished_inner_budget),
            partition("provider-a"),
            1,
            &mut bucket,
            0,
        );
        assert_eq!(first.provider_inner_attempt, Some(1));
        assert_eq!(first.outer_attempt, 0);
        assert_eq!(first.decision, OuterReplayDecision::Retry);

        let mut second_inner_budget = logical_retry(ledger.current_epoch());
        second_inner_budget.attempt = 1;
        let second = ledger.decide_outer_replay(
            Some(&second_inner_budget),
            partition("provider-a"),
            2,
            &mut bucket,
            1,
        );
        assert_eq!(second.provider_inner_attempt, Some(1));
        assert_eq!(second.outer_attempt, 1);
        assert_eq!(second.decision, OuterReplayDecision::AttemptBudgetExhausted);
        assert!(second.original_terminal_preserved);
    }

    #[test]
    fn configured_retry_can_rotate_after_semantic_output_but_automatic_replay_stays_blocked() {
        let mut committed =
            AiNativeRecoveryLedger::new(10_000, 2, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        committed.observe_events(&[ProviderStreamEvent::TextDelta {
            delta: "visible".to_string(),
        }]);
        let epoch_before_configured_retry = committed.current_epoch();
        committed.rotate_epoch_for_configured_retry().unwrap();
        assert_ne!(committed.current_epoch(), epoch_before_configured_retry);
        let committed_receipt = logical_retry(committed.current_epoch());
        let mut bucket = PartitionedRetryTokenBucket::default();
        assert_eq!(
            committed
                .decide_outer_replay(
                    Some(&committed_receipt),
                    partition("committed"),
                    1,
                    &mut bucket,
                    0,
                )
                .decision,
            OuterReplayDecision::SemanticCommitted
        );

        let mut terminal =
            AiNativeRecoveryLedger::new(10_000, 2, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        terminal.observe_events(&[ProviderStreamEvent::OutputProtocolFailure {
            failure: extension_contracts::provider_contract::ProviderOutputProtocolFailure {
                protocol: "openai_responses".to_string(),
                error_code: "semantic_terminal".to_string(),
                message: "provider rejected the semantic request".to_string(),
                retry_feedback: "none".to_string(),
                provider_details: json!({}),
            },
        }]);
        let terminal_receipt = logical_retry(terminal.current_epoch());
        assert_eq!(
            terminal
                .decide_outer_replay(
                    Some(&terminal_receipt),
                    partition("terminal"),
                    1,
                    &mut bucket,
                    0,
                )
                .decision,
            OuterReplayDecision::SemanticTerminal
        );
    }

    #[test]
    fn recovery_observation_is_metadata_only_and_uses_closed_terminology() {
        let mut ledger =
            AiNativeRecoveryLedger::new(10_000, 0, true, RecoveryInputMode::SemanticMapped)
                .unwrap();
        let receipt = logical_retry(ledger.current_epoch());
        let mut bucket = PartitionedRetryTokenBucket::default();
        let observation =
            ledger.decide_outer_replay(Some(&receipt), partition("provider-a"), 1, &mut bucket, 0);
        let value = serde_json::to_value(observation).unwrap();
        assert!(value.get("transport_epoch").is_some());
        assert!(value["provider_inner_receipt"]
            .get("socket_incarnation")
            .is_some());
        assert!(value.get("provider_final_commit").is_some());
        assert!(value.get("provider_disposition").is_some());
        let encoded = value.to_string();
        for forbidden in [
            "credential",
            "api_key",
            "prompt",
            "tool_output",
            "encrypted_content",
            "raw_cursor",
            "turn_state",
            "response_id",
        ] {
            assert!(!encoded.contains(forbidden));
        }
    }
}
