use super::*;

#[test]
fn correlated_send_receipt_projects_declared_identity_without_exempting_credentials() {
    let mut descriptor = system_notification_descriptor();
    descriptor.authorization = Some(
        serde_json::from_value(json!({
            "actions":[{"code":"qr","label":"Scan","kind":"qr_code"}],
            "managed_secret_keys":["account_identity","bot_token"],
            "managed_identity_keys":["account_identity"]
        }))
        .unwrap(),
    );
    descriptor.receive = Some(
        serde_json::from_value(json!({
            "contribution_id":"fixture.receive","supports_send_receipts":true,
            "default_system_prompt":"Read channel history with scoped tools.","mcp_tools":[]
        }))
        .unwrap(),
    );
    let request_id = Uuid::now_v7().to_string();
    let identity = "fixture-recipient";
    let mut input: NotificationRuntimeInput = serde_json::from_value(json!({
        "operation":{"type":"send","request_id":request_id,"input_payload":{"text":"Hello"},
            "target":{"sender_id":identity,"conversation_id":identity,"provider_message_id":"inbound-1"}}
    })).unwrap();
    let managed = BTreeMap::from([
        ("account_identity".into(), json!(identity)),
        ("bot_token".into(), json!("private-bot-token")),
    ]);
    let empty = BTreeMap::new();
    let public = json!({"receipt":{"request_id":request_id,"recipient_id":identity,
        "conversation_id":identity,"client_message_id":"client-1"},"result":{"accepted":true}});
    // The old whole-envelope guard falsely rejects a valid correlated send.
    assert!(public_contains_credentials(
        &public,
        &descriptor,
        &json!({}),
        &managed,
        &empty
    ));
    assert!(!send_result_contains_credentials(
        &public,
        &descriptor,
        &json!({}),
        &managed,
        &empty,
        &input
    ));
    assert!(!send_result_contains_credentials(
        &public,
        &descriptor,
        &json!({}),
        &empty,
        &managed,
        &input
    ));
    let mut legacy = descriptor.clone();
    legacy
        .authorization
        .as_mut()
        .unwrap()
        .managed_identity_keys
        .clear();
    assert!(send_result_contains_credentials(
        &public,
        &legacy,
        &json!({}),
        &managed,
        &empty,
        &input
    ));
    for (path, value) in [
        ("/receipt/client_message_id", identity),
        ("/receipt/provider_message_id", identity),
        ("/result/text", identity),
        ("/receipt/client_message_id", "private-bot-token"),
        ("/receipt/recipient_id", "private-bot-token"),
    ] {
        let mut unsafe_output = public.clone();
        let (parent, key) = path.rsplit_once('/').unwrap();
        unsafe_output
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), json!(value));
        assert!(
            send_result_contains_credentials(
                &unsafe_output,
                &descriptor,
                &json!({}),
                &managed,
                &empty,
                &input
            ),
            "{path}"
        );
    }
    let mut wrong = public.clone();
    wrong["receipt"]["request_id"] = json!(Uuid::now_v7());
    assert!(send_result_contains_credentials(
        &wrong,
        &descriptor,
        &json!({}),
        &managed,
        &empty,
        &input
    ));
    wrong = public.clone();
    wrong["receipt"]["conversation_id"] = json!("another-conversation");
    assert!(send_result_contains_credentials(
        &wrong,
        &descriptor,
        &json!({}),
        &managed,
        &empty,
        &input
    ));
    // A credential that happens to equal the identity is still a credential.
    let mut collision = managed.clone();
    collision.insert("bot_token".into(), json!(identity));
    assert!(send_result_contains_credentials(
        &public,
        &descriptor,
        &json!({}),
        &collision,
        &empty,
        &input
    ));
    descriptor.config_form=serde_json::from_value(json!({"schema_version":"1.0.0","fields":[{"key":"password","label":"Password","type":"string","control":"password"}]})).unwrap();
    assert!(send_result_contains_credentials(
        &public,
        &descriptor,
        &json!({"password":identity}),
        &managed,
        &empty,
        &input
    ));
    if let NotificationOperation::Send { target, .. } = &mut input.operation {
        *target = None;
    }
    // Proactive sends may disclose the exact declared account identity only in the same
    // correlated standard receipt fields, without creating a fictitious inbound target.
    assert!(!send_result_contains_credentials(
        &public,
        &descriptor,
        &json!({}),
        &managed,
        &empty,
        &input
    ));
}

#[test]
fn opted_in_legacy_account_projects_unknown_without_changing_other_channels() {
    let view: NotificationAuthorizationView =
        serde_json::from_value(json!({"status":"authorized"})).unwrap();
    let mut descriptor = system_notification_descriptor();
    descriptor.log = None;
    descriptor.authorization = Some(
        serde_json::from_value(json!({
            "actions":[{"code":"qr","label":"Scan","kind":"qr_code"}],
            "managed_secret_keys":["token"], "supports_delivery_window":true
        }))
        .unwrap(),
    );
    let projected = project_for_descriptor(view.clone(), &descriptor);
    assert_eq!(projected.status, Status::Authorized);
    assert_eq!(
        projected.delivery_window.unwrap().status,
        extension_contracts::NotificationDeliveryWindowStatus::Unknown
    );
    descriptor
        .authorization
        .as_mut()
        .unwrap()
        .supports_delivery_window = false;
    assert_eq!(
        project_for_descriptor(view, &descriptor).delivery_window,
        None
    );
    descriptor
        .authorization
        .as_mut()
        .unwrap()
        .supports_delivery_window = true;
    assert_eq!(
        project_for_descriptor(unauthorized(), &descriptor).delivery_window,
        None
    );
}

#[test]
fn host_secret_projection_rejects_existing_and_new_credentials_and_patch_deletion_is_exact() {
    let mut descriptor = system_notification_descriptor();
    descriptor.log = None;
    descriptor.config_form=serde_json::from_value(json!({"schema_version":"1.0.0","fields":[{"key":"token","label":"Token","type":"string","control":"password"}]})).unwrap();
    let config = json!({"token":"private-config-token"});
    let managed = BTreeMap::from([
        ("bot_token".into(), json!("private-bot-token")),
        ("cursor".into(), json!({"opaque":"private-cursor"})),
    ]);
    let patch = BTreeMap::from([("context_token".into(), json!("private-context-token"))]);
    for secret in [
        "private-config-token",
        "private-bot-token",
        "private-cursor",
        "private-context-token",
    ] {
        assert!(public_contains_credentials(
            &json!({"result":{"delivery_id":format!("sent:{secret}")}}),
            &descriptor,
            &config,
            &managed,
            &patch
        ));
        assert!(public_contains_credentials(
            &json!({"user_action":{"qr_code_url":format!("https://example.com/{secret}")}}),
            &descriptor,
            &config,
            &managed,
            &patch
        ));
    }
    assert!(!public_contains_credentials(
        &json!({"status":"authorized"}),
        &descriptor,
        &config,
        &managed,
        &patch
    ));
    let mut merged = managed.clone();
    apply_patch(
        &mut merged,
        BTreeMap::from([
            ("bot_token".into(), Value::Null),
            ("context_token".into(), json!("private-context-token")),
        ]),
    );
    assert!(!merged.contains_key("bot_token"));
    assert_eq!(merged["cursor"], managed["cursor"]);
    assert_eq!(merged["context_token"], patch["context_token"]);
}

#[test]
fn encrypted_cached_qr_url_is_public_only_in_the_exact_authorization_display_field() {
    let descriptor = system_notification_descriptor();
    let cached = json!("https://example.com/display-qr");
    let secrets = BTreeMap::from([("qr_code_url".into(), cached.clone())]);
    let empty = BTreeMap::new();
    let pending = json!({"status":"pending","user_action":{"kind":"qr_code","qr_code_url":cached,"poll_interval_seconds":2}});
    assert!(!public_contains_credentials(
        &pending,
        &descriptor,
        &json!({}),
        &secrets,
        &empty
    ));
    assert!(!public_contains_credentials(
        &json!({"authorization":pending,"result":null}),
        &descriptor,
        &json!({}),
        &empty,
        &secrets
    ));
    assert!(public_contains_credentials(
        &json!({"authorization":pending,"result":{"delivery_id":cached}}),
        &descriptor,
        &json!({}),
        &secrets,
        &empty
    ));
    let mut invalid = pending.clone();
    invalid["user_action"]["expires_at"] = cached.clone();
    assert!(public_contains_credentials(
        &invalid,
        &descriptor,
        &json!({}),
        &secrets,
        &empty
    ));
    // The QR-field exception never exempts the same string when classified as a bot token.
    let token = BTreeMap::from([("bot_token".into(), cached)]);
    assert!(public_contains_credentials(
        &pending,
        &descriptor,
        &json!({}),
        &token,
        &empty
    ));
}

#[test]
fn qr_challenge_may_appear_only_in_its_exact_cached_display_url() {
    let descriptor = system_notification_descriptor();
    let challenge = "scan-challenge";
    let url = format!("https://example.com/scan?key={challenge}");
    let patch = BTreeMap::from([
        ("qr_credential".into(), json!(challenge)),
        ("qr_code_url".into(), json!(url)),
    ]);
    let empty = BTreeMap::new();
    let pending = json!({"status":"pending","user_action":{"kind":"qr_code","qr_code_url":url,"poll_interval_seconds":2}});
    assert!(!public_contains_credentials(
        &pending,
        &descriptor,
        &json!({}),
        &empty,
        &patch
    ));
    assert!(!public_contains_credentials(
        &json!({"authorization":pending,"result":null}),
        &descriptor,
        &json!({}),
        &patch,
        &empty
    ));
    let mut reflected = pending.clone();
    reflected["user_action"]["expires_at"] = json!(challenge);
    assert!(public_contains_credentials(
        &reflected,
        &descriptor,
        &json!({}),
        &empty,
        &patch
    ));
    let mut substituted = pending.clone();
    substituted["user_action"]["qr_code_url"] =
        json!(format!("https://example.com/other?key={challenge}"));
    assert!(public_contains_credentials(
        &substituted,
        &descriptor,
        &json!({}),
        &empty,
        &patch
    ));
    assert!(public_contains_credentials(
        &json!({"authorization":pending,"result":{"delivery_id":challenge}}),
        &descriptor,
        &json!({}),
        &empty,
        &patch
    ));
    let tokens = BTreeMap::from([("bot_token".into(), json!(challenge))]);
    assert!(public_contains_credentials(
        &pending,
        &descriptor,
        &json!({}),
        &tokens,
        &patch
    ));
}

#[test]
fn delivery_window_projection_expires_at_boundary_without_revoking_authorization() {
    let active: NotificationAuthorizationView = serde_json::from_value(json!({"status":"authorized","delivery_window":{"status":"active","last_received_at_ms":100,"expires_at_ms":200}})).unwrap();
    assert_eq!(project_authorization_at(active.clone(), 199), active);
    for now in [200, 201, u64::MAX] {
        let expired = project_authorization_at(active.clone(), now);
        assert_eq!(expired.status, Status::Authorized);
        let window = expired.delivery_window.unwrap();
        assert_eq!(
            window.status,
            extension_contracts::NotificationDeliveryWindowStatus::Expired
        );
        assert_eq!(window.last_received_at_ms, Some(100));
        assert_eq!(window.expires_at_ms, Some(200));
    }
    for window in [
        json!({"status":"unknown","last_received_at_ms":100,"expires_at_ms":200}),
        json!({"status":"waiting_for_message"}),
    ] {
        let view: NotificationAuthorizationView =
            serde_json::from_value(json!({"status":"authorized","delivery_window":window}))
                .unwrap();
        assert_eq!(project_authorization_at(view.clone(), u64::MAX), view);
    }
    assert_eq!(
        project_authorization_at(unauthorized(), 200),
        unauthorized()
    );
    assert!(unauthorized().delivery_window.is_none());
}
