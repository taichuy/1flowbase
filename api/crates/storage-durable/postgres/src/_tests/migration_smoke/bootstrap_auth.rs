use super::*;

#[tokio::test]
async fn bootstrap_repository_upserts_password_local_and_root_user() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let root = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    assert_eq!(root.account, "root");
    assert!(store
        .list_permissions()
        .await
        .unwrap()
        .iter()
        .any(|permission| permission.code == "workspace.configure.all"));
    assert_eq!(
        store
            .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
            .await
            .unwrap()
            .unwrap()
            .id,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
    );
    let root_identities: Vec<(String, String)> = sqlx::query_as(
        r#"
        select subject_type, subject_value
        from user_auth_identities
        where user_id = $1
        order by subject_type asc
        "#,
    )
    .bind(root.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        root_identities,
        vec![
            ("account".to_string(), "root".to_string()),
            ("email".to_string(), "root@example.com".to_string()),
        ]
    );
}

#[tokio::test]
async fn bootstrap_repository_preserves_password_local_saved_config_on_conflict() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: "default password block".into(),
            options: serde_json::json!({
                "description": "Local password authentication",
                "config_form_schema": [
                    {
                        "key": "description",
                        "label": "Description",
                        "type": "string",
                        "control": "textarea",
                        "read_only": false,
                        "required": false
                    }
                ],
                "extension_config": {}
            }),
        })
        .await
        .unwrap();

    let mut saved = store
        .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
        .await
        .unwrap()
        .unwrap();
    saved.title = "Custom Password".into();
    saved.enabled = false;
    saved.public_ui_block = "custom saved password block".into();
    saved.options = serde_json::json!({
        "description": "Custom local password",
        "config_form_schema": [
            {
                "key": "description",
                "label": "Custom Description",
                "type": "string",
                "control": "textarea",
                "read_only": false,
                "required": false
            }
        ],
        "extension_config": {
            "lockout_after_attempts": 5
        }
    });
    store.update_authenticator_config(&saved).await.unwrap();

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: "upgraded default password block".into(),
            options: serde_json::json!({
                "description": "Local password authentication",
                "config_form_schema": [
                    {
                        "key": "description",
                        "label": "Description",
                        "type": "string",
                        "control": "textarea",
                        "read_only": false,
                        "required": false
                    }
                ],
                "extension_config": {}
            }),
        })
        .await
        .unwrap();

    let after_bootstrap = store
        .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_bootstrap.title, "Custom Password");
    assert!(!after_bootstrap.enabled);
    assert_eq!(
        after_bootstrap.public_ui_block,
        "custom saved password block"
    );
    assert_eq!(
        after_bootstrap.options["description"],
        serde_json::json!("Custom local password")
    );
    assert_eq!(
        after_bootstrap.options["config_form_schema"][0]["label"],
        serde_json::json!("Custom Description")
    );
    assert_eq!(
        after_bootstrap.options["extension_config"],
        serde_json::json!({
            "lockout_after_attempts": 5
        })
    );
}

#[tokio::test]
async fn ac_005_bootstrap_replaces_only_the_previous_official_authenticator_block() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let mut authenticator = domain::AuthenticatorRecord {
        id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        auth_type: "password-local".into(),
        title: "Password".into(),
        enabled: true,
        is_builtin: true,
        sort_order: 0,
        public_ui_block: "previous official block".into(),
        options: serde_json::json!({}),
    };
    store.upsert_authenticator(&authenticator).await.unwrap();

    let replaced = control_plane::ports::BootstrapRepository::
        replace_authenticator_public_ui_block_if_matches(
            &store,
            authenticator.id,
            "previous official block",
            "current official block",
        )
        .await
        .unwrap();
    assert!(replaced);
    assert_eq!(
        store
            .find_authenticator(authenticator.id)
            .await
            .unwrap()
            .unwrap()
            .public_ui_block,
        "current official block"
    );

    authenticator.public_ui_block = "custom saved block".into();
    store
        .update_authenticator_config(&authenticator)
        .await
        .unwrap();
    let replaced = control_plane::ports::BootstrapRepository::
        replace_authenticator_public_ui_block_if_matches(
            &store,
            authenticator.id,
            "previous official block",
            "another official block",
        )
        .await
        .unwrap();
    assert!(!replaced);
    assert_eq!(
        store
            .find_authenticator(authenticator.id)
            .await
            .unwrap()
            .unwrap()
            .public_ui_block,
        "custom saved block"
    );
}

#[tokio::test]
async fn bootstrap_repository_overwrites_non_builtin_authenticator_on_conflict() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let oidc_id = Uuid::now_v7();

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: oidc_id,
            auth_type: "oidc".into(),
            title: "OIDC".into(),
            enabled: false,
            is_builtin: false,
            sort_order: 10,
            public_ui_block: "old oidc block".into(),
            options: serde_json::json!({
                "description": "Old OIDC",
                "extension_config": {
                    "issuer_url": "https://old.example.com"
                }
            }),
        })
        .await
        .unwrap();

    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: oidc_id,
            auth_type: "oidc".into(),
            title: "OIDC Login".into(),
            enabled: true,
            is_builtin: false,
            sort_order: 20,
            public_ui_block: "new oidc block".into(),
            options: serde_json::json!({
                "description": "New OIDC",
                "extension_config": {
                    "issuer_url": "https://new.example.com"
                }
            }),
        })
        .await
        .unwrap();

    let oidc = store.find_authenticator(oidc_id).await.unwrap().unwrap();
    assert_eq!(oidc.title, "OIDC Login");
    assert!(oidc.enabled);
    // Provider reconciliation may refresh provider-owned defaults/config, but a
    // non-empty instance Block is user-owned and must survive plugin upgrades.
    assert_eq!(oidc.public_ui_block, "old oidc block");
    assert_eq!(oidc.options["description"], serde_json::json!("New OIDC"));
    assert_eq!(
        oidc.options["extension_config"]["issuer_url"],
        serde_json::json!("https://new.example.com")
    );
}
