//! Publication command for the sealed composition event. The caller holds the current
//! contribution-authority lease through the host Outbox commit on that same connection.
use crate::plugin_management::HostContributionGrantPolicy;
use anyhow::{bail, Result};
use control_plane_contracts::ports::{
    ContributionAuthorityLease, LifecyclePublicationPlan, RecordLifecycleFactInput,
};
use extension_contracts::{
    extension_bus::*, ManagedEventDelivery, ManagedEventFact, ManagedEventPublication,
    MANAGED_PROCESSED_EVENT_ID,
};
use uuid::Uuid;

/// Exact current subject and granted permissions, not the original Create actor or another
/// contribution from the same package. Kept explicit for all host publication consumers.
pub fn validate_managed_event_publication(
    installation: &domain::PluginInstallationRecord,
    authority: &domain::PluginContributionAuthoritySnapshot,
    identity: &ManagedExecutionIdentity,
    contribution: &ContributionDescriptor,
    cause: &ManagedEventDelivery,
    publication: &ManagedEventPublication,
) -> Result<()> {
    publication.validate()?;
    cause.validate()?;
    let workspace_id = Uuid::parse_str(identity.workspace_id().as_str())?;
    if installation.id.to_string() != identity.installation_id().as_str()
        || installation.organization != "acme"
        || installation.provider_code != "acme.composition-a"
        || contribution.contributor_module_id.as_str() != installation.provider_code
        || contribution.contribution_id != *identity.contribution_id()
        || installation.desired_state != domain::PluginDesiredState::ActiveRequested
        || cause.workspace_id != workspace_id.to_string()
        || cause.contract_id != extension_contracts::MANAGED_CREATE_EVENT_ID
        || cause.contract_version != "v1"
        || contribution.point_id.as_str() != extension_contracts::MANAGED_CREATE_EVENT_POINT
        || cause.payload.model_id != publication.payload.model_id
        || publication.contract_id != MANAGED_PROCESSED_EVENT_ID
    {
        bail!("managed event publisher or workspace mismatch");
    }
    let permissions = HostContributionGrantPolicy::root_composition().effective_permissions(
        installation,
        workspace_id,
        contribution,
        authority,
    )?;
    if !contribution
        .required_permissions
        .iter()
        .any(|p| p.as_str() == "event.publish")
        || !permissions.iter().any(|p| p.as_str() == "event.publish")
    {
        bail!("managed event publication permission required");
    }
    Ok(())
}

pub async fn publish_managed_event(
    lease: Box<dyn ContributionAuthorityLease>,
    identity: &ManagedExecutionIdentity,
    contribution: &ContributionDescriptor,
    cause: &ManagedEventDelivery,
    publication: ManagedEventPublication,
    plan: LifecyclePublicationPlan,
) -> Result<Uuid> {
    let installation = lease
        .installation(Uuid::parse_str(identity.installation_id().as_str())?)
        .ok_or_else(|| anyhow::anyhow!("managed publisher installation missing"))?;
    let authority = lease.snapshot();
    validate_managed_event_publication(
        installation,
        authority,
        identity,
        contribution,
        cause,
        &publication,
    )?;
    let event_id = derived_identity("event", identity.subject(), &cause.event_id)?;
    // Distinct from both the consumed event's transaction and the original Create transaction.
    let transaction_id = derived_identity(
        "publication-transaction",
        identity.subject(),
        &cause.event_id,
    )?;
    let fact = ManagedEventFact {
        event_id: event_id.to_string(),
        transaction_id: transaction_id.to_string(),
        contract_id: publication.contract_id.clone(),
        contract_version: publication.contract_version.clone(),
        workspace_id: identity.workspace_id().as_str().into(),
        publisher: identity.clone(),
        causation_id: cause.event_id.clone(),
        correlation_id: cause.correlation_id.clone(),
        payload: publication.payload,
    };
    lease
        .commit_derived_lifecycle_fact(RecordLifecycleFactInput {
            event_id,
            transaction_id,
            contract_id: publication.contract_id,
            contract_version: publication.contract_version,
            canonical_payload: serde_json::to_vec(&fact)?,
            occurred_at: time::OffsetDateTime::now_utc(),
            publication: plan,
        })
        .await?;
    Ok(event_id)
}

/// UUIDv8 derived solely from host-owned causal identity, stable subject and the one opened
/// output ordinal. Worker generation, worker event IDs and actor identity never participate.
fn derived_identity(
    kind: &str,
    subject: &ManagedContributionSubject,
    cause_id: &str,
) -> Result<Uuid> {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(serde_json::to_vec(&(
        "1flowbase.derived-publication/v1",
        kind,
        subject,
        cause_id,
        MANAGED_PROCESSED_EVENT_ID,
        "1",
        0u8,
    ))?);
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Uuid::from_bytes(bytes))
}
