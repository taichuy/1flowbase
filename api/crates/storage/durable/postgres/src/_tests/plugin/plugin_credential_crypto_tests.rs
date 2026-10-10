use super::*;
use serde_json::json;
fn binding() -> PluginCredentialBinding {
    PluginCredentialBinding {
        contribution_id: "ssh.settings".into(),
        installation_id: Uuid::now_v7().to_string(),
        publisher_namespace: "acme".into(),
        plugin_code: "ssh".into(),
        plugin_version: "1.0.0".into(),
        scope_id: Uuid::now_v7().to_string(),
        deadline_unix_ms: i64::MAX,
    }
}
#[test]
fn credential_ciphertext_is_bound_to_owner_scope_and_id() {
    let binding = binding();
    let secret = json!({"password":"fixture-secret"});
    let aad = associated_data(&binding, "host-1").unwrap();
    let encrypted = encrypt_secret_json_with_aad(&secret, "fixture-key", &aad).unwrap();
    assert!(!encrypted.to_string().contains("fixture-secret"));
    assert_eq!(
        decrypt_secret_json_with_aad(&encrypted, "fixture-key", &aad).unwrap(),
        secret
    );
    for field in ["publisher", "plugin", "scope", "id"] {
        let mut other = binding.clone();
        match field {
            "publisher" => other.publisher_namespace = "other".into(),
            "plugin" => other.plugin_code = "other".into(),
            "scope" => other.scope_id = Uuid::now_v7().to_string(),
            _ => {}
        }
        let aad = associated_data(&other, if field == "id" { "host-2" } else { "host-1" }).unwrap();
        assert!(decrypt_secret_json_with_aad(&encrypted, "fixture-key", &aad).is_err());
    }
    assert!(decrypt_secret_json_with_aad(&encrypted, "wrong-key", &aad).is_err());
}
#[tokio::test]
async fn credential_repository_roundtrip_isolated_and_encrypted() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    crate::run_migrations(&pool).await.unwrap();
    let repository = PgPluginCredentialRepository::new(pool.clone(), "fixture-key").unwrap();
    let binding = binding();
    let get = PluginCredentialRequest::Get {
        credential_id: "host-1".into(),
    };
    let put = PluginCredentialRequest::Put {
        credential_id: "host-1".into(),
        value: json!({"password":"fixture-secret"}),
    };
    repository
        .execute_plugin_credential(&binding, &put)
        .await
        .unwrap();
    let stored: Value = sqlx::query_scalar("select encrypted_value from plugin_credentials")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!stored.to_string().contains("fixture-secret"));
    assert_eq!(
        repository
            .execute_plugin_credential(&binding, &get)
            .await
            .unwrap()
            .value,
        Some(json!({"password":"fixture-secret"}))
    );
    for field in ["publisher", "plugin", "scope"] {
        let mut other = binding.clone();
        match field {
            "publisher" => other.publisher_namespace = "other".into(),
            "plugin" => other.plugin_code = "other".into(),
            _ => other.scope_id = Uuid::now_v7().to_string(),
        }
        assert!(repository
            .execute_plugin_credential(&other, &get)
            .await
            .unwrap()
            .value
            .is_none());
        repository
            .execute_plugin_credential(
                &other,
                &PluginCredentialRequest::Delete {
                    credential_id: "host-1".into(),
                },
            )
            .await
            .unwrap();
    }
    assert!(repository
        .execute_plugin_credential(&binding, &get)
        .await
        .unwrap()
        .value
        .is_some());
    repository
        .execute_plugin_credential(
            &binding,
            &PluginCredentialRequest::Delete {
                credential_id: "host-1".into(),
            },
        )
        .await
        .unwrap();
    assert!(repository
        .execute_plugin_credential(&binding, &get)
        .await
        .unwrap()
        .value
        .is_none());
}
