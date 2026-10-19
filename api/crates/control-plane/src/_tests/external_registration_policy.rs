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
