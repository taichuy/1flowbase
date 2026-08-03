use super::*;

#[tokio::test]
async fn application_public_api_key_service_requires_application_edit_permission_for_create() {
    let harness =
        ApplicationPublicApiTestHarness::new_with_permissions(vec!["application.view.all"]);
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());

    let error = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Native clients".into(),
            expires_at: None,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_004_application_api_keys_fail_closed_for_workflows() {
    let harness = ApplicationPublicApiTestHarness::new();
    let workflow = harness.seed_workflow_application(actor_user_id(), "Order workflow");
    let repository = harness.repository();
    let service = ApplicationApiKeyService::new(repository.clone());

    let create_error = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: workflow.id,
            name: "Unsupported workflow key".into(),
            expires_at: None,
        })
        .await
        .unwrap_err();
    assert!(create_error
        .to_string()
        .contains("application_api_key_application_type"));

    let list_error = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: workflow.id,
        })
        .await
        .unwrap_err();
    assert!(list_error
        .to_string()
        .contains("application_api_key_application_type"));

    let revoke_error = service
        .revoke_api_key(RevokeApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: workflow.id,
            api_key_id: Uuid::now_v7(),
        })
        .await
        .unwrap_err();
    assert!(revoke_error
        .to_string()
        .contains("application_api_key_application_type"));

    let legacy_token = "sk-workflow-legacy-token";
    repository
        .create_api_key(&CreateApiKeyInput {
            id: Uuid::now_v7(),
            name: "Legacy workflow key".into(),
            token_hash: hash_api_key_token(legacy_token),
            token_prefix: "sk-workflow".into(),
            key_kind: domain::ApiKeyKind::ApplicationApiKey,
            application_id: Some(workflow.id),
            role_code: None,
            creator_user_id: actor_user_id(),
            tenant_id: Uuid::nil(),
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: workflow.workspace_id,
            enabled: true,
            expires_at: None,
        })
        .await
        .unwrap();

    let auth_error = service
        .authenticate_bearer_token(legacy_token)
        .await
        .unwrap_err();
    assert!(auth_error.to_string().contains("not_authenticated"));
}

#[tokio::test]
async fn application_public_api_create_returns_sk_token_exactly_once_and_allows_duplicate_names() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());

    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Native clients".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    assert!(created.token.starts_with("sk-"));
    assert!(created.api_key.token_prefix.starts_with("sk-"));
    assert_eq!(created.token.len(), 56);
    assert_eq!(created.api_key.token_prefix.len(), 15);
    assert_eq!(created.token.matches('-').count(), 2);
    assert_ne!(created.api_key.token_prefix, created.token);

    let duplicate = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Native clients".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    assert!(duplicate.token.starts_with("sk-"));
    assert_eq!(duplicate.token.len(), 56);
    assert_eq!(duplicate.api_key.token_prefix.len(), 15);
    assert_ne!(duplicate.api_key.id, created.api_key.id);
    assert_eq!(duplicate.api_key.name, created.api_key.name);

    let listed = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert_eq!(listed.len(), 2);
    assert!(listed.iter().any(|key| key.id == created.api_key.id
        && key.token_prefix == created.api_key.token_prefix
        && key.token_prefix != created.token));
    assert!(listed.iter().any(|key| key.id == duplicate.api_key.id
        && key.token_prefix == duplicate.api_key.token_prefix
        && key.token_prefix != duplicate.token));
}

#[tokio::test]
async fn application_public_api_list_only_returns_current_actor_keys_for_current_application() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_app = harness.seed_application(actor_user_id(), "First App");
    let second_app = harness.seed_application(actor_user_id(), "Second App");
    let service = ApplicationApiKeyService::new(harness.repository());

    let mine = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: first_app.id,
            name: "Mine".into(),
            expires_at: None,
        })
        .await
        .unwrap();
    service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: other_user_id(),
            application_id: first_app.id,
            name: "Other user".into(),
            expires_at: None,
        })
        .await
        .unwrap();
    service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: second_app.id,
            name: "Other app".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    let listed = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: first_app.id,
        })
        .await
        .unwrap();

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, mine.api_key.id);
    assert_eq!(listed[0].application_id, Some(first_app.id));
    assert_eq!(listed[0].creator_user_id, actor_user_id());
}

#[tokio::test]
async fn application_public_api_delete_removes_key_and_makes_token_unusable() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Temporary".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    service
        .revoke_api_key(RevokeApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            api_key_id: created.api_key.id,
        })
        .await
        .unwrap();

    let listed = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    let auth_error = service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap_err();

    assert!(!harness.repository().contains_api_key(created.api_key.id));
    assert!(listed.is_empty());
    assert!(auth_error.to_string().contains("not_authenticated"));
}

#[tokio::test]
async fn application_public_api_authentication_records_last_used_time_for_key_list() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Runtime client".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    let before_use = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    assert_eq!(before_use[0].id, created.api_key.id);
    assert!(before_use[0].last_used_at.is_none());

    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();

    let after_use = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    assert_eq!(after_use[0].id, created.api_key.id);
    assert!(after_use[0].last_used_at.is_some());
}

#[tokio::test]
async fn application_public_api_last_used_write_is_throttled_for_sixty_seconds() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let cache = Arc::new(harness.last_used_cache());
    let service =
        ApplicationApiKeyService::new(harness.repository()).with_last_used_cache(cache.clone());
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Runtime client".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();
    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();

    assert_eq!(
        harness
            .repository()
            .api_key_last_used_write_count(created.api_key.id),
        1
    );
    assert_eq!(cache.last_ttl(), Some(Duration::seconds(60)));
}

#[tokio::test]
async fn application_public_api_last_used_write_failure_does_not_fail_authentication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    repository.fail_mark_api_key_used(true);
    let service = ApplicationApiKeyService::new(repository);
    let created = service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Runtime client".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    service
        .authenticate_bearer_token(&created.token)
        .await
        .unwrap();
}

#[tokio::test]
async fn application_public_api_root_has_no_global_view_every_users_key_list_path() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let service = ApplicationApiKeyService::new(harness.repository());
    service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Owner key".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    let root_visible = service
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: root_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert!(
        root_visible.is_empty(),
        "root may manage explicitly authorized app resources, but key list remains current-actor scoped"
    );
}

#[tokio::test]
async fn application_public_api_rejects_legacy_data_model_api_key_tokens() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Support Bot");
    let repository = harness.repository();
    let user_api_key_service = ApiKeyService::new(repository.clone());
    let application_key_service = ApplicationApiKeyService::new(repository);

    let apk = application_key_service
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "Application runtime".into(),
            expires_at: None,
        })
        .await
        .unwrap();

    assert!(apk.token.starts_with("sk-"));
    application_key_service
        .authenticate_bearer_token(&apk.token)
        .await
        .unwrap();
    assert!(user_api_key_service
        .authenticate_bearer_token("dmk_legacy_token")
        .await
        .is_err());
    assert!(application_key_service
        .authenticate_bearer_token("dmk_legacy_token")
        .await
        .is_err());
    assert!(user_api_key_service
        .authenticate_bearer_token(&apk.token)
        .await
        .is_err());
    assert!(application_key_service
        .authenticate_bearer_token("apk_legacy_token")
        .await
        .is_err());
}
