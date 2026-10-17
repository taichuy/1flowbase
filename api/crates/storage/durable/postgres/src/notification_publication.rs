//! Optional integration publication uses the existing immutable fact/per-target outbox.
//! It never applies a credit command, nor creates a second general EventBus snapshot.
use crate::PgControlPlaneStore;
use anyhow::{Result, ensure};
use async_trait::async_trait;
use control_plane_contracts::ports::*;
use extension_contracts::{AfterCommitFact, LifecycleContract};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

fn publication_id(input: &PublishOptionalNotificationInput) -> Uuid {
    let mut hash = Sha256::new();
    hash.update(b"1flowbase.optional-integration-publication/v1\0");
    hash.update(input.contract.contract_id().as_bytes());
    hash.update(b"\0");
    hash.update(input.contract.contract_version().as_bytes());
    hash.update(input.source_event_id.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80; // UUID v8, domain-separated host identity.
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn validate_fact<T: LifecycleContract + serde::de::DeserializeOwned>(
    input: &PublishOptionalNotificationInput,
) -> Result<AfterCommitFact<T>> {
    let fact: AfterCommitFact<T> = serde_json::from_slice(&input.canonical_payload)?;
    ensure!(
        fact.fact_id().as_str() == input.source_event_id.to_string(),
        "notification source identity mismatch"
    );
    ensure!(
        fact.transaction_id().as_str() == input.transaction_id.to_string(),
        "notification transaction mismatch"
    );
    ensure!(
        fact.contract().contract_id.as_str() == T::CONTRACT_ID
            && fact.contract().contract_version.as_str() == T::CONTRACT_VERSION,
        "notification typed contract mismatch"
    );
    let envelope: serde_json::Value = serde_json::from_slice(&input.canonical_payload)?;
    ensure!(
        envelope["occurred_at_unix_ms"].as_i64()
            == Some((input.occurred_at.unix_timestamp_nanos() / 1_000_000) as i64),
        "notification source timestamp mismatch"
    );
    Ok(fact)
}

impl PgControlPlaneStore {
    pub(crate) async fn publish_optional_notification_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        input: &PublishOptionalNotificationInput,
    ) -> Result<(
        OptionalNotificationPublicationOutcome,
        Option<FrozenLifecyclePublication>,
    )> {
        match input.contract {
            OptionalNotificationContract::Credit => {
                let fact = validate_fact::<CreditNotificationFact>(input)?;
                ensure!(
                    Some(fact.payload().workspace_id) == input.workspace_id,
                    "credit notification scope mismatch"
                );
            }
            OptionalNotificationContract::ModelDefinition => {
                let fact = validate_fact::<ModelDefinitionIntegrationNotification>(input)?;
                let scope = (fact.payload().scope_kind == domain::DataModelScopeKind::Workspace)
                    .then_some(fact.payload().scope_id);
                ensure!(
                    scope == input.workspace_id,
                    "model notification scope mismatch"
                );
            }
        }
        if input.contract == OptionalNotificationContract::ModelDefinition {
            self.validate_original_model_source(tx, input).await?;
        }
        let event_id = publication_id(input);
        // Same lock identity as the existing derived-publication helper. Resolve old
        // frozen responsibility before a new zero-target/current-plan decision.
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("managed-derived-publication:{event_id}"))
            .execute(&mut **tx)
            .await?;
        let existing = sqlx::query("select f.graph_fingerprint,d.subscriber_id,d.handler_id,d.handler_version from lifecycle_outbox f join lifecycle_outbox_deliveries d using(event_id) where f.event_id=$1 order by d.subscriber_id")
            .bind(event_id).fetch_all(&mut **tx).await?;
        let publication = if let Some(first) = existing.first() {
            Some(FrozenLifecyclePublication::native(
                LifecyclePublicationPlan {
                    graph_fingerprint: first.try_get("graph_fingerprint")?,
                    subscribers: existing
                        .iter()
                        .map(|row| {
                            Ok(LifecycleSubscriberTarget {
                                subscriber_id: row.try_get("subscriber_id")?,
                                handler_id: row.try_get("handler_id")?,
                                handler_version: row.try_get("handler_version")?,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                },
            ))
        } else {
            // The only accepted contracts explicitly permit optional-zero; a
            // workspace source error is never converted into NoTargets.
            self.lifecycle_publication_catalog
                .frozen_plan_for_workspace(
                    input.workspace_id,
                    input.contract.contract_id(),
                    input.contract.contract_version(),
                )
                .await?
        };
        if publication
            .as_ref()
            .is_none_or(|plan| plan.subscribers.is_empty())
        {
            return Ok((
                OptionalNotificationPublicationOutcome::NoTargets,
                publication,
            ));
        }
        let publication = publication.expect("nonempty publication checked above");
        let target_count = publication.subscribers.len();
        crate::lifecycle_outbox_repository::record_derived_lifecycle_fact_in_transaction(
            tx,
            &RecordLifecycleFactInput {
                event_id,
                transaction_id: input.transaction_id,
                contract_id: input.contract.contract_id().into(),
                contract_version: input.contract.contract_version().into(),
                canonical_payload: input.canonical_payload.clone(),
                occurred_at: input.occurred_at,
                publication: publication.plan.clone(),
            },
        )
        .await?;
        // Return the freeze owner: the actual transaction caller keeps it through
        // commit/rollback, rather than releasing a managed snapshot at helper return.
        Ok((
            OptionalNotificationPublicationOutcome::Enqueued {
                event_id,
                target_count,
            },
            Some(publication),
        ))
    }
    async fn validate_original_model_source(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        input: &PublishOptionalNotificationInput,
    ) -> Result<()> {
        // The original committed fact remains authoritative for identity and scope.
        let row = sqlx::query("select transaction_id,canonical_payload from lifecycle_outbox where event_id=$1 and contract_id='model_definition.committed' and contract_version='v1' for update")
            .bind(input.source_event_id).fetch_one(&mut **tx).await?;
        ensure!(
            row.try_get::<Uuid, _>("transaction_id")? == input.transaction_id,
            "model source transaction mismatch"
        );
        let original: AfterCommitFact<ModelDefinitionCommittedFact> =
            serde_json::from_slice(&row.try_get::<Vec<u8>, _>("canonical_payload")?)?;
        let proposed: AfterCommitFact<ModelDefinitionIntegrationNotification> =
            serde_json::from_slice(&input.canonical_payload)?;
        ensure!(
            original.fact_id() == proposed.fact_id()
                && original.transaction_id() == proposed.transaction_id(),
            "model source identity mismatch"
        );
        ensure!(
            original.contract().contract_id.as_str() == ModelDefinitionCommittedFact::CONTRACT_ID
                && original.contract().contract_version.as_str()
                    == ModelDefinitionCommittedFact::CONTRACT_VERSION,
            "model source contract mismatch"
        );
        ensure!(
            original.payload().model_definition_id == proposed.payload().model_definition_id
                && original.payload().scope_kind == proposed.payload().scope_kind
                && original.payload().scope_id == proposed.payload().scope_id,
            "model source body mismatch"
        );
        let original_value = serde_json::to_value(&original)?;
        let proposed_value = serde_json::to_value(&proposed)?;
        ensure!(
            original_value["occurred_at_unix_ms"] == proposed_value["occurred_at_unix_ms"],
            "model source time mismatch"
        );
        Ok(())
    }
}

#[async_trait]
impl OptionalNotificationPublicationPort for PgControlPlaneStore {
    async fn publish_optional_notification(
        &self,
        input: &PublishOptionalNotificationInput,
    ) -> Result<OptionalNotificationPublicationOutcome> {
        let mut tx = self.pool().begin().await?;
        let (outcome, publication) = self
            .publish_optional_notification_in_transaction(&mut tx, input)
            .await?;
        tx.commit().await?;
        drop(publication);
        Ok(outcome)
    }
}

/// The original money transaction owns this publication. Optional-zero produces
/// no transport row; the account/session/ledger already hold monetary facts.
/// Rejections have no monetary ledger, so always retain their original source
/// fact independently of the temporary notification transport. Its timestamp
/// records completion of source publication, never a subscriber business ACK.
pub(crate) async fn record_credit_notification_in_transaction(
    store: &PgControlPlaneStore,
    tx: &mut Transaction<'_, Postgres>,
    lifetimes: &mut Vec<FrozenLifecyclePublication>,
    workspace_id: Uuid,
    account_id: Option<Uuid>,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<()> {
    let event_type = CreditNotificationEventType::from_event_type(event_type)?;
    let source_event_id = Uuid::now_v7();
    let transaction_id = payload
        .get("transaction_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::now_v7);
    let occurred_at = time::OffsetDateTime::now_utc();
    let fact = AfterCommitFact::new(
        extension_contracts::LifecycleFactId::new(source_event_id.to_string())?,
        extension_contracts::LifecycleTransactionId::new(transaction_id.to_string())?,
        (occurred_at.unix_timestamp_nanos() / 1_000_000) as i64,
        CreditNotificationFact {
            workspace_id,
            account_id,
            event_type,
            payload: payload.clone(),
        },
    );
    let (_, lifetime) = store
        .publish_optional_notification_in_transaction(
            tx,
            &PublishOptionalNotificationInput {
                source_event_id,
                transaction_id,
                workspace_id: Some(workspace_id),
                occurred_at,
                contract: OptionalNotificationContract::Credit,
                canonical_payload: serde_json::to_vec(&fact)?,
            },
        )
        .await?;
    if let Some(lifetime) = lifetime {
        lifetimes.push(lifetime);
    }
    if event_type == CreditNotificationEventType::CreditCommandRejected {
        sqlx::query("insert into credit_event_outbox(event_id,workspace_id,account_id,event_type,payload,created_at,published_at) values($1,$2,$3,'CreditCommandRejected',$4,$5,$5)")
            .bind(source_event_id).bind(workspace_id).bind(account_id).bind(payload).bind(occurred_at)
            .execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "_tests/notification_publication/mod.rs"]
mod tests;
