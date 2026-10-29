use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{execute_notification, CapabilityStdioRequest, NotificationTypeDescriptor};
use serde_json::{json, Value};

fn descriptor() -> NotificationTypeDescriptor {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../plugins/fixtures/acme.notification/manifest.yaml"
    ))
    .unwrap();
    serde_json::from_value(manifest["notification_types"][0].clone()).unwrap()
}

fn request(text: Value) -> CapabilityStdioRequest {
    serde_json::from_value(json!({"method":"execute","input":{"contribution_code":"send","config_payload":{"token":"private-token","channel":"ops"},"input_payload":{"text":text}}})).unwrap()
}

#[test]
fn notification_sdk_keeps_config_separate_and_projects_only_declared_output() {
    let response = execute_notification(
        request(json!("hello")),
        &descriptor(),
        |config, input| -> Result<Value, String> {
            assert_eq!(config["token"], "private-token");
            assert!(input.get("token").is_none());
            Ok(
                json!({"accepted":true,"delivery_id":format!("{}:{}",config["channel"].as_str().unwrap(),input["text"].as_str().unwrap())}),
            )
        },
    );
    assert!(response.ok);
    assert_eq!(
        response.result,
        json!({"accepted":true,"delivery_id":"ops:hello"})
    );
    assert!(!serde_json::to_string(&response)
        .unwrap()
        .contains("private-token"));
}

#[test]
fn notification_sdk_rejects_invalid_input_output_and_redacts_provider_errors() {
    let calls = AtomicUsize::new(0);
    let response = execute_notification(
        request(json!(42)),
        &descriptor(),
        |_, _| -> Result<Value, String> {
            calls.fetch_add(1, Ordering::SeqCst);
            unreachable!("invalid input must never reach the provider")
        },
    );
    assert!(!response.ok);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for result in [
        Ok(json!({"accepted":"yes","delivery_id":"bad"})),
        Err("private-token".to_owned()),
    ] {
        let response = execute_notification(request(json!("hello")), &descriptor(), |_, _| result);
        assert!(!response.ok);
        assert!(response.result.is_null());
        assert!(!serde_json::to_string(&response)
            .unwrap()
            .contains("private-token"));
    }
}

fn managed_descriptor() -> NotificationTypeDescriptor {
    let mut value = descriptor();
    value.authorization = Some(serde_json::from_value(json!({
        "actions":[{"code":"qr","label":"Scan QR code","kind":"qr_code"}],
        "managed_secret_keys":["qr_credential","bot_token","bound_user_id","context_token","cursor"]
    })).unwrap());
    value
}

fn managed_request(operation: Value) -> CapabilityStdioRequest {
    let mut value = serde_json::to_value(request(json!("hello"))).unwrap();
    value["input"]["input_payload"] =
        json!({"operation":operation,"managed_secrets":{"bound_user_id":"scanned-peer"}});
    serde_json::from_value(value).unwrap()
}

#[tokio::test(flavor = "current_thread")]
async fn notification_sdk_send_preserves_completed_markdown_for_the_supplier() {
    use crate::{
        CommunicationCommandReply, CommunicationMessageBlock as Block,
        CommunicationMessageContent as Content, CommunicationMessageInline as Inline,
    };
    let generated = CommunicationCommandReply::from_content(Content::Blocks(vec![
        Block::BulletList(vec![vec![
            Inline::Code("/help".into()),
            Inline::Text(" — first\nsecond".into()),
        ]]),
    ]))
    .unwrap()
    .text;
    for expected in [
        generated,
        "**Answer**\n\n- one\n- two\n\n```text\n*literal*\n```".into(),
    ] {
        let response = crate::execute_communication(
            managed_request(json!({"type":"send","input_payload":{"text":expected}})),
            &managed_descriptor(),
            |_, input| async move {
                let crate::NotificationOperation::Send { input_payload, .. } = input.operation else {
                    panic!("supplier must receive a send");
                };
                assert_eq!(input_payload["text"], expected, "the supplier boundary must not render again");
                Ok::<_, String>(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({
                    "type":"send","outcome":{"status":"succeeded","result":{"accepted":true,"delivery_id":"markdown-fixture"}}
                })).unwrap())
            },
        ).await;
        assert!(response.ok);
        assert_eq!(response.result["outcome"]["status"], "succeeded");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn authorizable_sdk_round_trips_public_qr_and_host_only_patch_on_existing_execute() {
    let descriptor = managed_descriptor();
    let response = crate::execute_authorizable_notification(
        managed_request(json!({"type":"authorize","operation":{"type":"begin","action":"qr"}})),
        &descriptor,
        |config, input| async move {
            assert_eq!(config["token"], "private-token");
            assert_eq!(input.managed_secrets["bound_user_id"], "scanned-peer");
            Ok::<_, String>(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({
                "type":"authorization","authorization":{"status":"pending","user_action":{"kind":"qr_code","qr_code_url":"https://example.com/qr","poll_interval_seconds":2}},
                "managed_secret_patch":{"qr_credential":"encrypted-by-host"}
            })).unwrap())
        },
    ).await;
    assert!(response.ok);
    assert_eq!(response.result["type"], "authorization");
    assert_eq!(
        response.result["managed_secret_patch"]["qr_credential"],
        "encrypted-by-host"
    );
    let public = &response.result["authorization"];
    assert!(public.get("managed_secret_patch").is_none());
    assert!(!serde_json::to_string(public)
        .unwrap()
        .contains("encrypted-by-host"));
}

#[tokio::test(flavor = "current_thread")]
async fn authorizable_sdk_errors_and_untrustworthy_send_results_are_unknown() {
    let descriptor = managed_descriptor();
    for result in [
        Err("private-token raw provider failure".to_owned()),
        Ok(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({"type":"send","outcome":{"status":"succeeded","result":{"accepted":"yes"}}})).unwrap()),
        Ok(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({"type":"send","outcome":{"status":"failed","failure_code":"provider_result_unknown"}})).unwrap()),
        Ok(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({"type":"send","outcome":{"status":"failed","failure_code":"provider_rejected"},"managed_secret_patch":{"undeclared":"private-token"}})).unwrap()),
    ] {
        let response = crate::execute_authorizable_notification(
            managed_request(json!({"type":"send","input_payload":{"text":"hello"}})),
            &descriptor,
            |_, _| async move { result },
        ).await;
        assert!(response.ok);
        assert_eq!(response.result, json!({"type":"send","outcome":{"status":"unknown","failure_code":"provider_result_unknown"}}));
        assert!(response.error.is_none());
    }
    let response = crate::execute_authorizable_notification(
        managed_request(json!({"type":"authorize","operation":{"type":"poll"}})),
        &descriptor,
        |_, _| async { Err::<crate::NotificationRuntimeResult, _>("private-token") },
    )
    .await;
    assert!(!response.ok);
    assert!(!serde_json::to_string(&response)
        .unwrap()
        .contains("private-token"));
}

#[tokio::test(flavor = "current_thread")]
async fn authorizable_sdk_rejects_invalid_requests_before_provider_io_and_retains_patches() {
    let descriptor = managed_descriptor();
    let calls = AtomicUsize::new(0);
    let mut injected = serde_json::to_value(managed_request(
        json!({"type":"authorize","operation":{"type":"poll"}}),
    ))
    .unwrap();
    injected["input"]["config_payload"]["bot_token"] = json!("private-token");
    for request in [
        serde_json::from_value(injected).unwrap(),
        managed_request(json!({"type":"send","input_payload":{"text":42}})),
        managed_request(
            json!({"type":"authorize","operation":{"type":"begin","action":"not-declared"}}),
        ),
    ] {
        let response = crate::execute_authorizable_notification(request, &descriptor, |_, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            async {
                Err::<crate::NotificationRuntimeResult, String>("unexpected provider call".into())
            }
        })
        .await;
        assert!(!response.ok);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for (code, status) in [
        ("authorization_required", "unauthorized"),
        ("authorization_invalid", "invalid"),
        ("recipient_not_ready", "authorized"),
    ] {
        let response = crate::execute_authorizable_notification(
            managed_request(json!({"type":"send","input_payload":{"text":"hello"}})),
            &descriptor,
            |_, _| async move { Ok::<_, String>(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({"type":"send","outcome":{"status":"failed","failure_code":code},"authorization":{"status":status},"managed_secret_patch":{"cursor":"next"}})).unwrap()) },
        ).await;
        assert!(response.ok);
        assert_eq!(response.result["authorization"]["status"], status);
        assert_eq!(response.result["managed_secret_patch"]["cursor"], "next");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn delivery_window_is_public_typed_input_and_survives_sdk_round_trip() {
    let descriptor = managed_descriptor();
    for (code, status) in [
        ("subscription_required", "waiting_for_message"),
        ("subscription_expired", "expired"),
        ("subscription_unavailable", "unknown"),
    ] {
        let window = if status == "waiting_for_message" {
            json!({"status":status})
        } else {
            json!({"status":status,"last_received_at_ms":1,"expires_at_ms":2})
        };
        let mut request = managed_request(json!({"type":"send","input_payload":{"text":"hello"}}));
        request.input["input_payload"]["delivery_window"] = window.clone();
        let expected = window.clone();
        let response = crate::execute_authorizable_notification(request, &descriptor, |_, input| async move {
            assert_eq!(serde_json::to_value(input.delivery_window).unwrap(), expected);
            assert!(!input.managed_secrets.contains_key("delivery_window"));
            Ok::<_, String>(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({"type":"send","outcome":{"status":"failed","failure_code":code},"authorization":{"status":"authorized","delivery_window":expected}})).unwrap())
        }).await;
        assert!(response.ok);
        assert_eq!(response.result["authorization"]["delivery_window"], window);
        assert_eq!(response.result["outcome"]["failure_code"], code);
    }
}

fn communication_descriptor() -> NotificationTypeDescriptor {
    let mut descriptor = managed_descriptor();
    descriptor
        .authorization
        .as_mut()
        .unwrap()
        .managed_secret_keys
        .retain(|key| key != "cursor");
    descriptor.receive = Some(serde_json::from_value(json!({
        "contribution_id":"notification-fixture.receive","default_system_prompt":"Read history and files through tools.",
        "mcp_tools":[
            {"name":"messages","description":"Read messages","operation":"history_messages","input_schema":{"type":"object"}},
            {"name":"files","description":"Read files","operation":"history_files","input_schema":{"type":"object"}},
            {"name":"read_file","description":"Read file","operation":"read_file","input_schema":{"type":"object"}}
        ]
    })).unwrap());
    descriptor
}

#[tokio::test(flavor = "current_thread")]
async fn communication_sdk_receives_candidates_and_downloads_bounded_chunks() {
    let descriptor = communication_descriptor();
    let response = crate::execute_communication(
        managed_request(json!({"type":"receive","cursor":"committed","max_messages":5,"wait_seconds":0})),
        &descriptor,
        |_, input| async move {
            assert!(matches!(input.operation, crate::NotificationOperation::Receive { cursor:Some(ref value), .. } if value == "committed"));
            Ok::<_, String>(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({
                "type":"receive","outcome":{"status":"succeeded","batch":{"messages":[{
                    "provider_message_id":"stable-id","conversation_id":"peer","sender":{"id":"peer"},"received_at_ms":1,"text":"hello"
                }],"candidate_cursor":"candidate","has_more":false}}
            })).unwrap())
        },
    ).await;
    assert!(response.ok);
    assert_eq!(
        response.result["outcome"]["batch"]["candidate_cursor"],
        "candidate"
    );
    let response = crate::execute_communication(
        managed_request(json!({"type":"download_attachment","attachment":{"provider_message_id":"m1","provider_attachment_id":"a1","download_token":"opaque"},"offset":0,"max_bytes":3})),
        &descriptor,
        |_, _| async move { Ok::<_, String>(serde_json::from_value::<crate::NotificationRuntimeResult>(json!({
            "type":"download_attachment","outcome":{"status":"succeeded","offset":0,"data_base64":"YWJj","eof":true}
        })).unwrap()) },
    ).await;
    assert!(response.ok);
    assert_eq!(response.result["outcome"]["data_base64"], "YWJj");
}

#[tokio::test(flavor = "current_thread")]
async fn communication_sdk_rejects_receive_overflow_before_io_and_never_invents_a_cursor() {
    let descriptor = communication_descriptor();
    let response = crate::execute_communication::<String, _>(
        managed_request(json!({"type":"receive","max_messages":101,"wait_seconds":0})),
        &descriptor,
        |_, _| async move {
            panic!("invalid poll must not reach provider");
            #[allow(unreachable_code)]
            Ok(crate::NotificationRuntimeResult::DownloadAttachment {
                outcome: crate::NotificationAttachmentDownloadOutcome::Succeeded {
                    offset: 0,
                    data_base64: String::new(),
                    eof: true,
                },
            })
        },
    )
    .await;
    assert!(!response.ok);
    let response = crate::execute_communication(
        managed_request(json!({"type":"receive","max_messages":1,"wait_seconds":0})),
        &descriptor,
        |_, _| async move { Err::<crate::NotificationRuntimeResult, _>("private-provider-token") },
    )
    .await;
    assert!(!response.ok);
    assert!(response.result.is_null());
    assert!(!serde_json::to_string(&response)
        .unwrap()
        .contains("private-provider-token"));
}
