use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use async_trait::async_trait;
use plugin_framework::extension_bus::{
    Cardinality, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointKind, FailureSemantics,
    LifecycleSemantics, ModuleKind, OrderingSemantics, OverridePolicy, ScopeSemantics,
};
use serde::Serialize;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use super::runtime_events::{RuntimeEventDurability, RuntimeEventEnvelope};

pub const RUNTIME_EVENT_REQUIRED_POINT_ID: &str = "1flowbase.application.runtime-event.required";
pub const RUNTIME_EVENT_DIAGNOSTIC_POINT_ID: &str =
    "1flowbase.application.runtime-event.diagnostic";
pub const RUNTIME_EVENT_AFTER_COMMIT_POINT_ID: &str =
    "1flowbase.application.runtime-event.after-commit";
pub const RUNTIME_EVENT_REQUIRED_CONTRACT_ID: &str = "runtime-event-required-stream";
pub const RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID: &str = "runtime-event-diagnostic-stream";
pub const RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID: &str = "runtime-event-after-commit-stream";
pub const RUNTIME_EVENT_LANE_CONTRACT_VERSION: &str = "1";
pub const RUNTIME_EVENT_LANE_OWNER_MODULE_ID: &str = "1flowbase.boot-core";
const RUNTIME_EVENT_AFTER_COMMIT_MAX_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEventRequiredDeliveryStatus {
    Delivered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventRequiredDeliveryReceipt {
    pub event_id: String,
    pub sequence: i64,
    pub status: RuntimeEventRequiredDeliveryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEventRequiredDeliveryError {
    ReceiverClosed,
}

impl std::fmt::Display for RuntimeEventRequiredDeliveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("required runtime event receiver is closed")
    }
}

impl std::error::Error for RuntimeEventRequiredDeliveryError {}

#[derive(Clone)]
pub struct RuntimeEventRequiredLane {
    sender: mpsc::Sender<RuntimeEventEnvelope>,
}

impl RuntimeEventRequiredLane {
    pub(crate) fn new(sender: mpsc::Sender<RuntimeEventEnvelope>) -> Self {
        Self { sender }
    }

    pub async fn send(
        &self,
        event: RuntimeEventEnvelope,
    ) -> std::result::Result<RuntimeEventRequiredDeliveryReceipt, RuntimeEventRequiredDeliveryError>
    {
        let receipt = RuntimeEventRequiredDeliveryReceipt {
            event_id: event.event_id.clone(),
            sequence: event.sequence,
            status: RuntimeEventRequiredDeliveryStatus::Delivered,
        };
        self.sender
            .send(event)
            .await
            .map_err(|_| RuntimeEventRequiredDeliveryError::ReceiverClosed)?;
        Ok(receipt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEventDiagnosticDropReason {
    ReceiverFull,
    ReceiverClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEventDiagnosticDeliveryStatus {
    Delivered,
    Dropped(RuntimeEventDiagnosticDropReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventDiagnosticDeliveryReceipt {
    pub event_id: String,
    pub sequence: i64,
    pub status: RuntimeEventDiagnosticDeliveryStatus,
    pub dropped_total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeEventDiagnosticDeliverySnapshot {
    pub dropped_total: u64,
    pub receiver_full: u64,
    pub receiver_closed: u64,
}

#[derive(Default)]
pub(crate) struct RuntimeEventDiagnosticDeliveryCounters {
    dropped_total: AtomicU64,
    receiver_full: AtomicU64,
    receiver_closed: AtomicU64,
}

impl RuntimeEventDiagnosticDeliveryCounters {
    fn record(&self, reason: RuntimeEventDiagnosticDropReason) -> u64 {
        match reason {
            RuntimeEventDiagnosticDropReason::ReceiverFull => {
                self.receiver_full.fetch_add(1, Ordering::Relaxed);
            }
            RuntimeEventDiagnosticDropReason::ReceiverClosed => {
                self.receiver_closed.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.dropped_total.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn snapshot(&self) -> RuntimeEventDiagnosticDeliverySnapshot {
        RuntimeEventDiagnosticDeliverySnapshot {
            dropped_total: self.dropped_total.load(Ordering::Relaxed),
            receiver_full: self.receiver_full.load(Ordering::Relaxed),
            receiver_closed: self.receiver_closed.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone)]
pub struct RuntimeEventDiagnosticLane {
    sender: mpsc::Sender<RuntimeEventEnvelope>,
    counters: Arc<RuntimeEventDiagnosticDeliveryCounters>,
}

impl RuntimeEventDiagnosticLane {
    pub(crate) fn new(
        sender: mpsc::Sender<RuntimeEventEnvelope>,
        counters: Arc<RuntimeEventDiagnosticDeliveryCounters>,
    ) -> Self {
        Self { sender, counters }
    }

    pub fn try_send(&self, event: RuntimeEventEnvelope) -> RuntimeEventDiagnosticDeliveryReceipt {
        let event_id = event.event_id.clone();
        let sequence = event.sequence;
        let (status, dropped_total) = match self.sender.try_send(event) {
            Ok(()) => (
                RuntimeEventDiagnosticDeliveryStatus::Delivered,
                self.counters.snapshot().dropped_total,
            ),
            Err(mpsc::error::TrySendError::Full(_)) => {
                let reason = RuntimeEventDiagnosticDropReason::ReceiverFull;
                (
                    RuntimeEventDiagnosticDeliveryStatus::Dropped(reason),
                    self.counters.record(reason),
                )
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                let reason = RuntimeEventDiagnosticDropReason::ReceiverClosed;
                (
                    RuntimeEventDiagnosticDeliveryStatus::Dropped(reason),
                    self.counters.record(reason),
                )
            }
        };
        RuntimeEventDiagnosticDeliveryReceipt {
            event_id,
            sequence,
            status,
            dropped_total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeEventAfterCommitSubscriberId(String);

impl RuntimeEventAfterCommitSubscriberId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            anyhow::bail!("runtime event after-commit subscriber id must not be empty");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RuntimeEventAfterCommitIdempotencyKey {
    pub run_id: Uuid,
    pub sequence: i64,
    pub subscriber_id: RuntimeEventAfterCommitSubscriberId,
}

#[derive(Debug, Clone)]
pub struct RuntimeEventAfterCommitDelivery {
    pub event: RuntimeEventEnvelope,
    pub idempotency_key: RuntimeEventAfterCommitIdempotencyKey,
}

#[async_trait]
pub trait RuntimeEventAfterCommitSubscriber: Send + Sync {
    async fn deliver(&self, delivery: RuntimeEventAfterCommitDelivery) -> Result<()>;
}

#[derive(Clone)]
pub struct TrustedRuntimeEventAfterCommitRegistration {
    pub contribution_id: String,
    pub subscriber_id: RuntimeEventAfterCommitSubscriberId,
    pub timeout: Duration,
    pub max_attempts: u32,
    pub subscriber: Arc<dyn RuntimeEventAfterCommitSubscriber>,
}

#[derive(Clone)]
struct OrderedAfterCommitSubscriber {
    contribution_id: String,
    registration: TrustedRuntimeEventAfterCommitRegistration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventAfterCommitFailureReason {
    SubscriberError,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventAfterCommitDeliveryStatus {
    Delivered,
    DuplicateSuppressed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeEventAfterCommitSubscriberReceipt {
    pub contribution_id: String,
    pub subscriber_id: RuntimeEventAfterCommitSubscriberId,
    pub idempotency_key: RuntimeEventAfterCommitIdempotencyKey,
    pub status: RuntimeEventAfterCommitDeliveryStatus,
    pub attempts: u32,
    pub duration_micros: u64,
    pub failure_reason: Option<RuntimeEventAfterCommitFailureReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeEventAfterCommitReceipt {
    pub graph_fingerprint: Option<String>,
    pub event_id: String,
    pub run_id: Uuid,
    pub sequence: i64,
    pub skipped_ephemeral: bool,
    pub subscribers: Vec<RuntimeEventAfterCommitSubscriberReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeEventAfterCommitClaimState {
    InFlight,
    Completed,
}

#[derive(Clone)]
pub struct RuntimeEventAfterCommitLane {
    graph_fingerprint: Option<String>,
    subscribers: Vec<OrderedAfterCommitSubscriber>,
    claims: Arc<
        Mutex<BTreeMap<RuntimeEventAfterCommitIdempotencyKey, RuntimeEventAfterCommitClaimState>>,
    >,
}

impl Default for RuntimeEventAfterCommitLane {
    fn default() -> Self {
        Self {
            graph_fingerprint: None,
            subscribers: Vec::new(),
            claims: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

impl RuntimeEventAfterCommitLane {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_graph(
        graph: Arc<EffectiveExtensionGraph>,
        registrations: Vec<TrustedRuntimeEventAfterCommitRegistration>,
    ) -> Result<Self> {
        let point = graph
            .points()
            .iter()
            .find(|point| {
                point.descriptor().point_id.as_str() == RUNTIME_EVENT_AFTER_COMMIT_POINT_ID
            })
            .ok_or_else(|| anyhow::anyhow!("runtime event after-commit point is unavailable"))?;
        let descriptor = point.descriptor();
        if descriptor.owner_module_id.as_str() != RUNTIME_EVENT_LANE_OWNER_MODULE_ID
            || descriptor.point_kind != ExtensionPointKind::EventStream
            || descriptor.contract.contract_id.as_str() != RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID
            || descriptor.contract.contract_version.as_str() != RUNTIME_EVENT_LANE_CONTRACT_VERSION
            || descriptor.scope != ScopeSemantics::Global
            || descriptor.cardinality != Cardinality::Many
            || descriptor.ordering != OrderingSemantics::Dependency
            || descriptor.failure != FailureSemantics::IsolateContribution
            || descriptor.delivery != DeliverySemantics::AfterCommitDurable
            || descriptor.lifecycle != LifecycleSemantics::Invocation
            || descriptor.override_policy != OverridePolicy::Sealed
        {
            anyhow::bail!("runtime event after-commit point contract mismatch");
        }

        let mut registrations_by_id = BTreeMap::new();
        let mut subscriber_ids = BTreeSet::new();
        for registration in registrations {
            if registration.timeout.is_zero()
                || registration.max_attempts == 0
                || registration.max_attempts > RUNTIME_EVENT_AFTER_COMMIT_MAX_ATTEMPTS
            {
                anyhow::bail!("runtime event after-commit subscriber policy is invalid");
            }
            if !subscriber_ids.insert(registration.subscriber_id.clone()) {
                anyhow::bail!("duplicate runtime event after-commit subscriber id");
            }
            if registrations_by_id
                .insert(registration.contribution_id.clone(), registration)
                .is_some()
            {
                anyhow::bail!("duplicate runtime event after-commit registration");
            }
        }

        let mut subscribers = Vec::new();
        for contribution in point.contributions() {
            if !matches!(
                contribution.provenance().module_kind(),
                ModuleKind::BootCore | ModuleKind::TrustedHost
            ) {
                anyhow::bail!("runtime event after-commit subscriber is not trusted");
            }
            let contribution_id = contribution.descriptor().contribution_id.as_str();
            let registration = registrations_by_id.remove(contribution_id).ok_or_else(|| {
                anyhow::anyhow!("runtime event after-commit subscriber is not registered")
            })?;
            subscribers.push(OrderedAfterCommitSubscriber {
                contribution_id: contribution_id.to_string(),
                registration,
            });
        }
        if !registrations_by_id.is_empty() {
            anyhow::bail!("runtime event after-commit registration is absent from graph");
        }

        Ok(Self {
            graph_fingerprint: Some(graph.fingerprint().as_str().to_string()),
            subscribers,
            claims: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub(crate) async fn deliver_after_commit(
        &self,
        event: RuntimeEventEnvelope,
    ) -> RuntimeEventAfterCommitReceipt {
        if event.durability == RuntimeEventDurability::Ephemeral {
            return RuntimeEventAfterCommitReceipt {
                graph_fingerprint: self.graph_fingerprint.clone(),
                event_id: event.event_id,
                run_id: event.run_id,
                sequence: event.sequence,
                skipped_ephemeral: true,
                subscribers: Vec::new(),
            };
        }
        let mut receipts = Vec::new();
        for subscriber in &self.subscribers {
            let key = RuntimeEventAfterCommitIdempotencyKey {
                run_id: event.run_id,
                sequence: event.sequence,
                subscriber_id: subscriber.registration.subscriber_id.clone(),
            };
            let claimed = {
                let mut claims = self.claims.lock().await;
                if claims.contains_key(&key) {
                    false
                } else {
                    claims.insert(key.clone(), RuntimeEventAfterCommitClaimState::InFlight);
                    true
                }
            };
            if !claimed {
                receipts.push(RuntimeEventAfterCommitSubscriberReceipt {
                    contribution_id: subscriber.contribution_id.clone(),
                    subscriber_id: subscriber.registration.subscriber_id.clone(),
                    idempotency_key: key,
                    status: RuntimeEventAfterCommitDeliveryStatus::DuplicateSuppressed,
                    attempts: 0,
                    duration_micros: 0,
                    failure_reason: None,
                });
                continue;
            }

            let started = Instant::now();
            let mut attempts = 0;
            let mut failure_reason = None;
            while attempts < subscriber.registration.max_attempts {
                attempts += 1;
                let delivery = RuntimeEventAfterCommitDelivery {
                    event: event.clone(),
                    idempotency_key: key.clone(),
                };
                match tokio::time::timeout(
                    subscriber.registration.timeout,
                    subscriber.registration.subscriber.deliver(delivery),
                )
                .await
                {
                    Ok(Ok(())) => {
                        failure_reason = None;
                        break;
                    }
                    Ok(Err(_)) => {
                        failure_reason = Some(RuntimeEventAfterCommitFailureReason::SubscriberError)
                    }
                    Err(_) => failure_reason = Some(RuntimeEventAfterCommitFailureReason::Timeout),
                }
            }
            {
                let mut claims = self.claims.lock().await;
                if failure_reason.is_none() {
                    claims.insert(key.clone(), RuntimeEventAfterCommitClaimState::Completed);
                } else if claims.get(&key) == Some(&RuntimeEventAfterCommitClaimState::InFlight) {
                    claims.remove(&key);
                }
            }
            receipts.push(RuntimeEventAfterCommitSubscriberReceipt {
                contribution_id: subscriber.contribution_id.clone(),
                subscriber_id: subscriber.registration.subscriber_id.clone(),
                idempotency_key: key,
                status: if failure_reason.is_none() {
                    RuntimeEventAfterCommitDeliveryStatus::Delivered
                } else {
                    RuntimeEventAfterCommitDeliveryStatus::Failed
                },
                attempts,
                duration_micros: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
                failure_reason,
            });
        }

        RuntimeEventAfterCommitReceipt {
            graph_fingerprint: self.graph_fingerprint.clone(),
            event_id: event.event_id,
            run_id: event.run_id,
            sequence: event.sequence,
            skipped_ephemeral: false,
            subscribers: receipts,
        }
    }
}
