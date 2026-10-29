use extension_contracts::*;

fn frame() -> ManagedEventHostFrame {
    let request = CommunicationCommandRequest {
        command_id: "acme/bot/status".into(),
        arguments: "today".into(),
        channel_id: "channel".into(),
        conversation_id: "chat".into(),
        session_id: "session".into(),
        input_id: "input".into(),
    };
    ManagedEventHostFrame {
        protocol: MANAGED_EVENT_PROTOCOL_V2.into(),
        call_id: "command-call".into(),
        handler: "communication_command".into(),
        execution_identity: ManagedExecutionIdentity::new(
            ManagedInstallationId::new("installation").unwrap(),
            ManagedWorkspaceId::new("workspace").unwrap(),
            ContributionId::new("acme.bot.commands").unwrap(),
            ManagedArtifactFingerprint::from_bytes(b"artifact"),
            ManagedBindingFingerprint::from_bytes(b"binding"),
        ),
        generation: std::num::NonZeroU64::new(1).unwrap(),
        graph_fingerprint: "frozen".into(),
        authority_revision: 1,
        deadline_unix_ms: 10,
        delivery: ManagedEventDelivery {
            event_id: "event".into(),
            contract_id: COMMUNICATION_COMMAND_POINT.into(),
            contract_version: "1".into(),
            point_id: COMMUNICATION_COMMAND_POINT.into(),
            point_contract_version: "1".into(),
            schema: ManagedEventSchema {
                contract_id: COMMUNICATION_COMMAND_POINT.into(),
                contract_version: "1".into(),
                payload_schema: CommunicationCommandRequest::event_schema(),
            },
            workspace_id: "workspace".into(),
            causation_id: "input".into(),
            correlation_id: "input".into(),
            payload: serde_json::to_value(request).unwrap(),
        },
    }
}

#[test]
fn communication_command_sdk_typed_handler_returns_bounded_destination_free_reply() {
    let frame = frame();
    let mut output = Vec::new();
    crate::serve_communication_command(
        serde_json::to_vec(&frame).unwrap().as_slice(),
        &mut output,
        |request| {
            assert_eq!(request.arguments, "today");
            CommunicationCommandReply {
                text: "Status ready".into(),
            }
        },
    )
    .unwrap();
    let response: ManagedEventWorkerFrame = serde_json::from_slice(&output).unwrap();
    response.validate_for(&frame).unwrap();
    assert!(
        matches!(&response.result,ManagedEventOutcome::CommandReply { reply } if reply.text=="Status ready")
    );
    for field in ["workspace_id", "actor_id", "recipient", "channel_id"] {
        let mut forged = serde_json::to_value(&response).unwrap();
        forged["result"]["reply"][field] = "forged".into();
        assert!(serde_json::from_value::<ManagedEventWorkerFrame>(forged).is_err());
    }
    let mut oversized = response.clone();
    oversized.result = ManagedEventOutcome::CommandReply {
        reply: CommunicationCommandReply {
            text: "x".repeat(32_769),
        },
    };
    assert!(oversized.validate_for(&frame).is_err());
    let mut empty = response.clone();
    empty.result = ManagedEventOutcome::CommandReply {
        reply: CommunicationCommandReply { text: " ".into() },
    };
    assert!(empty.validate_for(&frame).is_err());
    let mut ack = response;
    ack.result = ManagedEventOutcome::Acknowledged;
    assert!(
        ack.validate_for(&frame).is_err(),
        "ACK cannot replace durable command reply"
    );
}

#[test]
fn communication_command_sdk_renders_structured_parameters_before_writing_wire() {
    use CommunicationMessageBlock::{BulletList, Paragraph};
    use CommunicationMessageInline::{Code, Strong, Text};
    let frame = frame();
    let mut output = Vec::new();
    crate::serve_communication_command(
        serde_json::to_vec(&frame).unwrap().as_slice(),
        &mut output,
        |_| {
            CommunicationMessageContent::Blocks(vec![
                Paragraph(vec![Strong("Available commands".into())]),
                BulletList(vec![
                    vec![
                        Code("/help".into()),
                        Text(" — Show available commands.".into()),
                    ],
                    vec![Code("/model".into()), Text(" — Select a model.".into())],
                ]),
            ])
        },
    )
    .unwrap();
    let response: ManagedEventWorkerFrame = serde_json::from_slice(&output).unwrap();
    response.validate_for(&frame).unwrap();
    let ManagedEventOutcome::CommandReply { reply } = response.result else {
        panic!("missing reply")
    };
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../extension-contracts/src/_tests/notification/message_content_fixtures.json"
    ))
    .unwrap();
    assert_eq!(
        reply.text,
        expected["structured_commands"].as_str().unwrap()
    );
    assert_eq!(
        serde_json::to_value(reply)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn communication_command_sdk_preserves_authored_markdown_and_checks_rendered_size() {
    let frame = serde_json::to_vec(&frame()).unwrap();
    let markdown = "**Status**\n\n- ready\n\n```text\n*literal*\n```";
    let mut output = Vec::new();
    crate::serve_communication_command(frame.as_slice(), &mut output, |_| {
        CommunicationCommandReply {
            text: markdown.into(),
        }
    })
    .unwrap();
    let response: ManagedEventWorkerFrame = serde_json::from_slice(&output).unwrap();
    assert!(
        matches!(response.result, ManagedEventOutcome::CommandReply{reply} if reply.text == markdown)
    );

    output.clear();
    crate::serve_communication_command(frame.as_slice(), &mut output, |_| "first\nsecond").unwrap();
    let response: ManagedEventWorkerFrame = serde_json::from_slice(&output).unwrap();
    assert!(
        matches!(response.result, ManagedEventOutcome::CommandReply{reply} if reply.text == "first\\\nsecond")
    );
    output.clear();
    assert!(
        crate::serve_communication_command(frame.as_slice(), &mut output, |_| {
            "*".repeat(COMMUNICATION_COMMAND_REPLY_MAX_BYTES / 2 + 1)
        })
        .is_err()
    );
    assert!(
        output.is_empty(),
        "an invalid rendered reply must not emit a partial frame"
    );
}
