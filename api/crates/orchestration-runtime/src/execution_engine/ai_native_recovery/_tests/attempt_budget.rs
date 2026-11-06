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

#[test]
fn validated_terminal_receipt_blocks_rotation_even_with_unspent_budget() {
    let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
        100,
        1,
        true,
        RecoveryInputMode::NativeOpaque,
        4,
    )
    .unwrap();
    let mut last = receipt(&ledger, 0);
    last.commit_level = CommitLevel::Terminal;
    last.disposition = RecoveryDisposition::TerminalInterruption;
    assert_eq!(decide(&mut ledger, Some(&last)).total_attempts_charged, 1);
    assert!(!ledger.allows_configured_replay_at(1));
    assert!(ledger.rotate_epoch_for_configured_replay().is_err());
}

#[test]
fn selected_representation_changes_policy_without_replenishing_budget_or_commit() {
    let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
        100,
        2,
        false,
        RecoveryInputMode::NativeOpaque,
        3,
    )
    .unwrap();
    let epoch = ledger.current_epoch();
    ledger.select_input_representation(RecoveryInputMode::SemanticMapped, true);
    assert!(matches!(
        ledger.provider_directive().policy,
        RecoveryPolicy::SemanticMapped { .. }
    ));
    assert!(ledger.allows_configured_replay_at(1));
    let provider_receipt = receipt(&ledger, 0);
    let outcome = decide(&mut ledger, Some(&provider_receipt));
    assert_eq!(outcome.total_attempts_charged, 1);
    ledger.select_input_representation(RecoveryInputMode::NativeOpaque, true);
    assert!(matches!(
        ledger.provider_directive().policy,
        RecoveryPolicy::NativeOpaque { .. }
    ));
    assert!(!ledger.allows_configured_replay_at(1));
    assert_eq!(
        ledger
            .provider_directive()
            .policy
            .budget()
            .max_inner_attempts,
        2
    );
    ledger.semantic_committed = true;
    ledger.select_input_representation(RecoveryInputMode::SemanticMapped, true);
    assert!(ledger.semantic_replay_blocked());
    assert!(!ledger.allows_configured_replay_at(1));
    assert_ne!(ledger.current_epoch(), epoch);
}

#[test]
fn native_exact_authorization_preserves_opaque_policy_and_only_allows_transport_replay() {
    let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
        100,
        2,
        false,
        RecoveryInputMode::NativeOpaque,
        3,
    )
    .unwrap();
    let epoch = ledger.current_epoch();
    ledger.select_input_representation(RecoveryInputMode::NativeOpaque, true);
    let denied = receipt(&ledger, 0);
    assert_eq!(
        decide(&mut ledger, Some(&denied)).decision,
        OuterReplayDecision::NonReproducible
    );
    assert_eq!(ledger.total_attempts_charged, 1);
    ledger.authorize_native_exact_transport_replay();
    assert_eq!(
        ledger.current_epoch(),
        epoch,
        "authorization cannot rotate epochs"
    );
    assert_eq!(
        ledger.total_attempts_charged, 1,
        "authorization cannot clear charges"
    );
    assert_eq!(ledger.total_attempt_budget, 3);
    assert_eq!(ledger.absolute_deadline_unix_ms(), 100);
    assert!(matches!(
        ledger.provider_directive().policy,
        RecoveryPolicy::NativeOpaque { .. }
    ));
    assert!(!ledger.allows_configured_replay_at(1));
    let retry = receipt(&ledger, 0);
    assert_eq!(
        decide(&mut ledger, Some(&retry)).decision,
        OuterReplayDecision::Retry
    );
    assert_eq!(ledger.total_attempts_charged, 2);
    // Selecting the next representation revokes the previous snapshot's authorization.
    ledger.select_input_representation(RecoveryInputMode::NativeOpaque, true);
    assert!(!ledger.reproducible);
    ledger.authorize_native_exact_transport_replay();
    let exhausted = receipt(&ledger, 0);
    assert_eq!(
        decide(&mut ledger, Some(&exhausted)).decision,
        OuterReplayDecision::AttemptBudgetExhausted
    );
    assert_eq!(ledger.total_attempts_charged, 3);
}

#[test]
fn native_exact_authorization_does_not_erase_commit_terminal_or_deadline_gates() {
    for commit in [CommitLevel::SemanticCommitted, CommitLevel::Terminal] {
        let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
            100,
            2,
            false,
            RecoveryInputMode::NativeOpaque,
            4,
        )
        .unwrap();
        ledger.select_input_representation(RecoveryInputMode::NativeOpaque, false);
        ledger.authorize_native_exact_transport_replay();
        let mut last = receipt(&ledger, 0);
        last.commit_level = commit;
        last.disposition = if commit == CommitLevel::Terminal {
            RecoveryDisposition::TerminalInterruption
        } else {
            RecoveryDisposition::SameEpochReconnect
        };
        let outcome = decide(&mut ledger, Some(&last));
        assert!(matches!(
            outcome.decision,
            OuterReplayDecision::SemanticCommitted | OuterReplayDecision::SemanticTerminal
        ));
        let epoch = ledger.current_epoch();
        ledger.authorize_native_exact_transport_replay();
        assert!(ledger.semantic_replay_blocked());
        assert_eq!(ledger.current_epoch(), epoch);
        assert_eq!(ledger.total_attempts_charged, 1);
        assert!(!ledger.allows_configured_replay_at(1));
        assert!(ledger.rotate_epoch_for_configured_replay().is_err());
    }
    let mut expired = AiNativeRecoveryLedger::with_total_attempt_budget(
        1,
        2,
        false,
        RecoveryInputMode::NativeOpaque,
        4,
    )
    .unwrap();
    expired.authorize_native_exact_transport_replay();
    let last = receipt(&expired, 0);
    assert_eq!(
        decide(&mut expired, Some(&last)).decision,
        OuterReplayDecision::DeadlineExceeded
    );
    let mut semantic = AiNativeRecoveryLedger::with_total_attempt_budget(
        100,
        2,
        false,
        RecoveryInputMode::SemanticMapped,
        4,
    )
    .unwrap();
    semantic.authorize_native_exact_transport_replay();
    assert!(
        !semantic.reproducible,
        "native grant cannot authorize an unrelated semantic input"
    );
}

#[test]
fn native_owner_loss_admission_preserves_deadline_total_budget_and_semantic_barriers() {
    use extension_contracts::provider_contract::{RecoveryReason, RecoveryTransport};
    for case in ["deadline", "budget", "semantic", "generic"] {
        let mut ledger = AiNativeRecoveryLedger::with_total_attempt_budget(
            100,
            1,
            false,
            RecoveryInputMode::NativeOpaque,
            if case == "budget" { 1 } else { 4 },
        )
        .unwrap();
        let receipt = ProviderRecoveryReceipt {
            attempt: 0,
            transport: RecoveryTransport::AiNativeWebSocket,
            transport_epoch: ledger.current_epoch(),
            socket_incarnation: None,
            commit_level: CommitLevel::LifecycleOnly,
            disposition: RecoveryDisposition::LogicalInvocationRetry,
            reason: if case == "generic" {
                RecoveryReason::TransportDisconnected
            } else {
                RecoveryReason::ContinuationOwnerUnavailable
            },
        };
        if case == "semantic" {
            ledger.observe_events(&[ProviderStreamEvent::TextDelta {
                delta: "committed".into(),
            }]);
        }
        let partition =
            ProviderRetryPartition::new(format!("owner-loss-{case}"), "endpoint", "credential")
                .unwrap();
        let before = ledger.current_epoch();
        let result = decide_native_owner_replay_with_default_bucket(
            &mut ledger,
            &receipt,
            partition,
            if case == "deadline" { 100 } else { 1 },
            0,
        );
        assert_ne!(result.decision, OuterReplayDecision::Retry, "{case}");
        assert_eq!(ledger.current_epoch(), before, "denial cannot rotate epoch");
        assert_eq!(
            result.total_attempts_charged, 1,
            "debit actual attempt exactly once"
        );
        assert_eq!(ledger.absolute_deadline_unix_ms(), 100);
    }
}
