use std::time::Duration;

use control_plane::ports::EphemeralEntrySnapshot;
use orchestration_runtime::transport_session::{
    DeadlineKind, SafeRegistrySnapshot, TerminationKind, TransportSessionState,
};

const CONTRACT_CODE: &str = "provider-transport-sessions";

pub(super) fn project(snapshot: SafeRegistrySnapshot) -> Vec<EphemeralEntrySnapshot> {
    let observed_at_ms = snapshot.observed_at.as_millis();
    let tombstone_ttl_ms = duration_millis(snapshot.tombstone_ttl);
    let mut entries = snapshot
        .sessions
        .into_iter()
        .map(|session| {
            let provider = session.provider_id.as_str();
            let target = session.runtime_target_id.as_str();
            let logical_session = session.fence.session_id.as_str();
            let generation = session.fence.generation.get();
            let ttl = session.logical_ttl.min(session.physical_ttl);
            let expires_at_ms = observed_at_ms.saturating_add(duration_millis(ttl));
            let metadata = serde_json::json!({
                "status": state_name(session.state),
                "owner": session.owner_id.as_str(),
                "logical_ttl_ms": duration_millis(session.logical_ttl),
                "physical_ttl_ms": duration_millis(session.physical_ttl),
                "expires_at_unix_ms": expires_at_ms,
                "generation": generation,
                "connection_age_ms": duration_millis(session.age),
                "state_age_ms": duration_millis(session.state_age),
                "downstream_runtime_target_id": target,
                "active_invocation": session.inflight,
                "deadline_kind": deadline_name(session.deadline_kind),
                "eviction_priority": session.eviction_priority,
                "close_reason": null,
                "close_acknowledged": null,
            });
            entry(
                provider,
                logical_session,
                generation,
                "physical_generation",
                state_name(session.state),
                session.owner_id.as_str(),
                Some(ttl),
                observed_at_ms.saturating_sub(duration_millis(session.age)),
                expires_at_ms,
                metadata,
            )
        })
        .collect::<Vec<_>>();

    entries.extend(snapshot.tombstones.into_iter().map(|receipt| {
        let provider = receipt.provider_id.as_str();
        let target = receipt.runtime_target_id.as_str();
        let logical_session = receipt.fence.session_id.as_str();
        let generation = receipt.fence.generation.get();
        let elapsed_ms = observed_at_ms.saturating_sub(receipt.terminated_at.as_millis());
        let remaining_ms = tombstone_ttl_ms.saturating_sub(elapsed_ms);
        let metadata = serde_json::json!({
            "status": "terminated",
            "owner": receipt.owner_id.as_str(),
            "logical_ttl_ms": 0,
            "physical_ttl_ms": 0,
            "tombstone_ttl_ms": remaining_ms,
            "expires_at_unix_ms": receipt.terminated_at.as_millis().saturating_add(tombstone_ttl_ms),
            "generation": generation,
            "connection_age_ms": duration_millis(receipt.connection_age),
            "downstream_runtime_target_id": target,
            "active_invocation": false,
            "deadline_kind": termination_deadline_name(receipt.kind),
            "eviction_priority": eviction_priority(receipt.previous_state),
            "close_reason": termination_name(receipt.kind),
            "close_acknowledged": receipt.close_acknowledged,
        });
        entry(
            provider,
            logical_session,
            generation,
            "physical_generation_tombstone",
            "terminated",
            receipt.owner_id.as_str(),
            Some(Duration::from_millis(remaining_ms)),
            receipt
                .terminated_at
                .as_millis()
                .saturating_sub(duration_millis(receipt.connection_age)),
            receipt
                .terminated_at
                .as_millis()
                .saturating_add(tombstone_ttl_ms),
            metadata,
        )
    }));
    entries
}

#[expect(
    clippy::too_many_arguments,
    reason = "the arguments mirror the generic observation DTO"
)]
fn entry(
    provider: &str,
    logical_session: &str,
    generation: u64,
    entry_kind: &str,
    status: &str,
    owner: &str,
    ttl: Option<Duration>,
    created_at_ms: u64,
    expires_at_ms: u64,
    metadata: serde_json::Value,
) -> EphemeralEntrySnapshot {
    let inspection_path = vec![
        provider.to_string(),
        logical_session.to_string(),
        generation.to_string(),
    ];
    EphemeralEntrySnapshot {
        contract_code: CONTRACT_CODE.to_string(),
        group_code: Some(provider.to_string()),
        entry_ref: inspection_path.join("/"),
        key: generation.to_string(),
        inspection_path,
        entry_kind: entry_kind.to_string(),
        status: status.to_string(),
        owner: Some(owner.to_string()),
        value_size_bytes: 0,
        metadata_size_bytes: serde_json::to_vec(&metadata)
            .map(|value| value.len() as u64)
            .unwrap_or(0),
        ttl_seconds: ttl.map(|value| value.as_secs().min(i64::MAX as u64) as i64),
        created_at_unix: Some(millis_to_unix_seconds(created_at_ms)),
        expires_at_unix: Some(millis_to_unix_seconds(expires_at_ms)),
        sensitive: false,
        metadata,
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn millis_to_unix_seconds(millis: u64) -> i64 {
    (millis / 1_000).min(i64::MAX as u64) as i64
}

fn state_name(state: TransportSessionState) -> &'static str {
    match state {
        TransportSessionState::Opening => "opening",
        TransportSessionState::Active => "active",
        TransportSessionState::WaitingTool => "waiting_tool",
        TransportSessionState::IdleAffinity => "idle_affinity",
        TransportSessionState::Orphaned => "orphaned",
        TransportSessionState::Draining => "draining",
        TransportSessionState::Faulted => "faulted",
        TransportSessionState::Closing => "closing",
    }
}

fn deadline_name(kind: DeadlineKind) -> &'static str {
    match kind {
        DeadlineKind::Task => "task",
        DeadlineKind::LogicalAbsolute => "logical_absolute",
        DeadlineKind::StateLease => "state_lease",
        DeadlineKind::PhysicalHard => "physical_hard",
    }
}

fn termination_deadline_name(kind: TerminationKind) -> Option<&'static str> {
    match kind {
        TerminationKind::DeadlineExceeded(kind) => Some(deadline_name(kind)),
        TerminationKind::ProviderHardMax => Some("physical_hard"),
        _ => None,
    }
}

fn termination_name(kind: TerminationKind) -> &'static str {
    match kind {
        TerminationKind::DeadlineExceeded(_) => "deadline_exceeded",
        TerminationKind::CapacityEvicted => "capacity_evicted",
        TerminationKind::ProviderFault => "provider_fault",
        TerminationKind::ProviderHardMax => "provider_hard_max",
        TerminationKind::OwnerOrphaned => "owner_orphaned",
        TerminationKind::OwnerClosed => "owner_closed",
        TerminationKind::Shutdown => "shutdown",
    }
}

fn eviction_priority(state: TransportSessionState) -> Option<u8> {
    match state {
        TransportSessionState::Orphaned => Some(0),
        TransportSessionState::IdleAffinity => Some(1),
        TransportSessionState::Draining => Some(2),
        TransportSessionState::WaitingTool => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestration_runtime::transport_session::{
        AdmissionRequest, TransportClock, TransportInstant, TransportOwnerId, TransportProviderId,
        TransportRegistryConfig, TransportRuntimeTargetId, TransportSessionId,
        TransportSessionRegistry,
    };

    #[derive(Clone)]
    struct FixedClock(TransportInstant);

    impl TransportClock for FixedClock {
        fn now(&self) -> TransportInstant {
            self.0
        }
    }

    #[test]
    fn projects_provider_logical_session_and_physical_generation_without_payload_fields() {
        let clock = FixedClock(TransportInstant::from_millis(10_000));
        let mut config = TransportRegistryConfig::default();
        config.invocation_default = Duration::from_secs(30);
        let mut registry = TransportSessionRegistry::new(clock, config).unwrap();
        let fence = registry
            .admit(AdmissionRequest {
                session_id: TransportSessionId::new("logical-a").unwrap(),
                owner_id: TransportOwnerId::new("owner-a").unwrap(),
                provider_id: TransportProviderId::new("provider-a").unwrap(),
                runtime_target_id: TransportRuntimeTargetId::new("runtime-target-a").unwrap(),
                task_deadline: None,
                provider_hard_deadline: None,
            })
            .unwrap();
        registry.activate(&fence).unwrap();
        registry.begin_invocation(&fence).unwrap();

        let entries = project(registry.safe_snapshot());
        assert_eq!(entries[0].inspection_path, ["provider-a", "logical-a", "1"]);
        assert_eq!(entries[0].ttl_seconds, Some(30));
        assert_eq!(entries[0].metadata["active_invocation"], true);
        let encoded = serde_json::to_string(&entries).unwrap();
        for forbidden in [
            "credential",
            "prompt",
            "tool_output",
            "encrypted_content",
            "raw_cursor",
            "response_id",
        ] {
            assert!(
                !encoded.contains(forbidden),
                "leaked forbidden field {forbidden}"
            );
        }
    }
}
