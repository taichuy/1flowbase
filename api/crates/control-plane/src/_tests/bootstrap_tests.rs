use crate::_tests::support::MemoryBootstrapRepository;
use crate::bootstrap::{BootstrapConfig, BootstrapService};

fn bootstrap_config() -> BootstrapConfig {
    BootstrapConfig {
        workspace_name: "1flowbase".into(),
        root_account: "root".into(),
        root_email: "root@example.com".into(),
        root_password_hash: "hash".into(),
        root_name: "Root".into(),
        root_nickname: "Root".into(),
    }
}

fn saved_password_authenticator(public_ui_block: &str) -> domain::AuthenticatorRecord {
    domain::AuthenticatorRecord {
        id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        auth_type: "password-local".into(),
        title: "Password".into(),
        enabled: true,
        is_builtin: true,
        sort_order: 0,
        public_ui_block: public_ui_block.to_string(),
        options: serde_json::json!({}),
    }
}

#[tokio::test]
async fn ac_005_bootstrap_upgrades_only_the_previous_official_password_block() {
    let previous_repository = MemoryBootstrapRepository::default();
    previous_repository
        .seed_authenticator(saved_password_authenticator(
            crate::auth::public_ui::PREVIOUS_PASSWORD_LOCAL_PUBLIC_UI_BLOCK,
        ))
        .await;
    BootstrapService::new(previous_repository.clone())
        .run(&bootstrap_config())
        .await
        .unwrap();
    assert_eq!(
        previous_repository
            .authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
            .await
            .unwrap()
            .public_ui_block,
        crate::auth::public_ui::PASSWORD_LOCAL_PUBLIC_UI_BLOCK
    );

    let custom_repository = MemoryBootstrapRepository::default();
    custom_repository
        .seed_authenticator(saved_password_authenticator("custom saved block"))
        .await;
    BootstrapService::new(custom_repository.clone())
        .run(&bootstrap_config())
        .await
        .unwrap();
    assert_eq!(
        custom_repository
            .authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
            .await
            .unwrap()
            .public_ui_block,
        "custom saved block"
    );
}

#[tokio::test]
async fn bootstrap_service_is_idempotent() {
    let repository = MemoryBootstrapRepository::default();
    let service = BootstrapService::new(repository.clone());
    let config = BootstrapConfig {
        workspace_name: "1flowbase".into(),
        root_account: "root".into(),
        root_email: "root@example.com".into(),
        root_password_hash: "hash".into(),
        root_name: "Root".into(),
        root_nickname: "Root".into(),
    };

    service.run(&config).await.unwrap();
    service.run(&config).await.unwrap();

    assert_eq!(repository.authenticator_upserts(), 2);
    assert_eq!(repository.root_user_creates(), 1);
}

#[tokio::test]
async fn bootstrap_service_seeds_password_local_authenticator_options() {
    let repository = MemoryBootstrapRepository::default();
    let service = BootstrapService::new(repository.clone());
    let config = BootstrapConfig {
        workspace_name: "1flowbase".into(),
        root_account: "root".into(),
        root_email: "root@example.com".into(),
        root_password_hash: "hash".into(),
        root_name: "Root".into(),
        root_nickname: "Root".into(),
    };

    service.run(&config).await.unwrap();

    let password_local = repository
        .authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
        .await
        .expect("password-local should be seeded");
    assert_eq!(
        password_local.options["description"],
        serde_json::json!("Local password authentication")
    );
    assert!(password_local.options["config_form_schema"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field["key"] != "name"));
    assert!(password_local.options["config_form_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "title" && field["required"] == true));
    assert!(password_local.options["config_form_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "description"
            && field["control"] == "textarea"
            && field["required"] == false));
    assert!(password_local.options["config_form_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "enabled"
            && field["type"] == "boolean"
            && field["control"] == "switch"));
    assert_eq!(
        password_local.options["extension_config"],
        serde_json::json!({ "self_registration_enabled": false })
    );
    assert!(password_local.options.get("name").is_none());
    assert!(password_local.options.get("title").is_none());
    assert!(password_local.options.get("enabled").is_none());
}

#[tokio::test]
async fn bootstrap_service_seeds_single_root_tenant_and_default_workspace() {
    let repository = MemoryBootstrapRepository::default();
    let service = BootstrapService::new(repository.clone());
    let config = BootstrapConfig {
        workspace_name: "1flowbase".into(),
        root_account: "root".into(),
        root_email: "root@example.com".into(),
        root_password_hash: "hash".into(),
        root_name: "Root".into(),
        root_nickname: "Root".into(),
    };

    service.run(&config).await.unwrap();
    service.run(&config).await.unwrap();

    assert_eq!(repository.root_tenant_upserts(), 2);
    assert_eq!(repository.workspace_upserts(), 2);
    assert_eq!(repository.workspace_role_template_seeds(), 1);
    assert_eq!(repository.root_user_creates(), 1);
}

#[tokio::test]
async fn bootstrap_service_returns_ids_needed_for_follow_up_startup_bootstrap() {
    let repository = MemoryBootstrapRepository::default();
    let service = BootstrapService::new(repository.clone());
    let config = BootstrapConfig {
        workspace_name: "1flowbase".into(),
        root_account: "root".into(),
        root_email: "root@example.com".into(),
        root_password_hash: "hash".into(),
        root_name: "Root".into(),
        root_nickname: "Root".into(),
    };

    let first = service.run(&config).await.unwrap();
    let second = service.run(&config).await.unwrap();

    assert_eq!(first.workspace_id, second.workspace_id);
    assert_eq!(first.root_user_id, second.root_user_id);
}

#[tokio::test]
async fn ac_001_initialized_catalog_does_not_load_the_official_seed() {
    let repository = MemoryBootstrapRepository::default();
    repository.mark_official_catalog_initialized();

    let result = BootstrapService::new(repository.clone())
        .run_with_official_catalog_loader(&bootstrap_config(), || {
            anyhow::bail!("initialized catalog must not load the embedded Seed")
        })
        .await;

    assert!(result.is_ok());
    assert_eq!(repository.official_catalog_bootstraps(), 0);
}

#[tokio::test]
async fn ac_002_uninitialized_catalog_loads_the_official_seed() {
    let repository = MemoryBootstrapRepository::default();

    let error = BootstrapService::new(repository)
        .run_with_official_catalog_loader(&bootstrap_config(), || {
            anyhow::bail!("controlled Seed load")
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("controlled Seed load"));
}
