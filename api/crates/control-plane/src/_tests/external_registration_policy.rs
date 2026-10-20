use super::*;

#[test]
fn external_registration_requires_verified_nonempty_valid_email() {
    let entry = domain::LoginEntryRecord {
        id: Uuid::now_v7(),
        connection_id: Uuid::now_v7(),
        auth_type: "github".into(),
        title: "GitHub".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 1,
        public_ui_block: String::new(),
        options: serde_json::json!({"extension_config":{"self_registration_enabled":true}}),
    };
    for profile in [
        serde_json::json!({}),
        serde_json::json!({"email":"member@example.com","email_verified":false}),
        serde_json::json!({"email":"","email_verified":true}),
        serde_json::json!({"email":"bad@@example.com","email_verified":true}),
        serde_json::json!({"email":"member@.com","email_verified":true}),
        serde_json::json!({"email":"member\n@example.com","email_verified":true}),
    ] {
        let identity = domain::VerifiedExternalIdentity::try_from(domain::ExternalIdentityClaim {
            connection_id: entry.connection_id,
            subject_type: "github".into(),
            subject_value: "42".into(),
            issuer: None,
            realm: None,
            profile,
            verified: true,
            metadata: serde_json::json!({}),
        })
        .unwrap();
        assert!(external_registration_email(&entry, &identity, true)
            .unwrap_err()
            .to_string()
            .contains("external_registration_email_required"));
    }
    let identity = domain::VerifiedExternalIdentity::try_from(domain::ExternalIdentityClaim {
        connection_id: entry.connection_id,
        subject_type: "github".into(),
        subject_value: "42".into(),
        issuer: None,
        realm: None,
        profile: serde_json::json!({"email":" Member+Tag@Example.COM ","email_verified":true}),
        verified: true,
        metadata: serde_json::json!({}),
    })
    .unwrap();
    assert_eq!(
        external_registration_email(&entry, &identity, true).unwrap(),
        "member+tag@example.com"
    );
}

#[test]
fn external_registration_profile_hints_are_optional_bounded_and_not_provider_specific() {
    let valid = serde_json::json!({"account":" GitHub-User ", "nickname":" 昵称 ", "name":" Explicit Name ",
        "avatar_url":"https://avatars.example.test/user"});
    assert_eq!(
        external_registration_account(&valid).as_deref(),
        Some("GitHub-User")
    );
    assert_eq!(
        external_registration_text(&valid, "nickname").as_deref(),
        Some("昵称")
    );
    assert_eq!(
        external_registration_avatar(&valid).as_deref(),
        Some("https://avatars.example.test/user")
    );
    for account in [
        "",
        " ",
        "/root",
        "bad user",
        "name@example.com",
        &"a".repeat(65),
    ] {
        assert!(external_registration_account(&serde_json::json!({"account":account})).is_none());
    }
    for nickname in ["", " ", "bad\nname", &"字".repeat(129)] {
        assert!(
            external_registration_text(&serde_json::json!({"nickname":nickname}), "nickname")
                .is_none()
        );
    }
    for avatar in [
        "javascript:alert(1)",
        "data:image/png;base64,AA",
        "http://example.test/avatar",
        "https://user:password@example.test/avatar",
        "https://example.test/\navatar",
    ] {
        assert!(external_registration_avatar(&serde_json::json!({"avatar_url":avatar})).is_none());
    }
    let raw = serde_json::json!({"login":"github-login","name":"GitHub Public Name"});
    assert!(external_registration_account(&raw).is_none());
    assert!(external_registration_text(&raw, "nickname").is_none());
}

#[test]
fn external_registration_name_requires_an_explicit_valid_provider_hint() {
    assert_eq!(
        external_registration_text(&serde_json::json!({"name":" Explicit Name "}), "name")
            .as_deref(),
        Some("Explicit Name")
    );
    for profile in [
        serde_json::json!({"account":"login-only"}),
        serde_json::json!({"name":""}),
        serde_json::json!({"name":"  "}),
        serde_json::json!({"name":"bad\nname"}),
        serde_json::json!({"name":"字".repeat(129)}),
    ] {
        assert!(external_registration_text(&profile, "name").is_none());
    }
}
