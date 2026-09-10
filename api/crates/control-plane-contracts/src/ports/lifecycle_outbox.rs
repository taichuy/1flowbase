use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleSubscriberTarget {
    pub subscriber_id: String,
    pub handler_id: String,
    pub handler_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecyclePublicationPlan {
    pub graph_fingerprint: String,
    pub subscribers: Vec<LifecycleSubscriberTarget>,
}

/// Host-private lifetime only: never serialized or included in graph/worker identity.
/// Transaction owners retain this envelope until their actual commit or rollback completes.
pub struct FrozenLifecyclePublication {
    pub plan: LifecyclePublicationPlan,
    _lifetime: Option<Box<dyn Send + Sync>>,
}
impl FrozenLifecyclePublication {
    pub fn native(plan: LifecyclePublicationPlan) -> Self {
        Self {
            plan,
            _lifetime: None,
        }
    }
    pub fn managed(plan: LifecyclePublicationPlan, lifetime: Box<dyn Send + Sync>) -> Self {
        Self {
            plan,
            _lifetime: Some(lifetime),
        }
    }
}
impl std::ops::Deref for FrozenLifecyclePublication {
    type Target = LifecyclePublicationPlan;
    fn deref(&self) -> &Self::Target {
        &self.plan
    }
}

#[derive(Clone, Default)]
pub struct LifecyclePublicationCatalog {
    plans: BTreeMap<(String, String), LifecyclePublicationPlan>,
    workspace_source: std::sync::Arc<
        std::sync::OnceLock<std::sync::Arc<dyn WorkspaceLifecyclePublicationSource>>,
    >,
}

impl LifecyclePublicationCatalog {
    pub fn new(
        plans: impl IntoIterator<Item = ((String, String), LifecyclePublicationPlan)>,
    ) -> anyhow::Result<Self> {
        let mut indexed = BTreeMap::new();
        for (contract, plan) in plans {
            if indexed.insert(contract.clone(), plan).is_some() {
                anyhow::bail!(
                    "duplicate lifecycle publication plan for {}@{}",
                    contract.0,
                    contract.1
                );
            }
        }
        Ok(Self {
            plans: indexed,
            workspace_source: Default::default(),
        })
    }

    pub fn plan_for(
        &self,
        contract_id: &str,
        contract_version: &str,
    ) -> Option<&LifecyclePublicationPlan> {
        self.plans
            .get(&(contract_id.to_string(), contract_version.to_string()))
    }
}

/// Reads one immutable workspace snapshot containing graph and subscriber plan together.
#[async_trait]
pub trait WorkspaceLifecyclePublicationSource: Send + Sync {
    async fn plan_for_workspace(
        &self,
        workspace_id: Uuid,
        contract_id: &str,
        contract_version: &str,
    ) -> anyhow::Result<Option<FrozenLifecyclePublication>>;
}
impl LifecyclePublicationCatalog {
    pub fn attach_workspace_source(
        &self,
        source: std::sync::Arc<dyn WorkspaceLifecyclePublicationSource>,
    ) -> anyhow::Result<()> {
        self.workspace_source
            .set(source)
            .map_err(|_| anyhow::anyhow!("workspace lifecycle publication source already attached"))
    }
    pub async fn frozen_plan_for_workspace(
        &self,
        workspace_id: Option<Uuid>,
        contract_id: &str,
        contract_version: &str,
    ) -> anyhow::Result<Option<FrozenLifecyclePublication>> {
        if let (Some(workspace), Some(source)) = (workspace_id, self.workspace_source.get()) {
            if let Some(plan) = source
                .plan_for_workspace(workspace, contract_id, contract_version)
                .await?
            {
                return Ok(Some(plan));
            }
        }
        Ok(self
            .plan_for(contract_id, contract_version)
            .cloned()
            .map(FrozenLifecyclePublication::native))
    }
}
impl std::fmt::Debug for LifecyclePublicationCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecyclePublicationCatalog")
            .field("plans", &self.plans)
            .field(
                "workspace_source_attached",
                &self.workspace_source.get().is_some(),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOutboxStatus {
    Pending,
    Claimed,
    Delivered,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLifecycleFactInput {
    pub event_id: Uuid,
    pub transaction_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub canonical_payload: Vec<u8>,
    pub occurred_at: OffsetDateTime,
    pub publication: LifecyclePublicationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleOutboxRecord {
    pub event_id: Uuid,
    pub transaction_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub canonical_payload: Vec<u8>,
    pub occurred_at: OffsetDateTime,
    pub graph_fingerprint: String,
    pub subscriber_id: String,
    pub handler_id: String,
    pub handler_version: String,
    pub status: LifecycleOutboxStatus,
    pub attempt_count: i32,
    pub available_at: OffsetDateTime,
    pub claimed_by: Option<Uuid>,
    pub claimed_at: Option<OffsetDateTime>,
    pub claim_id: Option<Uuid>,
    pub claim_expires_at: Option<OffsetDateTime>,
    pub pause_reason: Option<LifecycleDeliveryPauseReason>,
    pub paused_at: Option<OffsetDateTime>,
    pub delivered_at: Option<OffsetDateTime>,
}

#[async_trait]
pub trait LifecycleOutboxRepository: Send + Sync {
    async fn record_lifecycle_fact(
        &self,
        input: &RecordLifecycleFactInput,
    ) -> anyhow::Result<LifecycleOutboxRecord>;

    async fn claim_lifecycle_facts(
        &self,
        worker_id: Uuid,
        limit: u32,
        claim_lease: time::Duration,
    ) -> anyhow::Result<Vec<LifecycleOutboxRecord>>;

    async fn mark_lifecycle_fact_delivered(
        &self,
        event_id: Uuid,
        subscriber_id: &str,
        worker_id: Uuid,
        claim_id: Uuid,
    ) -> anyhow::Result<LifecycleOutboxRecord>;

    async fn retry_lifecycle_fact(
        &self,
        event_id: Uuid,
        subscriber_id: &str,
        worker_id: Uuid,
        claim_id: Uuid,
        available_at: OffsetDateTime,
        error: &str,
    ) -> anyhow::Result<LifecycleOutboxRecord>;
    async fn pause_lifecycle_fact(
        &self,
        event_id: Uuid,
        subscriber_id: &str,
        worker_id: Uuid,
        claim_id: Uuid,
        reason: LifecycleDeliveryPauseReason,
    ) -> anyhow::Result<LifecycleOutboxRecord>;
}

/// Same durable Outbox, with host-derived event identity. First successful publication freezes
/// time and targets; repeats must preserve payload/contract/transaction and reuse those targets.
#[async_trait]
pub trait DerivedLifecyclePublicationRepository: LifecycleOutboxRepository {
    async fn record_derived_lifecycle_fact(
        &self,
        input: &RecordLifecycleFactInput,
    ) -> anyhow::Result<LifecycleOutboxRecord>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleDeliveryPauseReason {
    FrozenGraphUnavailable,
    FrozenHandlerUnavailable,
    AuthorityRevoked,
    InstallationInactive,
    RetryBudgetExhausted,
}
impl LifecycleDeliveryPauseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FrozenGraphUnavailable => "frozen_graph_unavailable",
            Self::FrozenHandlerUnavailable => "frozen_handler_unavailable",
            Self::AuthorityRevoked => "authority_revoked",
            Self::InstallationInactive => "installation_inactive",
            Self::RetryBudgetExhausted => "retry_budget_exhausted",
        }
    }
}
/// An expired/replaced claim is a terminal result for that attempt, never a new retry owner.
#[derive(Debug, thiserror::Error)]
#[error("lifecycle delivery claim expired or replaced")]
pub struct LifecycleClaimLost;

#[derive(Debug, thiserror::Error)]
#[error("lifecycle delivery paused: {0:?}")]
pub struct LifecycleDeliveryBlocked(pub LifecycleDeliveryPauseReason);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedFrozenExecutionTarget {
    pub graph_fingerprint: String,
    pub handler_id: String,
    pub handler_version: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeManagedLifecycleDelivery {
    pub event_id: Uuid,
    pub subscriber_id: String,
    pub expected: ManagedFrozenExecutionTarget,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManagedExecutionReference {
    pub frozen_reference_count: usize,
    pub target: ManagedFrozenExecutionTarget,
    pub current: bool,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManagedLifecycleDelivery {
    pub event_id: Uuid,
    pub contract_id: String,
    pub contract_version: String,
    pub subscriber_id: String,
    pub target: ManagedFrozenExecutionTarget,
    pub status: String,
    pub pause_reason: Option<String>,
    pub ownership: String,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManagedExecutionState {
    pub installation_id: Uuid,
    pub workspace_id: Uuid,
    pub executions: Vec<ManagedExecutionReference>,
    pub deliveries: Vec<ManagedLifecycleDelivery>,
    /// Only the first bounded subscriber-history window was inspected; this is not an empty-backlog proof.
    pub deliveries_truncated: bool,
}

pub const MANAGED_DELIVERY_PAGE_LIMIT: usize = 256;
#[derive(Debug)]
pub struct ManagedLifecycleDeliveryPage {
    pub deliveries: Vec<ManagedLifecycleDelivery>,
    pub truncated: bool,
}
#[derive(Debug, thiserror::Error)]
#[error("managed backlog check busy: bounded inspection did not prove the scope empty")]
pub struct ManagedLifecycleBacklogCheckBusy;

/// Durable rows remain the delivery truth. No separate pause or retirement table.
#[async_trait]
pub trait ManagedLifecycleOutboxRepository: Send + Sync {
    async fn managed_lifecycle_delivery_page(
        &self,
        installation_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<ManagedLifecycleDeliveryPage>;
    async fn managed_lifecycle_delivery(
        &self,
        installation_id: Uuid,
        workspace_id: Uuid,
        input: &ResumeManagedLifecycleDelivery,
    ) -> anyhow::Result<Option<LifecycleOutboxRecord>>;
    /// None checks all installation backlog; Some checks exact targets plus unverifiable legacy rows.
    /// Exhausting the bounded inspection budget returns Busy, never a false empty result.
    async fn managed_installation_has_backlog(
        &self,
        installation_id: Uuid,
        workspace_id: Option<Uuid>,
        targets: Option<&[ManagedFrozenExecutionTarget]>,
    ) -> anyhow::Result<bool>;
    async fn lifecycle_target_has_backlog(
        &self,
        workspace_id: Uuid,
        graph_fingerprint: &str,
        target: &LifecycleSubscriberTarget,
    ) -> anyhow::Result<bool>;
}

/// Prevent artifact removal while host references can still create or execute a frozen target.
/// The caller holds the returned private guard through filesystem staging and DB completion.
#[async_trait]
pub trait ManagedArtifactRemovalGuard: Send + Sync {
    async fn guard_managed_artifact_removal(
        &self,
        installation_ids: &[Uuid],
    ) -> anyhow::Result<Box<dyn Send + Sync>>;
}

/// New opaque versions carry installation ownership. Pre-P09B and malformed versions have no
/// verifiable installation owner and must never be guessed into a resume operation.
pub fn managed_handler_installation(version: &str) -> Option<Uuid> {
    let parts = version.split(':').collect::<Vec<_>>();
    // Fingerprint text includes its own algorithm prefix; validate the complete emitted shape.
    if parts.len() != 7
        || Uuid::parse_str(parts[0]).is_err()
        || parts[1] != "sha256"
        || parts[3] != "sha256"
        || ![parts[2], parts[4]]
            .iter()
            .all(|digest| digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
        || parts[5].parse::<u64>().is_err()
    {
        return None;
    }
    Uuid::parse_str(parts[6]).ok()
}
