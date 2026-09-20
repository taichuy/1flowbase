use super::*;
use extension_contracts::provider_contract::{
    RecoveryReason, RecoveryTransport, SocketIncarnation,
};

fn receipt(ledger: &AiNativeRecoveryLedger, attempt: u16) -> ProviderRecoveryReceipt {
    ProviderRecoveryReceipt {
        attempt,
        transport: RecoveryTransport::AiNativeWebSocket,
        transport_epoch: ledger.current_epoch(),
        socket_incarnation: Some(SocketIncarnation::new(1).unwrap()),
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::LogicalInvocationRetry,
        reason: RecoveryReason::TransportDisconnected,
    }
}
fn decide(
    ledger: &mut AiNativeRecoveryLedger,
    receipt: Option<&ProviderRecoveryReceipt>,
) -> AiNativeRecoveryReceipt {
    ledger.decide_outer_replay(
        receipt,
        ProviderRetryPartition::new("budget", "endpoint", "owner").unwrap(),
        1,
        &mut PartitionedRetryTokenBucket::default(),
        0,
    )
}

#[test]
fn actual_attempt_count_exhausts_budget_one_and_two_without_reset() {
    for total in [1, 2] {
        let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
            100,
            8,
            true,
            RecoveryInputMode::NativeOpaque,
            total,
        )
        .unwrap();
        for consumed in 1..=total {
            assert_eq!(
                u32::from(
                    ledger
                        .provider_directive()
                        .policy
                        .budget()
                        .max_inner_attempts
                ),
                total - consumed + 1
            );
            let last = receipt(&ledger, 0);
            let result = decide(&mut ledger, Some(&last));
            assert_eq!(result.provider_attempts_consumed, Some(1));
            assert_eq!(result.total_attempts_charged, consumed);
            assert_eq!(result.total_attempt_budget, total);
            assert_eq!(
                result.decision,
                if consumed == total {
                    OuterReplayDecision::AttemptBudgetExhausted
                } else {
                    OuterReplayDecision::Retry
                }
            );
        }
        assert!(!ledger.allows_configured_replay_at(1));
        assert!(ledger.rotate_epoch_for_configured_replay().is_err());
    }
}

#[test]
fn configured_rotation_does_not_replenish_partial_allocation() {
    let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
        100,
        0,
        true,
        RecoveryInputMode::SemanticMapped,
        3,
    )
    .unwrap();
    let first = receipt(&ledger, 1);
    let result = decide(&mut ledger, Some(&first));
    assert_eq!(result.total_attempts_charged, 2);
    assert_eq!(
        result.provider_directive.policy.budget().max_inner_attempts,
        2
    );
    ledger.rotate_epoch_for_configured_replay().unwrap();
    assert_eq!(
        ledger
            .provider_directive()
            .policy
            .budget()
            .max_inner_attempts,
        1
    );
    assert_eq!(ledger.absolute_deadline_unix_ms(), 100);
    let last = receipt(&ledger, 0);
    assert_eq!(decide(&mut ledger, Some(&last)).total_attempts_charged, 3);
    assert!(ledger.rotate_epoch_for_configured_replay().is_err());
}

#[test]
fn unreported_attempts_charge_allocation_without_claiming_actual_count() {
    let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
        100,
        1,
        true,
        RecoveryInputMode::SemanticMapped,
        2,
    )
    .unwrap();
    let result = decide(&mut ledger, None);
    assert_eq!(result.provider_attempts_consumed, None);
    assert_eq!(result.total_attempts_charged, 2);
    assert!(!ledger.allows_configured_replay_at(1));
}

#[test]
fn legal_exhausted_terminal_is_accepted_but_overflow_is_not() {
    for total in [1, 2] {
        let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
            100,
            1,
            true,
            RecoveryInputMode::NativeOpaque,
            total,
        )
        .unwrap();
        let mut last = receipt(&ledger, total as u16 - 1);
        last.commit_level = CommitLevel::Terminal;
        last.disposition = RecoveryDisposition::TerminalInterruption;
        last.reason = RecoveryReason::BudgetExhausted;
        let result = decide(&mut ledger, Some(&last));
        assert_eq!(result.decision, OuterReplayDecision::SemanticTerminal);
        assert_eq!(result.total_attempts_charged, total);
        let mut other = AiNativeRecoveryLedger::with_total_attempt_budget(
            100,
            1,
            true,
            RecoveryInputMode::NativeOpaque,
            total,
        )
        .unwrap();
        last.transport_epoch = other.current_epoch();
        last.attempt += 1;
        let result = decide(&mut other, Some(&last));
        assert_eq!(result.decision, OuterReplayDecision::InvalidReceipt);
        assert_eq!(result.provider_attempts_consumed, None);
        assert_eq!(result.total_attempts_charged, total);
    }
}
