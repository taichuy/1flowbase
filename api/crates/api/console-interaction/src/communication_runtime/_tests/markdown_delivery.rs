//! The command receipt's completed Markdown must reach the real supplier worker unchanged.
use super::*;
use control_plane::{
    notifications::{BuiltinCommunicationIntent, CommunicationCommandIntent},
    ports::NotificationLogListInput,
};
use domain::{CommunicationConversationScope, CommunicationSendStatus};
use extension_contracts::{
    CommunicationCommandReply, CommunicationMessageBlock as Block,
    CommunicationMessageContent as Content, CommunicationMessageInline as Inline,
};
use futures_util::FutureExt;
use std::panic::AssertUnwindSafe;

#[tokio::test]
async fn communication_command_markdown_reaches_the_real_host_supplier_boundary_unchanged() {
    let expected = "**Status**\n\n- `/next [id]` — Ready\\\n  Second line";
    let fixture = Fixture::with_expected_sends(Some(json!([{
        "sender_id":"alice", "conversation_id":"external-room",
        "provider_message_id":"alice-input", "text":expected,
    }])))
    .await;
    let key = (fixture.actor.current_workspace_id, fixture.channel);
    let (signal, stop) = watch::channel(false);
    let mut worker = None;
    let outcome =
        AssertUnwindSafe(async {
            fixture.receive().await;
            let service = &fixture.state.notification_service;
            let pending = service
                .scan_pending_admissions(key.0, key.1, 100)
                .await
                .unwrap();
            let mut selected = None;
            for input in pending {
                let scope = CommunicationConversationScope {
                    workspace_id: key.0,
                    instance_id: key.1,
                    external_identity: input.conversation.external_identity,
                    conversation_id: input.conversation.id,
                };
                if scope.external_identity == "alice" {
                    selected = Some((scope, input.input_id));
                } else {
                    assert!(service.exclude_input(&scope, input.input_id).await.unwrap());
                }
            }
            let (scope, input) = selected.unwrap();
            // Start at durable handler completion. Plugin worker -> completion is covered
            // separately; this fixture exercises the actual Host send and supplier process.
            let command = service
                .enqueue_command(
                    &scope,
                    input,
                    CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Help),
                )
                .await
                .unwrap();
            let claim = service
                .claim_builtin_command(key.0, key.1, 60)
                .await
                .unwrap()
                .unwrap();
            let reply = CommunicationCommandReply::from_content(Content::Blocks(vec![
                Block::Paragraph(vec![Inline::Strong("Status".into())]),
                Block::BulletList(vec![vec![
                    Inline::Code("/next [id]".into()),
                    Inline::Text(" — Ready\nSecond line".into()),
                ]]),
            ]))
            .unwrap();
            assert_eq!(reply.text, expected);
            service
                .complete_builtin_command(
                    &claim,
                    Uuid::parse_str(&command.request.session_id).unwrap(),
                    reply,
                    None,
                )
                .await
                .unwrap();
            worker = Some(tokio::spawn(super::super::commands::run(
                fixture.state.clone(),
                key,
                stop,
            )));
            let sent = tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    let record = service
                        .command_for_input(&scope, input)
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(
                        !matches!(
                            record.send_status,
                            CommunicationSendStatus::Failed | CommunicationSendStatus::Unknown
                        ),
                        "{record:?}"
                    );
                    if record.send_status == CommunicationSendStatus::Succeeded {
                        break record;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect("real supplier must receive the exact target and Markdown");
            assert_eq!(sent.reply.unwrap().text, expected);
            let history: Vec<String> = sqlx::query_scalar(
            "SELECT content FROM communication_messages WHERE input_id=$1 AND direction='outbound'",
        ).bind(input).fetch_all(fixture.state.store.pool()).await.unwrap();
            assert_eq!(history, vec![expected.to_string()]);
            let logs = service
                .logs(
                    &fixture.actor,
                    NotificationLogListInput {
                        instance_id: Some(key.1),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            assert_eq!(
                logs.items.len(),
                1,
                "one supplier send for the completed receipt"
            );
            assert_eq!(logs.items[0].status, "success");
            assert_eq!(logs.items[0].actor_user_id, fixture.actor.user_id);
        })
        .catch_unwind()
        .await;
    signal.send_replace(true);
    let joined = if let Some(mut worker) = worker {
        match tokio::time::timeout(Duration::from_secs(10), &mut worker).await {
            Ok(result) => result.is_ok(),
            Err(_) => {
                worker.abort();
                let _ = worker.await;
                false
            }
        }
    } else {
        true
    };
    let stopped = fixture.runtime.host.stop().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(joined, "command worker must stop cleanly");
    stopped.unwrap();
}
