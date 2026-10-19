use super::*;

#[test]
fn b2_transport_session_contract_is_typed_bounded_and_deny_unknown() {
    let directive = ProviderTransportSessionDirective {
        worker_incarnation: None,
        logical_session_id: "logical_01".into(),
        generation: 41,
        task_id: "task_01".into(),
        state: ProviderLogicalSessionState::Waiting,
        physical_deadline_unix_ms: 1_800_000_000_000,
    };
    let mut input = ProviderInvocationInput::default();
    input
        .set_transport_session_directive(directive.clone())
        .unwrap();
    assert_eq!(
        input.transport_session_directive().unwrap(),
        Some(directive)
    );
    assert!(input.run_context[PROVIDER_TRANSPORT_SESSION_CONTEXT_KEY]
        .get("provider_config")
        .is_none());

    let unknown = json!({
        "logical_session_id": "logical_01",
        "generation": 41,
        "task_id": "task_01",
        "state": "idle",
        "physical_deadline_unix_ms": 1_800_000_000_000_i64,
        "session_key": "must-not-cross"
    });
    assert!(serde_json::from_value::<ProviderTransportSessionDirective>(unknown).is_err());
    let missing_generation = json!({
        "logical_session_id": "logical_01",
        "task_id": "task_01",
        "state": "idle",
        "physical_deadline_unix_ms": 1_800_000_000_000_i64
    });
    assert!(
        serde_json::from_value::<ProviderTransportSessionDirective>(missing_generation).is_err()
    );
    assert!(ProviderTransportSessionCommand {
        worker_incarnation: None,
        logical_session_id: "contains a space".into(),
        generation: 1,
        action: ProviderTransportSessionAction::Drain,
        deadline_unix_ms: 1,
    }
    .validate()
    .is_err());
}

#[test]
fn b2_transport_session_receipt_is_safe_and_round_trips_through_metadata() {
    let receipt = ProviderTransportSessionReceipt {
        closure_evidence: None,
        generation: 7,
        reused: true,
        physical_state: ProviderPhysicalTransportState::Closed,
        connection_age_ms: 42,
        ttl_remaining_ms: 0,
        close_reason: Some(ProviderTransportSessionCloseReason::RequestedClose),
        close_acknowledged: Some(true),
    };
    let mut result = ProviderInvocationResult {
        provider_metadata: json!({}),
        ..ProviderInvocationResult::default()
    };
    result
        .set_transport_session_receipt(receipt.clone())
        .unwrap();
    assert_eq!(result.transport_session_receipt().unwrap(), Some(receipt));
    let value = &result.provider_metadata[PROVIDER_TRANSPORT_SESSION_RECEIPT_METADATA_KEY];
    assert!(value.get("logical_session_id").is_none());
    assert!(value.get("session_key").is_none());

    let request = ProviderStdioRequest {
        method: ProviderStdioMethod::TransportSession,
        input: serde_json::to_value(ProviderTransportSessionCommand {
            worker_incarnation: None,
            logical_session_id: "logical_01".into(),
            generation: 7,
            action: ProviderTransportSessionAction::Close,
            deadline_unix_ms: 1_800_000_000_000,
        })
        .unwrap(),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap()["method"],
        "transport_session"
    );
}

#[test]
fn transport_outcomes_keep_receipt_generation_fault_and_ack_domains_distinct() {
    let empty = ProviderInvocationResult {
        provider_metadata: json!({}),
        ..ProviderInvocationResult::default()
    };
    assert_eq!(
        empty.transport_outcome(7, None).unwrap(),
        ProviderInvocationTransportOutcome::ReceiptMissing
    );

    let mut stale = empty.clone();
    let stale_receipt = ProviderTransportSessionReceipt {
        closure_evidence: None,
        generation: 6,
        reused: true,
        physical_state: ProviderPhysicalTransportState::Ready,
        connection_age_ms: 42,
        ttl_remaining_ms: 100,
        close_reason: None,
        close_acknowledged: None,
    };
    stale.set_transport_session_receipt(stale_receipt).unwrap();
    assert_eq!(
        stale.transport_outcome(7, None).unwrap(),
        ProviderInvocationTransportOutcome::StaleGeneration {
            expected_generation: 7,
            received_generation: 6,
        }
    );

    let fault_receipt = ProviderTransportSessionReceipt {
        closure_evidence: None,
        generation: 7,
        reused: true,
        physical_state: ProviderPhysicalTransportState::Faulted,
        connection_age_ms: 42,
        ttl_remaining_ms: 0,
        close_reason: Some(ProviderTransportSessionCloseReason::TransportFault),
        close_acknowledged: None,
    };
    let mut fault = empty;
    fault
        .set_transport_session_receipt(fault_receipt.clone())
        .unwrap();
    assert_eq!(
        fault.transport_outcome(7, None).unwrap(),
        ProviderInvocationTransportOutcome::PhysicalConnectionFault {
            generation: 7,
            state: ProviderPhysicalTransportState::Faulted,
        }
    );
    assert_eq!(
        fault_receipt.lifecycle_ack_outcome(7).unwrap(),
        ProviderLifecycleAckOutcome::PhysicalConnectionFault {
            generation: 7,
            state: ProviderPhysicalTransportState::Faulted,
        }
    );

    let ack_missing = ProviderTransportSessionReceipt {
        closure_evidence: None,
        generation: 7,
        reused: true,
        physical_state: ProviderPhysicalTransportState::Closed,
        connection_age_ms: 42,
        ttl_remaining_ms: 0,
        close_reason: Some(ProviderTransportSessionCloseReason::RequestedClose),
        close_acknowledged: None,
    };
    assert_eq!(
        ack_missing.lifecycle_ack_outcome(7).unwrap(),
        ProviderLifecycleAckOutcome::AcknowledgementMissing
    );
    assert_eq!(
        ack_missing.lifecycle_ack_outcome(8).unwrap(),
        ProviderLifecycleAckOutcome::StaleGeneration {
            expected_generation: 8,
            received_generation: 7,
        }
    );
}

#[test]
fn legal_http_fallback_is_recovery_not_websocket_ack_or_max_age() {
    let epoch = TransportEpoch::new(17).unwrap();
    let directive = ProviderRecoveryDirective {
        policy: semantic_mapped_recovery_policy(),
        transport_epoch: epoch,
        initial_commit_level: CommitLevel::LifecycleOnly,
        cursor_provenance: Some(CursorProvenance::durable()),
    };
    let recovery = ProviderRecoveryReceipt {
        attempt: 0,
        transport: RecoveryTransport::ProviderHttp,
        transport_epoch: epoch,
        socket_incarnation: None,
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::PreCommitHttpFallback,
        reason: RecoveryReason::TransportDisconnected,
    };
    let mut result = ProviderInvocationResult {
        provider_metadata: json!({}),
        ..ProviderInvocationResult::default()
    };
    result.set_recovery_receipt(recovery.clone()).unwrap();

    assert_eq!(
        result.transport_outcome(7, Some(&directive)).unwrap(),
        ProviderInvocationTransportOutcome::HttpFallback { recovery }
    );
    assert!(result.transport_session_receipt().unwrap().is_none());
    assert!(result.transport_outcome(7, None).is_err());
}

#[test]
fn c2_provider_timing_receipt_is_bounded_typed_and_contains_no_payload() {
    let receipt = ProviderInvocationTimingReceipt {
        schema_version: PROVIDER_INVOCATION_TIMING_SCHEMA_VERSION,
        connect_ms: Some(18),
        upstream_ms: 243,
        termination_kind: ProviderInvocationTerminationKind::Completed,
    };
    let mut result = ProviderInvocationResult {
        provider_metadata: json!({}),
        ..ProviderInvocationResult::default()
    };

    result
        .set_invocation_timing_receipt(receipt.clone())
        .unwrap();

    assert_eq!(result.invocation_timing_receipt().unwrap(), Some(receipt));
    let serialized = serde_json::to_string(&result.provider_metadata).unwrap();
    for forbidden in [
        "api_key",
        "prompt",
        "tool_output",
        "encrypted_content",
        "previous_response_id",
        "response_id",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}

fn semantic_mapped_recovery_policy() -> RecoveryPolicy {
    RecoveryPolicy::SemanticMapped {
        budget: RecoveryBudget {
            max_inner_attempts: 3,
            absolute_deadline_unix_ms: 1_800_000_000_000,
        },
    }
}

#[test]
fn d1_recovery_directive_round_trips_without_changing_the_legacy_directive() {
    let epoch = TransportEpoch::new(17).unwrap();
    let incarnation = SocketIncarnation::new(4).unwrap();
    let directive = ProviderRecoveryDirective {
        policy: semantic_mapped_recovery_policy(),
        transport_epoch: epoch,
        initial_commit_level: CommitLevel::LifecycleOnly,
        cursor_provenance: Some(CursorProvenance::connection_bound(epoch, incarnation)),
    };
    let legacy = ProviderTransportSessionDirective {
        worker_incarnation: None,
        logical_session_id: "logical_01".into(),
        generation: 41,
        task_id: "task_01".into(),
        state: ProviderLogicalSessionState::Waiting,
        physical_deadline_unix_ms: 1_800_000_000_000,
    };
    let mut input = ProviderInvocationInput::default();
    input
        .set_transport_session_directive(legacy.clone())
        .unwrap();
    input.set_recovery_directive(directive.clone()).unwrap();

    assert_eq!(input.recovery_directive().unwrap(), Some(directive));
    assert_eq!(input.transport_session_directive().unwrap(), Some(legacy));
    assert_eq!(
        input.run_context[PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY]["transport_epoch"],
        17
    );
    assert!(input.run_context[PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY]
        .get("socket_incarnation")
        .is_none());
    assert!(input.run_context[PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY]
        .get("disposition")
        .is_none());
    assert_eq!(
        input.run_context[PROVIDER_TRANSPORT_SESSION_CONTEXT_KEY]["generation"],
        41
    );
    assert!(serde_json::from_value::<TransportEpoch>(json!(0)).is_err());
    assert!(serde_json::from_value::<SocketIncarnation>(json!(0)).is_err());
}

#[test]
fn d1_recovery_validator_rejects_replay_after_commit_and_native_http_fallback() {
    let epoch = TransportEpoch::new(17).unwrap();
    for (disposition, transport) in [
        (
            RecoveryDisposition::PreCommitHttpFallback,
            RecoveryTransport::ProviderHttp,
        ),
        (
            RecoveryDisposition::OneFullContextRebuild,
            RecoveryTransport::AiNativeWebSocket,
        ),
        (
            RecoveryDisposition::LogicalInvocationRetry,
            RecoveryTransport::AiNativeWebSocket,
        ),
    ] {
        let receipt = ProviderRecoveryReceipt {
            attempt: 0,
            transport,
            transport_epoch: epoch,
            socket_incarnation: (transport == RecoveryTransport::AiNativeWebSocket)
                .then(|| SocketIncarnation::new(1).unwrap()),
            commit_level: CommitLevel::SemanticCommitted,
            disposition,
            reason: RecoveryReason::TransportDisconnected,
        };
        assert!(receipt
            .validate()
            .unwrap_err()
            .contains("cannot select a replay"));
    }

    let native_directive = ProviderRecoveryDirective {
        policy: RecoveryPolicy::NativeOpaque {
            budget: RecoveryBudget {
                max_inner_attempts: 3,
                absolute_deadline_unix_ms: 1_800_000_000_000,
            },
        },
        transport_epoch: epoch,
        initial_commit_level: CommitLevel::LifecycleOnly,
        cursor_provenance: Some(CursorProvenance::durable()),
    };
    let native_http_fallback = ProviderRecoveryReceipt {
        attempt: 0,
        transport: RecoveryTransport::ProviderHttp,
        transport_epoch: epoch,
        socket_incarnation: None,
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::PreCommitHttpFallback,
        reason: RecoveryReason::TransportDisconnected,
    };
    assert!(native_http_fallback
        .validate_against(&native_directive)
        .unwrap_err()
        .contains("forbids provider HTTP fallback"));
    let fallback_on_wrong_transport = ProviderRecoveryReceipt {
        transport: RecoveryTransport::AiNativeWebSocket,
        socket_incarnation: Some(SocketIncarnation::new(1).unwrap()),
        ..native_http_fallback.clone()
    };
    assert!(fallback_on_wrong_transport
        .validate()
        .unwrap_err()
        .contains("requires provider HTTP transport"));

    let backward = ProviderRecoveryReceipt {
        attempt: 0,
        transport: RecoveryTransport::AiNativeWebSocket,
        transport_epoch: epoch,
        socket_incarnation: Some(SocketIncarnation::new(1).unwrap()),
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::SameEpochReconnect,
        reason: RecoveryReason::TransportDisconnected,
    };
    let committed_directive = ProviderRecoveryDirective {
        policy: semantic_mapped_recovery_policy(),
        transport_epoch: epoch,
        initial_commit_level: CommitLevel::SemanticCommitted,
        cursor_provenance: None,
    };
    assert!(backward
        .validate_against(&committed_directive)
        .unwrap_err()
        .contains("cannot move backwards"));
}

#[test]
fn d1_connection_bound_cursor_uses_provider_reported_epoch_and_incarnation() {
    let bound_epoch = TransportEpoch::new(8).unwrap();
    let bound_incarnation = SocketIncarnation::new(3).unwrap();
    let cross_epoch = ProviderRecoveryDirective {
        policy: semantic_mapped_recovery_policy(),
        transport_epoch: TransportEpoch::new(9).unwrap(),
        initial_commit_level: CommitLevel::LifecycleOnly,
        cursor_provenance: Some(CursorProvenance::connection_bound(
            bound_epoch,
            bound_incarnation,
        )),
    };
    assert!(cross_epoch.validate().unwrap_err().contains("cannot cross"));

    let directive = ProviderRecoveryDirective {
        policy: semantic_mapped_recovery_policy(),
        transport_epoch: bound_epoch,
        initial_commit_level: CommitLevel::LifecycleOnly,
        cursor_provenance: Some(CursorProvenance::connection_bound(
            bound_epoch,
            bound_incarnation,
        )),
    };
    let advanced_incarnation = ProviderRecoveryReceipt {
        attempt: 0,
        transport: RecoveryTransport::AiNativeWebSocket,
        transport_epoch: bound_epoch,
        socket_incarnation: Some(SocketIncarnation::new(4).unwrap()),
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::SameEpochReconnect,
        reason: RecoveryReason::TransportDisconnected,
    };
    advanced_incarnation.validate_against(&directive).unwrap();

    let stale_equal_incarnation = ProviderRecoveryReceipt {
        socket_incarnation: Some(bound_incarnation),
        ..advanced_incarnation.clone()
    };
    assert!(stale_equal_incarnation
        .validate_against(&directive)
        .unwrap_err()
        .contains("must advance"));

    let stale_lower_incarnation = ProviderRecoveryReceipt {
        socket_incarnation: Some(SocketIncarnation::new(2).unwrap()),
        ..advanced_incarnation.clone()
    };
    assert!(stale_lower_incarnation
        .validate_against(&directive)
        .unwrap_err()
        .contains("must advance"));

    let connection_bound_http_fallback = ProviderRecoveryReceipt {
        attempt: 0,
        transport: RecoveryTransport::ProviderHttp,
        transport_epoch: bound_epoch,
        socket_incarnation: None,
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::PreCommitHttpFallback,
        reason: RecoveryReason::TransportDisconnected,
    };
    assert!(connection_bound_http_fallback
        .validate_against(&directive)
        .unwrap_err()
        .contains("cannot use provider HTTP fallback"));

    let rebuilt_on_new_socket = ProviderRecoveryReceipt {
        disposition: RecoveryDisposition::OneFullContextRebuild,
        ..advanced_incarnation
    };
    rebuilt_on_new_socket.validate_against(&directive).unwrap();
}

#[test]
fn d1_recovery_receipt_is_metadata_only_and_round_trips() {
    let receipt = ProviderRecoveryReceipt {
        attempt: 2,
        transport: RecoveryTransport::AiNativeWebSocket,
        transport_epoch: TransportEpoch::new(17).unwrap(),
        socket_incarnation: Some(SocketIncarnation::new(4).unwrap()),
        commit_level: CommitLevel::Terminal,
        disposition: RecoveryDisposition::SemanticTerminal,
        reason: RecoveryReason::SemanticCompleted,
    };
    let mut result = ProviderInvocationResult {
        provider_metadata: json!({}),
        ..ProviderInvocationResult::default()
    };
    result.set_recovery_receipt(receipt.clone()).unwrap();
    assert_eq!(result.recovery_receipt().unwrap(), Some(receipt));

    let value = &result.provider_metadata[PROVIDER_RECOVERY_RECEIPT_METADATA_KEY];
    assert_eq!(value["attempt"], 2);
    assert_eq!(value["transport_epoch"], 17);
    let encoded = serde_json::to_string(value).unwrap();
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

    let unknown_payload = json!({
        "attempt": 2,
        "transport": "ai_native_web_socket",
        "transport_epoch": 17,
        "socket_incarnation": 4,
        "commit_level": "terminal",
        "disposition": "semantic_terminal",
        "reason": "semantic_completed",
        "raw_cursor": "must-not-cross"
    });
    assert!(serde_json::from_value::<ProviderRecoveryReceipt>(unknown_payload).is_err());

    let capped_directive = ProviderRecoveryDirective {
        policy: RecoveryPolicy::SemanticMapped {
            budget: RecoveryBudget {
                max_inner_attempts: 2,
                absolute_deadline_unix_ms: 1_800_000_000_000,
            },
        },
        transport_epoch: TransportEpoch::new(17).unwrap(),
        initial_commit_level: CommitLevel::LifecycleOnly,
        cursor_provenance: None,
    };
    assert!(result
        .recovery_receipt()
        .unwrap()
        .unwrap()
        .validate_against(&capped_directive)
        .unwrap_err()
        .contains("exceeds its directive budget"));
}

mod transport_outcome_mode_tests {
    use super::*;
    use extension_contracts::provider_contract::{
        ProviderFinishReason, ProviderInvocationTransportClassification as Classification,
    };

    fn result() -> ProviderInvocationResult {
        ProviderInvocationResult {
            finish_reason: Some(ProviderFinishReason::Stop),
            provider_metadata: json!({}),
            ..ProviderInvocationResult::default()
        }
    }

    fn directive() -> ProviderRecoveryDirective {
        ProviderRecoveryDirective {
            policy: semantic_mapped_recovery_policy(),
            transport_epoch: TransportEpoch::new(17).unwrap(),
            initial_commit_level: CommitLevel::LifecycleOnly,
            cursor_provenance: Some(CursorProvenance::durable()),
        }
    }

    fn fallback() -> ProviderRecoveryReceipt {
        ProviderRecoveryReceipt {
            attempt: 0,
            transport: RecoveryTransport::ProviderHttp,
            transport_epoch: TransportEpoch::new(17).unwrap(),
            socket_incarnation: None,
            commit_level: CommitLevel::LifecycleOnly,
            disposition: RecoveryDisposition::PreCommitHttpFallback,
            reason: RecoveryReason::TransportDisconnected,
        }
    }

    fn ready() -> ProviderTransportSessionReceipt {
        ProviderTransportSessionReceipt {
            closure_evidence: None,
            generation: 7,
            reused: false,
            physical_state: ProviderPhysicalTransportState::Ready,
            connection_age_ms: 0,
            ttl_remaining_ms: 100,
            close_reason: None,
            close_acknowledged: None,
        }
    }

    #[test]
    fn direct_http_does_not_require_or_claim_websocket_receipts() {
        let mut result = result();
        assert_eq!(
            result
                .transport_outcome_for_mode(RecoveryTransport::ProviderHttp, 0, None)
                .unwrap(),
            Classification::Http
        );
        assert_eq!(
            result
                .transport_outcome_for_mode(RecoveryTransport::AiNativeWebSocket, 7, None)
                .unwrap(),
            Classification::Session(ProviderInvocationTransportOutcome::ReceiptMissing)
        );
        assert!(result.transport_outcome(0, None).is_err());
        result.set_transport_session_receipt(ready()).unwrap();
        assert!(result
            .transport_outcome_for_mode(RecoveryTransport::ProviderHttp, 0, None)
            .is_err());
        assert_eq!(
            result.transport_outcome(7, None).unwrap(),
            ProviderInvocationTransportOutcome::Ready { receipt: ready() }
        );
        assert!(matches!(
            result.transport_outcome(8, None).unwrap(),
            ProviderInvocationTransportOutcome::StaleGeneration { .. }
        ));
    }

    #[test]
    fn fallback_requires_mode_authorization_epoch_budget_and_reconstructible_cursor() {
        let mut result = result();
        result.set_recovery_receipt(fallback()).unwrap();
        assert_eq!(
            result.transport_outcome(7, Some(&directive())).unwrap(),
            ProviderInvocationTransportOutcome::HttpFallback {
                recovery: fallback()
            }
        );
        assert!(result.transport_outcome(7, None).is_err());
        assert!(result
            .transport_outcome_for_mode(RecoveryTransport::ProviderHttp, 0, Some(&directive()))
            .is_err());
        let mut invalid = directive();
        invalid.transport_epoch = TransportEpoch::new(18).unwrap();
        assert!(result.transport_outcome(7, Some(&invalid)).is_err());
        invalid = directive();
        invalid.policy = RecoveryPolicy::NativeOpaque {
            budget: *invalid.policy.budget(),
        };
        assert!(result.transport_outcome(7, Some(&invalid)).is_err());
        invalid = directive();
        invalid.cursor_provenance = Some(CursorProvenance::connection_bound(
            invalid.transport_epoch,
            SocketIncarnation::new(1).unwrap(),
        ));
        assert!(result.transport_outcome(7, Some(&invalid)).is_err());
        invalid = directive();
        invalid.initial_commit_level = CommitLevel::SemanticCommitted;
        assert!(result.transport_outcome(7, Some(&invalid)).is_err());
        let mut exhausted = fallback();
        exhausted.attempt = directive().policy.budget().max_inner_attempts;
        // A structurally valid receipt can still exceed its invocation's budget.
        result.set_recovery_receipt(exhausted).unwrap();
        assert!(result.transport_outcome(7, Some(&directive())).is_err());
        result.set_recovery_receipt(fallback()).unwrap();
        let mut stale = ready();
        stale.generation = 6;
        result.set_transport_session_receipt(stale).unwrap();
        assert!(matches!(
            result.transport_outcome(7, Some(&directive())).unwrap(),
            ProviderInvocationTransportOutcome::StaleGeneration { .. }
        ));
    }

    fn native_directive() -> ProviderRecoveryDirective {
        let mut directive = directive();
        directive.policy = RecoveryPolicy::NativeOpaque {
            budget: *directive.policy.budget(),
        };
        directive.cursor_provenance = None;
        directive
    }

    fn full_context_http_rebuild() -> ProviderRecoveryReceipt {
        ProviderRecoveryReceipt {
            attempt: 2,
            disposition: RecoveryDisposition::OneFullContextRebuild,
            ..fallback()
        }
    }

    #[test]
    fn native_full_context_http_rebuild_is_authorized_websocket_recovery() {
        let receipt = full_context_http_rebuild();
        for reason in [
            RecoveryReason::TransportDisconnected,
            RecoveryReason::TransportRejected,
        ] {
            let receipt = ProviderRecoveryReceipt {
                reason,
                ..receipt.clone()
            };
            receipt.validate_against(&native_directive()).unwrap();
            assert_eq!(receipt.consumed_attempts().unwrap(), 3);
            let mut result = result();
            result.set_recovery_receipt(receipt.clone()).unwrap();
            assert_eq!(
                result
                    .transport_outcome(7, Some(&native_directive()))
                    .unwrap(),
                ProviderInvocationTransportOutcome::HttpFallback { recovery: receipt }
            );
            assert!(result.transport_outcome(7, None).is_err());
            // A receipt describing a transport switch cannot authorize direct HTTP.
            assert!(result
                .transport_outcome_for_mode(
                    RecoveryTransport::ProviderHttp,
                    0,
                    Some(&native_directive()),
                )
                .unwrap_err()
                .contains("cannot claim WebSocket recovery or fallback"));
        }
        let mut durable = native_directive();
        durable.cursor_provenance = Some(CursorProvenance::durable());
        receipt.validate_against(&durable).unwrap();
        let earliest = ProviderRecoveryReceipt {
            attempt: 1,
            ..receipt
        };
        earliest.validate_against(&native_directive()).unwrap();
    }

    #[test]
    fn http_full_context_rebuild_rejects_invalid_transition_facts() {
        let receipt = full_context_http_rebuild();
        let mut invalid = receipt.clone();
        invalid.attempt = 0;
        assert!(invalid
            .validate()
            .unwrap_err()
            .contains("requires a recovery attempt"));
        for reason in [
            RecoveryReason::ProtocolError,
            RecoveryReason::DeadlineExceeded,
            RecoveryReason::FencingRejected,
            RecoveryReason::BudgetExhausted,
            RecoveryReason::SemanticCompleted,
            RecoveryReason::SemanticFailed,
        ] {
            invalid = receipt.clone();
            invalid.reason = reason;
            assert!(invalid
                .validate()
                .unwrap_err()
                .contains("transport recovery reason"));
        }
        invalid = receipt.clone();
        invalid.socket_incarnation = Some(SocketIncarnation::new(1).unwrap());
        assert!(invalid
            .validate()
            .unwrap_err()
            .contains("cannot claim a socket"));
        for commit_level in [CommitLevel::SemanticCommitted, CommitLevel::Terminal] {
            invalid = receipt.clone();
            invalid.commit_level = commit_level;
            assert!(invalid
                .validate()
                .unwrap_err()
                .contains("cannot select a replay"));
        }
    }

    #[test]
    fn native_http_rebuild_preserves_host_cursor_epoch_budget_and_commit_fences() {
        let receipt = full_context_http_rebuild();
        let mut invalid = native_directive();
        invalid.cursor_provenance = Some(CursorProvenance::connection_bound(
            invalid.transport_epoch,
            SocketIncarnation::new(1).unwrap(),
        ));
        assert!(receipt
            .validate_against(&invalid)
            .unwrap_err()
            .contains("connection-bound cursor cannot use provider HTTP fallback"));
        invalid = native_directive();
        invalid.transport_epoch = TransportEpoch::new(18).unwrap();
        assert!(receipt
            .validate_against(&invalid)
            .unwrap_err()
            .contains("cannot cross"));
        invalid = native_directive();
        invalid.policy = RecoveryPolicy::NativeOpaque {
            budget: RecoveryBudget {
                max_inner_attempts: receipt.attempt,
                ..*invalid.policy.budget()
            },
        };
        assert!(receipt
            .validate_against(&invalid)
            .unwrap_err()
            .contains("exceeds"));
        invalid = native_directive();
        invalid.initial_commit_level = CommitLevel::SemanticCommitted;
        assert!(receipt
            .validate_against(&invalid)
            .unwrap_err()
            .contains("cannot move backwards"));

        let mut result = result();
        result.set_recovery_receipt(receipt).unwrap();
        let mut stale = ready();
        stale.generation = 6;
        result.set_transport_session_receipt(stale).unwrap();
        assert!(matches!(
            result
                .transport_outcome(7, Some(&native_directive()))
                .unwrap(),
            ProviderInvocationTransportOutcome::StaleGeneration { .. }
        ));
    }

    #[test]
    fn native_direct_http_semantic_terminal_remains_legal() {
        let receipt = ProviderRecoveryReceipt {
            attempt: 0,
            commit_level: CommitLevel::Terminal,
            disposition: RecoveryDisposition::SemanticTerminal,
            reason: RecoveryReason::SemanticCompleted,
            ..fallback()
        };
        let mut result = result();
        result.set_recovery_receipt(receipt).unwrap();
        assert_eq!(
            result
                .transport_outcome_for_mode(
                    RecoveryTransport::ProviderHttp,
                    0,
                    Some(&native_directive()),
                )
                .unwrap(),
            Classification::Http
        );
        assert!(result
            .transport_outcome(7, Some(&native_directive()))
            .is_err());
    }

    #[test]
    fn failed_results_and_timing_never_classify_as_success() {
        for transport in [
            RecoveryTransport::ProviderHttp,
            RecoveryTransport::AiNativeWebSocket,
        ] {
            let mut failed = result();
            failed.finish_reason = Some(ProviderFinishReason::Error);
            assert!(failed
                .transport_outcome_for_mode(transport, 7, None)
                .is_err());
            for termination_kind in [
                ProviderInvocationTerminationKind::UpstreamError,
                ProviderInvocationTerminationKind::TransportError,
                ProviderInvocationTerminationKind::Deadline,
            ] {
                let mut failed = result();
                failed
                    .set_invocation_timing_receipt(ProviderInvocationTimingReceipt {
                        schema_version: PROVIDER_INVOCATION_TIMING_SCHEMA_VERSION,
                        connect_ms: None,
                        upstream_ms: 1,
                        termination_kind,
                    })
                    .unwrap();
                assert!(failed
                    .transport_outcome_for_mode(transport, 7, None)
                    .is_err());
            }
        }
        let mut failed = result();
        failed.finish_reason = Some(ProviderFinishReason::Error);
        failed.set_recovery_receipt(fallback()).unwrap();
        assert!(failed.transport_outcome(7, Some(&directive())).is_err());
    }

    #[test]
    fn terminal_recovery_failure_and_missing_fallback_cannot_masquerade_as_success() {
        let mut result = result();
        let mut recovery = fallback();
        recovery.commit_level = CommitLevel::Terminal;
        recovery.disposition = RecoveryDisposition::TerminalInterruption;
        recovery.reason = RecoveryReason::SemanticFailed;
        result.set_recovery_receipt(recovery.clone()).unwrap();
        assert!(result
            .transport_outcome_for_mode(RecoveryTransport::ProviderHttp, 0, Some(&directive()))
            .is_err());
        recovery.disposition = RecoveryDisposition::SemanticTerminal;
        result.set_recovery_receipt(recovery.clone()).unwrap();
        assert!(result
            .transport_outcome_for_mode(RecoveryTransport::ProviderHttp, 0, Some(&directive()))
            .is_err());
        for reason in [
            RecoveryReason::DeadlineExceeded,
            RecoveryReason::FencingRejected,
            RecoveryReason::BudgetExhausted,
        ] {
            recovery.reason = reason;
            result.set_recovery_receipt(recovery.clone()).unwrap();
            assert!(result
                .transport_outcome_for_mode(RecoveryTransport::ProviderHttp, 0, Some(&directive()))
                .is_err());
        }
        recovery.reason = RecoveryReason::SemanticCompleted;
        result.set_recovery_receipt(recovery).unwrap();
        assert_eq!(
            result
                .transport_outcome_for_mode(RecoveryTransport::ProviderHttp, 0, Some(&directive()))
                .unwrap(),
            Classification::Http
        );
        // HTTP success alone is not evidence of authorized WS -> HTTP fallback.
        result.set_transport_session_receipt(ready()).unwrap();
        assert!(result.transport_outcome(7, Some(&directive())).is_err());
    }
}

#[test]
fn closure_identity_and_physical_facts_are_independent_from_peer_ack() {
    let identity = ProviderTransportSessionIdentity {
        logical_session_id: "close-session".into(),
        generation: 9,
        worker_incarnation: 3,
    };
    let mut receipt = ProviderTransportSessionReceipt {
        generation: 9,
        reused: false,
        physical_state: ProviderPhysicalTransportState::Closed,
        connection_age_ms: 1,
        ttl_remaining_ms: 0,
        close_reason: Some(ProviderTransportSessionCloseReason::RequestedClose),
        close_acknowledged: Some(false),
        closure_evidence: Some(ProviderTransportClosureEvidence {
            source: ProviderTransportClosureSource::ProviderLocalRelease,
            no_ack_reason: Some(ProviderTransportNoAckReason::Timeout),
            identity: identity.clone(),
            local_released: true,
            peer_close_acknowledged: Some(false),
        }),
    };
    assert_eq!(
        receipt.closure_outcome(&identity).unwrap(),
        ProviderTransportClosureOutcome::Released {
            peer_close_acknowledged: Some(false)
        }
    );
    for wrong in [
        ProviderTransportSessionIdentity {
            logical_session_id: "other".into(),
            ..identity.clone()
        },
        ProviderTransportSessionIdentity {
            generation: 10,
            ..identity.clone()
        },
        ProviderTransportSessionIdentity {
            worker_incarnation: 4,
            ..identity.clone()
        },
    ] {
        assert_eq!(
            receipt.closure_outcome(&wrong).unwrap(),
            ProviderTransportClosureOutcome::IdentityMismatch
        );
    }
    receipt.closure_evidence.as_mut().unwrap().local_released = false;
    assert_eq!(
        receipt.closure_outcome(&identity).unwrap(),
        ProviderTransportClosureOutcome::NotReleased
    );
    receipt.closure_evidence.as_mut().unwrap().local_released = true;
    let encoded = serde_json::to_value(&receipt).unwrap();
    assert_eq!(
        serde_json::from_value::<ProviderTransportSessionReceipt>(encoded.clone()).unwrap(),
        receipt
    );
    let mut unknown = encoded;
    unknown["closure_evidence"]["unsafe_payload"] = json!("not accepted");
    assert!(serde_json::from_value::<ProviderTransportSessionReceipt>(unknown).is_err());
    receipt.close_acknowledged = None;
    let evidence = receipt.closure_evidence.as_mut().unwrap();
    evidence.source = ProviderTransportClosureSource::ConfirmedWorkerExit;
    evidence.peer_close_acknowledged = None;
    evidence.no_ack_reason = Some(ProviderTransportNoAckReason::Unknown);
    assert_eq!(
        receipt.closure_outcome(&identity).unwrap(),
        ProviderTransportClosureOutcome::Released {
            peer_close_acknowledged: None
        }
    );
    receipt.close_acknowledged = Some(true);
    receipt
        .closure_evidence
        .as_mut()
        .unwrap()
        .peer_close_acknowledged = Some(true);
    assert!(
        receipt.validate().is_err(),
        "worker exit must not manufacture peer ACK"
    );
    receipt.closure_evidence = None;
    assert_eq!(
        receipt.closure_outcome(&identity).unwrap(),
        ProviderTransportClosureOutcome::EvidenceMissing
    );
}

#[test]
fn closure_evidence_rejects_contradictory_or_unbound_identity() {
    let mut receipt = ProviderTransportSessionReceipt {
        generation: 9,
        reused: false,
        physical_state: ProviderPhysicalTransportState::Closed,
        connection_age_ms: 0,
        ttl_remaining_ms: 0,
        close_reason: Some(ProviderTransportSessionCloseReason::RequestedClose),
        close_acknowledged: Some(true),
        closure_evidence: Some(ProviderTransportClosureEvidence {
            source: ProviderTransportClosureSource::ProviderLocalRelease,
            no_ack_reason: None,
            identity: ProviderTransportSessionIdentity {
                logical_session_id: "close-session".into(),
                generation: 9,
                worker_incarnation: 3,
            },
            local_released: true,
            peer_close_acknowledged: Some(true),
        }),
    };
    assert!(receipt.validate().is_ok());
    receipt.closure_evidence.as_mut().unwrap().local_released = false;
    assert!(receipt.validate().is_err());
    receipt.closure_evidence.as_mut().unwrap().local_released = true;
    receipt
        .closure_evidence
        .as_mut()
        .unwrap()
        .identity
        .worker_incarnation = 0;
    assert!(receipt.validate().is_err());
    receipt
        .closure_evidence
        .as_mut()
        .unwrap()
        .identity
        .worker_incarnation = 3;
    receipt
        .closure_evidence
        .as_mut()
        .unwrap()
        .identity
        .generation = 10;
    assert!(receipt.validate().is_err());
    receipt
        .closure_evidence
        .as_mut()
        .unwrap()
        .identity
        .generation = 9;
    receipt.physical_state = ProviderPhysicalTransportState::Ready;
    assert!(receipt.validate().is_err());
}

const RECOVERY_RECEIPT_WIRE_KEY: &str = "1flowbase_provider_recovery";

#[test]
fn recovery_receipt_is_parsed_from_the_shared_details_key_on_both_paths() {
    assert_eq!(
        RECOVERY_RECEIPT_WIRE_KEY,
        PROVIDER_RECOVERY_RECEIPT_METADATA_KEY
    );
    let epoch = TransportEpoch::new(149).unwrap();
    let receipt = ProviderRecoveryReceipt {
        attempt: 0,
        transport: RecoveryTransport::AiNativeWebSocket,
        transport_epoch: epoch,
        socket_incarnation: Some(SocketIncarnation::new(6).unwrap()),
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::SameEpochReconnect,
        reason: RecoveryReason::TransportDisconnected,
    };
    let details = json!({ RECOVERY_RECEIPT_WIRE_KEY: &receipt });
    assert_eq!(
        recovery_receipt_from_details(&details).unwrap(),
        Some(receipt.clone())
    );

    // The success path and the error path share exactly one parser.
    let mut result = ProviderInvocationResult {
        provider_metadata: json!({}),
        ..ProviderInvocationResult::default()
    };
    result.set_recovery_receipt(receipt.clone()).unwrap();
    assert_eq!(result.recovery_receipt().unwrap(), Some(receipt));

    // Absent, non-object and malformed containers stay distinguishable from a
    // present-but-corrupt receipt: only the latter is an error.
    assert_eq!(recovery_receipt_from_details(&json!({})).unwrap(), None);
    assert_eq!(recovery_receipt_from_details(&json!(null)).unwrap(), None);
    let corrupt = json!({
        RECOVERY_RECEIPT_WIRE_KEY: {
            "attempt": 0,
            "transport": "ai_native_websocket",
            "transport_epoch": 149,
            "socket_incarnation": 6,
            "commit_level": "lifecycle_only",
            "disposition": "same_epoch_reconnect"
        }
    });
    assert!(recovery_receipt_from_details(&corrupt).is_err());
}

#[test]
fn socketless_websocket_receipt_requires_no_physical_reconnect_claim() {
    let epoch = TransportEpoch::new(149).unwrap();
    // A failure observed before any socket existed reports the real terminal
    // outcome without fabricating an incarnation.
    let socketless = ProviderRecoveryReceipt {
        attempt: 0,
        transport: RecoveryTransport::AiNativeWebSocket,
        transport_epoch: epoch,
        socket_incarnation: None,
        commit_level: CommitLevel::Terminal,
        disposition: RecoveryDisposition::TerminalInterruption,
        reason: RecoveryReason::TransportDisconnected,
    };
    socketless.validate().unwrap();
    let details = json!({ RECOVERY_RECEIPT_WIRE_KEY: &socketless });
    assert!(recovery_receipt_from_details(&details).unwrap().is_some());

    // A logical retry starts a new invocation and makes no physical socket claim.
    let logical_retry = ProviderRecoveryReceipt {
        socket_incarnation: None,
        commit_level: CommitLevel::LifecycleOnly,
        disposition: RecoveryDisposition::LogicalInvocationRetry,
        ..socketless.clone()
    };
    logical_retry.validate().unwrap();
    logical_retry
        .validate_against(&ProviderRecoveryDirective {
            policy: RecoveryPolicy::NativeOpaque {
                budget: RecoveryBudget {
                    max_inner_attempts: 2,
                    absolute_deadline_unix_ms: 100,
                },
            },
            transport_epoch: epoch,
            initial_commit_level: CommitLevel::LifecycleOnly,
            cursor_provenance: None,
        })
        .unwrap();
    assert_eq!(
        recovery_receipt_from_details(&json!({ RECOVERY_RECEIPT_WIRE_KEY: &logical_retry }))
            .unwrap(),
        Some(logical_retry)
    );

    // Physical reconnect or rebuild still has to name its socket.
    for disposition in [
        RecoveryDisposition::SameEpochReconnect,
        RecoveryDisposition::OneFullContextRebuild,
    ] {
        let invalid = ProviderRecoveryReceipt {
            socket_incarnation: None,
            commit_level: CommitLevel::LifecycleOnly,
            disposition,
            ..socketless.clone()
        };
        assert!(invalid
            .validate()
            .unwrap_err()
            .contains("requires a socket incarnation"));
    }
}

#[test]
fn recovery_attempt_index_and_consumed_count_respect_total_budget() {
    for budget in [1, 2, 16] {
        let directive = ProviderRecoveryDirective {
            policy: RecoveryPolicy::NativeOpaque {
                budget: RecoveryBudget {
                    max_inner_attempts: budget,
                    absolute_deadline_unix_ms: 100,
                },
            },
            transport_epoch: TransportEpoch::new(1).unwrap(),
            initial_commit_level: CommitLevel::LifecycleOnly,
            cursor_provenance: None,
        };
        let mut receipt = ProviderRecoveryReceipt {
            attempt: budget - 1,
            transport: RecoveryTransport::AiNativeWebSocket,
            transport_epoch: directive.transport_epoch,
            socket_incarnation: None,
            commit_level: CommitLevel::Terminal,
            disposition: RecoveryDisposition::TerminalInterruption,
            reason: RecoveryReason::BudgetExhausted,
        };
        assert!(receipt.validate_against(&directive).is_ok());
        assert_eq!(receipt.consumed_attempts().unwrap(), budget);
        receipt.attempt = budget;
        assert!(receipt.validate_against(&directive).is_err());
    }
}
