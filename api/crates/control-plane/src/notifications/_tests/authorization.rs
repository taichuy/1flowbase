use super::*;

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
