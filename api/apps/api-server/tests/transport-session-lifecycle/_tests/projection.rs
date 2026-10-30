use super::*;
use crate::application_public_api::ApplicationPublicApiTestHarness;
use control_plane::{
    application_public_api::{
        api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
        native::{ApplicationNativeRunService, GetNativeRunCommand, NativeRunError},
    },
    errors::ControlPlaneError,
};

async fn response(error: NativeRunError, status: StatusCode, code: &str) {
    let response = response_projection::native_error(error).into_response();
    assert_eq!(response.status(), status);
    let bytes = axum::body::to_bytes(response.into_body(), 16_384)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["code"], code);
    assert!(body["message"].is_string());
    assert_eq!(body.as_object().unwrap().len(), 2);
}

#[tokio::test]
async fn real_authentication_and_authenticated_repository_faults_keep_http_status_and_shape() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let actor_id = uuid::Uuid::from_u128(0x11111111111111111111111111111111);
    let application = harness.seed_application(actor_id, "HTTP projection regression");
    let keys = ApplicationApiKeyService::new(repository.clone());
    let issued = keys
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: actor_id,
            application_id: application.id,
            name: "isolated".into(),
            expires_at: None,
        })
        .await
        .unwrap();
    let native = ApplicationNativeRunService::new(repository.clone());
    let actor = keys.authenticate_bearer_token(&issued.token).await.unwrap();
    for application_lookup in [false, true] {
        for unavailable in [false, true] {
            repository.set_authentication_lookup_failure(application_lookup, Some(unavailable));
            let error = native
                .get_native_run(GetNativeRunCommand {
                    bearer_token: issued.token.clone(),
                    run_id: uuid::Uuid::nil(),
                })
                .await
                .unwrap_err();
            response(
                error,
                if unavailable {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                },
                if unavailable {
                    "authentication_dependency"
                } else {
                    "internal_error"
                },
            )
            .await;
            repository.set_authentication_lookup_failure(application_lookup, None);
            assert!(keys.authenticate_bearer_token(&issued.token).await.is_ok());
        }
    }
    for unavailable in [false, true] {
        repository.set_native_repository_failure(Some("get_published_flow_run"), unavailable);
        let error = native
            .get_native_run_for_actor(actor.clone(), uuid::Uuid::nil())
            .await
            .unwrap_err();
        response(
            error,
            if unavailable {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            if unavailable {
                "native_repository"
            } else {
                "internal_error"
            },
        )
        .await;
        repository.set_native_repository_failure(None, false);
    }
    let missing = native
        .get_native_run_for_actor(actor, uuid::Uuid::nil())
        .await
        .unwrap_err();
    response(missing, StatusCode::NOT_FOUND, "application_run_not_found").await;
    for token in ["invalid-key", "sk-unknown-key"] {
        let error = native
            .get_native_run(GetNativeRunCommand {
                bearer_token: token.into(),
                run_id: uuid::Uuid::nil(),
            })
            .await
            .unwrap_err();
        response(error, StatusCode::UNAUTHORIZED, "not_authenticated").await;
    }
}

#[tokio::test]
async fn validation_statuses_and_contextual_service_statuses_remain_distinct() {
    for (error, status, code) in [
        (
            NativeRunValidationError::InvalidMapping,
            StatusCode::BAD_REQUEST,
            "invalid_mapping",
        ),
        (
            NativeRunValidationError::NotFound,
            StatusCode::NOT_FOUND,
            "application_run_not_found",
        ),
        (
            NativeRunValidationError::InvalidState,
            StatusCode::CONFLICT,
            "invalid_state",
        ),
        (
            NativeRunValidationError::IdempotencyConflict,
            StatusCode::CONFLICT,
            "idempotency_conflict",
        ),
        (
            NativeRunValidationError::ApplicationNotPublished,
            StatusCode::CONFLICT,
            "application_not_published",
        ),
    ] {
        response(error.into(), status, code).await;
    }
    for contextual in [false, true] {
        let source = anyhow::Error::from(ControlPlaneError::UpstreamUnavailable("dependency"));
        let source = if contextual {
            source.context("outer source")
        } else {
            source
        };
        response(
            NativeRunError::Service(source),
            StatusCode::SERVICE_UNAVAILABLE,
            "dependency",
        )
        .await;
        response(
            NativeRunError::Service(
                anyhow::Error::from(std::io::Error::other("not_authenticated"))
                    .context("repository"),
            ),
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
        )
        .await;
    }
}
