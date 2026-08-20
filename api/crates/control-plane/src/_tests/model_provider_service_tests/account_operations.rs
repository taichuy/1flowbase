use super::*;

#[tokio::test]
async fn account_operations_project_usage_and_dispatch_each_consume_once() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let package_root =
        std::env::temp_dir().join(format!("provider-account-operations-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let runtime = MemoryProviderRuntime::default();
    let service = model_provider_service(
        repository.clone(),
        runtime.clone(),
        "provider-secret-master-key",
    );
    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Account operations fixture".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "configured-secret",
            }),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            preview_token: None,
        })
        .await
        .expect("fixture instance should be created");

    let usage = service
        .get_usage_windows(repository.actor.user_id, created.instance.id)
        .await
        .expect("usage should stay a typed provider account projection");
    assert_eq!(usage.windows[0].limit_window_seconds, 18_000);
    assert_eq!(usage.windows[1].used_percent, 61.0);

    let count = service
        .count_reset_credits(repository.actor.user_id, created.instance.id)
        .await
        .expect("count should return the provider's available reset credits");
    assert_eq!(count.available_count, 2);

    let consumed = service
        .consume_reset_credit(ConsumeModelProviderResetCreditCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            idempotency_key: "attempt-123".to_string(),
        })
        .await
        .expect("one consume command should dispatch once");
    assert!(consumed.consumed);
    assert_eq!(
        runtime.reset_credit_operations().await,
        vec![
            ProviderResetCreditOperation::Count,
            ProviderResetCreditOperation::Consume {
                idempotency_key: "attempt-123".to_string(),
            },
        ]
    );
    assert_eq!(
        repository.audit_events().await,
        vec![
            "model_provider.created",
            "model_provider.reset_credit_consumed",
        ]
    );
}
