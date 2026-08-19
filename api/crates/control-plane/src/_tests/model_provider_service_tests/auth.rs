use std::io::Write;

use super::*;

fn enable_fixture_provider_auth(package_root: &std::path::Path) {
    let mut provider = fs::OpenOptions::new()
        .append(true)
        .open(package_root.join("provider/fixture_provider.yaml"))
        .expect("fixture provider definition should be writable");
    writeln!(
        provider,
        r#"auth:
  actions:
    - code: device_code
      label: Device Code
      user_action_kinds:
        - device_code
  managed_secret_keys:
    - access_token
    - refresh_token"#
    )
    .expect("fixture provider auth declaration should be appended");
}

fn authorized_patch(key: &str, value: &str) -> ProviderAuthResult {
    ProviderAuthResult {
        status: ProviderAuthStatus::Authorized,
        message: None,
        user_action: None,
        managed_secret_patch: [(key.to_string(), json!(value))].into_iter().collect(),
    }
}

#[tokio::test]
async fn a_ac_02_authentication_patches_instance_secret_without_exposing_it_and_maintains_before_validate(
) {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let package_root = std::env::temp_dir().join(format!("provider-auth-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    enable_fixture_provider_auth(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let runtime = MemoryProviderRuntime::default();
    runtime
        .push_auth_result(authorized_patch("access_token", "initial-access-token"))
        .await;
    runtime
        .push_auth_result(authorized_patch("access_token", "rotated-access-token"))
        .await;
    let service = model_provider_service(
        repository.clone(),
        runtime.clone(),
        "provider-secret-master-key",
    );
    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Subscription fixture".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "configured-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            preview_token: None,
        })
        .await
        .expect("fixture instance should be created");

    let authentication = service
        .authenticate_instance(AuthenticateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            operation: ProviderAuthOperation::Begin {
                action: "device_code".to_string(),
            },
        })
        .await
        .expect("declared authentication action should patch the instance secret");
    assert_eq!(authentication.result.status, ProviderAuthStatus::Authorized);
    assert!(
        authentication
            .instance
            .config_json
            .get("access_token")
            .is_none(),
        "managed auth secrets must not enter the normal instance projection"
    );
    assert_eq!(
        repository.secret_json(created.instance.id).await["access_token"],
        "initial-access-token"
    );
    assert_eq!(
        repository.secret_version(created.instance.id).await,
        Some(2)
    );

    let validated = service
        .validate_instance(repository.actor.user_id, created.instance.id)
        .await
        .expect("maintain should refresh before the validate runtime call");
    assert!(
        validated.instance.config_json.get("access_token").is_none(),
        "validation response must not expose a managed auth secret"
    );
    assert_eq!(
        runtime.auth_operations().await,
        vec![
            ProviderAuthOperation::Begin {
                action: "device_code".to_string(),
            },
            ProviderAuthOperation::Maintain,
        ]
    );
    assert_eq!(
        runtime.validate_configs().await[0]["access_token"],
        "rotated-access-token"
    );
    assert_eq!(
        repository.secret_version(created.instance.id).await,
        Some(3)
    );
    assert_eq!(
        repository.audit_events().await,
        vec![
            "model_provider.created",
            "model_provider.authentication",
            "model_provider.validated"
        ]
    );
}

#[tokio::test]
async fn a_ac_05_rejects_undeclared_auth_secret_keys_and_stale_secret_versions() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryModelProviderRepository::new(actor_with_permissions(
        workspace_id,
        &["state_model.view.all", "state_model.manage.all"],
    ));
    let package_root = std::env::temp_dir().join(format!("provider-auth-{}", Uuid::now_v7()));
    create_provider_fixture(&package_root);
    enable_fixture_provider_auth(&package_root);
    let installation_id = repository
        .seed_installation(
            &package_root.display().to_string(),
            PluginDesiredState::ActiveRequested,
            true,
        )
        .await;
    let runtime = MemoryProviderRuntime::default();
    runtime
        .push_auth_result(authorized_patch("undeclared_secret", "must-not-persist"))
        .await;
    let service = model_provider_service(repository.clone(), runtime, "provider-secret-master-key");
    let created = service
        .create_instance(CreateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            installation_id,
            display_name: "Subscription fixture".to_string(),
            config_json: json!({
                "base_url": "https://api.example.com",
                "api_key": "configured-secret"
            }),
            configured_models: Vec::new(),
            enabled_model_ids: Vec::new(),
            included_in_main: None,
            preview_token: None,
        })
        .await
        .expect("fixture instance should be created");

    let error = service
        .authenticate_instance(AuthenticateModelProviderInstanceCommand {
            actor_user_id: repository.actor.user_id,
            instance_id: created.instance.id,
            operation: ProviderAuthOperation::Begin {
                action: "device_code".to_string(),
            },
        })
        .await
        .expect_err("undeclared managed secret keys must fail closed");
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidInput("provider_auth_secret_key"))
    ));
    assert!(repository
        .secret_json(created.instance.id)
        .await
        .get("undeclared_secret")
        .is_none());

    repository
        .patch_secret(&PatchModelProviderSecretInput {
            provider_instance_id: created.instance.id,
            expected_secret_version: Some(1),
            plaintext_secret_json: json!({ "api_key": "replacement" }),
            master_key: "provider-secret-master-key".to_string(),
        })
        .await
        .expect("the current secret version should be accepted");
    let stale_error = repository
        .patch_secret(&PatchModelProviderSecretInput {
            provider_instance_id: created.instance.id,
            expected_secret_version: Some(1),
            plaintext_secret_json: json!({ "api_key": "stale" }),
            master_key: "provider-secret-master-key".to_string(),
        })
        .await
        .expect_err("a stale secret patch must fail closed");
    assert!(matches!(
        stale_error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "model_provider_secret_version_conflict"
        ))
    ));
}
