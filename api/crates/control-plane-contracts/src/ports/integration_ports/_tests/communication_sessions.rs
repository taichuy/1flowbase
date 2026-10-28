use super::*;

fn turn() -> CommunicationReplyTurn {
    CommunicationReplyTurn {
        id: Uuid::now_v7(),
        conversation_id: Uuid::now_v7(),
        session_id: Uuid::now_v7(),
        session_epoch: 1,
        anchor_input_id: Uuid::now_v7(),
        state: CommunicationTurnState::Open,
        run_id: Some(Uuid::now_v7()),
        selected_model: None,
        received_watermark: 2,
        consumed_watermark: 2,
        checkpoint_revision: 3,
        processing_status: CommunicationProcessingStatus::Processing,
        send_status: CommunicationSendStatus::None,
        failure_code: None,
        outbound_message_id: None,
    }
}

#[test]
fn communication_final_candidate_requires_exact_durable_checkpoint_and_all_inputs() {
    let candidate = CommunicationTurnFinalCandidate {
        expected_received_watermark: 2,
        expected_checkpoint_revision: 3,
        final_reply: "complete".into(),
    };
    let current = turn();
    assert!(candidate.matches_checkpoint(&current));
    for stale in [
        CommunicationReplyTurn {
            received_watermark: 3,
            ..current.clone()
        },
        CommunicationReplyTurn {
            consumed_watermark: 1,
            ..current.clone()
        },
        CommunicationReplyTurn {
            checkpoint_revision: 4,
            ..current.clone()
        },
        CommunicationReplyTurn {
            checkpoint_revision: 2,
            ..current.clone()
        },
    ] {
        assert!(!candidate.matches_checkpoint(&stale));
    }
    let no_checkpoint = CommunicationTurnFinalCandidate {
        expected_checkpoint_revision: 0,
        ..candidate
    };
    assert!(!no_checkpoint.matches_checkpoint(&CommunicationReplyTurn {
        checkpoint_revision: 0,
        ..current
    }));
}
