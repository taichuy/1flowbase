use super::*;
use control_plane::application_public_api::native::{
    ApplicationNativeRunService, CancelNativeRunCommand, CreateNativeRunCommand,
    GetNativeRunByProviderResponseIdCommand, GetNativeRunCommand, NativeRunError,
};

#[tokio::test]
async fn authentication_dependency_failures_keep_source_across_native_entry_points_and_recover() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Authentication dependency probe");
    let repository = harness.repository();
    let keys = ApplicationApiKeyService::new(repository.clone());
    let issued = keys
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            name: "isolated probe".into(),
            expires_at: None,
        })
        .await
        .unwrap();
    let native = ApplicationNativeRunService::new(repository.clone());
    let published =
        control_plane::application_public_api::run_service::ApplicationPublishedRunService::new(
            repository.clone(),
        );
    for application_lookup in [false, true] {
        for unavailable in [false, true] {
            repository.set_authentication_lookup_failure(application_lookup, Some(unavailable));
            let direct = keys
                .authenticate_bearer_token(&issued.token)
                .await
                .unwrap_err();
            assert!(direct.downcast_ref::<std::io::Error>().is_some());
            assert!(direct.to_string().contains("repository lookup"));
            assert_eq!(
                direct.downcast_ref::<ControlPlaneError>().is_some(),
                unavailable
            );
            let command = || {
                CreateNativeRunCommand {
                bearer_token: issued.token.clone(),
                request: serde_json::from_value(serde_json::json!({"query":"probe"})).unwrap(),
                protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            }
            };
            let errors = [
                native.create_native_run(command()).await.unwrap_err(),
                published.start_native_run(command()).await.unwrap_err(),
                native
                    .get_native_run(GetNativeRunCommand {
                        bearer_token: issued.token.clone(),
                        run_id: Uuid::nil(),
                    })
                    .await
                    .unwrap_err(),
                native
                    .get_native_run_by_provider_response_id(
                        GetNativeRunByProviderResponseIdCommand {
                            bearer_token: issued.token.clone(),
                            provider_response_id: "resp_probe".into(),
                        },
                    )
                    .await
                    .unwrap_err(),
                native
                    .cancel_native_run(CancelNativeRunCommand {
                        bearer_token: issued.token.clone(),
                        run_id: Uuid::nil(),
                    })
                    .await
                    .unwrap_err(),
            ];
            for error in errors {
                let NativeRunError::Service(error) = error else {
                    panic!("dependency fault became credential rejection")
                };
                assert!(error.downcast_ref::<std::io::Error>().is_some());
                assert_eq!(
                    error.downcast_ref::<ControlPlaneError>().is_some(),
                    unavailable
                );
                assert!(error.chain().count() >= 2);
            }
            repository.set_authentication_lookup_failure(application_lookup, None);
            assert!(keys.authenticate_bearer_token(&issued.token).await.is_ok());
        }
    }
    for token in ["not-a-key", "sk-unknown-application-key"] {
        let error = keys.authenticate_bearer_token(token).await.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::NotAuthenticated)
        ));
        let error = native
            .get_native_run(GetNativeRunCommand {
                bearer_token: token.into(),
                run_id: Uuid::nil(),
            })
            .await
            .unwrap_err();
        let NativeRunError::Service(error) = error else {
            panic!("authentication source was discarded")
        };
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::NotAuthenticated)
        ));
    }
}

#[tokio::test]
async fn disabled_expired_and_wrong_kind_application_keys_are_typed_rejections() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Credential rejection probe");
    let repository = harness.repository();
    let service = ApplicationApiKeyService::new(repository.clone());
    for (index, enabled, expires_at, key_kind) in [
        (0, false, None, domain::ApiKeyKind::ApplicationApiKey),
        (
            1,
            true,
            Some(time::OffsetDateTime::now_utc() - time::Duration::seconds(1)),
            domain::ApiKeyKind::ApplicationApiKey,
        ),
        (2, true, None, domain::ApiKeyKind::UserApiKey),
    ] {
        let token = format!("sk-isolated-rejection-probe-{index}");
        repository
            .create_api_key(&CreateApiKeyInput {
                id: Uuid::now_v7(),
                name: "rejection probe".into(),
                token_hash: hash_api_key_token(&token),
                token_prefix: "sk-isolated".into(),
                key_kind,
                application_id: Some(application.id),
                role_code: None,
                creator_user_id: actor_user_id(),
                tenant_id: Uuid::nil(),
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: application.workspace_id,
                enabled,
                expires_at,
            })
            .await
            .unwrap();
        let error = service.authenticate_bearer_token(&token).await.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::NotAuthenticated)
        ));
        assert_eq!(
            repository.api_key_last_used_write_count(
                repository
                    .find_api_key_by_token_hash(&hash_api_key_token(&token))
                    .await
                    .unwrap()
                    .unwrap()
                    .id
            ),
            0
        );
    }
}
