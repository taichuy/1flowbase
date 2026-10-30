use super::*;
use control_plane::application_public_api::{
    native::{ApplicationNativeRunService, NativeRunError},
    protocol_translation::TranslationProtocol,
};
use control_plane::errors::ControlPlaneError;

fn assert_service_source(error: NativeRunError, operation: &str, unavailable: bool) {
    let NativeRunError::Service(source) = error else {
        panic!("{operation}: dependency failure was replaced by business validation: {error:?}");
    };
    assert_eq!(
        source.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::Other
    );
    assert_eq!(source.to_string(), operation);
    assert_eq!(
        source.downcast_ref::<ControlPlaneError>().is_some(),
        unavailable
    );
    assert!(source.chain().count() >= 2);
}

async fn check_start_dependency(operation: &'static str) {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Dependency regression");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let actor = ApplicationApiKeyService::new(repository.clone())
        .authenticate_bearer_token(&token)
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());
    for unavailable in [false, true] {
        let mut request = native_request(
            "blocking",
            Some(if unavailable {
                "dependency-typed"
            } else {
                "dependency-untyped"
            }),
        );
        if operation == "create_published_flow_run_superseding_callback_predecessors" {
            request = super::native_start::responses_request_for_turn(
                "fault-thread",
                "fault-turn",
                "compaction",
                json!("summarize"),
            );
        }
        repository.set_native_repository_failure(Some(operation), unavailable);
        let error = service
            .start_native_run_for_actor(actor.clone(), request.clone(), TranslationProtocol::Native)
            .await
            .unwrap_err();
        assert_service_source(error, operation, unavailable);
        repository.set_native_repository_failure(None, false);
        assert!(service
            .start_native_run_for_actor(actor.clone(), request, TranslationProtocol::Native)
            .await
            .is_ok());
    }
}

macro_rules! start_fault {
    ($name:ident) => {
        #[tokio::test]
        async fn $name() {
            check_start_dependency(stringify!($name)).await;
        }
    };
}
start_fault!(get_application);
start_fault!(load_active_application_publication);
start_fault!(get_application_compiled_plan);
start_fault!(bind_application_public_conversation);
start_fault!(list_application_public_conversation_messages);
start_fault!(find_published_flow_run_by_idempotency_key);
start_fault!(list_application_environment_variables);
start_fault!(create_published_flow_run);
start_fault!(create_published_flow_run_superseding_callback_predecessors);

#[tokio::test]
async fn native_read_and_cancel_dependencies_preserve_sources_and_recover() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application =
        harness.seed_application(actor_user_id(), "Read cancel dependency regression");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let actor = ApplicationApiKeyService::new(repository.clone())
        .authenticate_bearer_token(&token)
        .await
        .unwrap();
    let start = ApplicationPublishedRunService::new(repository.clone());
    let native = ApplicationNativeRunService::new(repository.clone());
    for operation in [
        "get_published_flow_run",
        "find_published_flow_run_by_provider_response_id",
        "get_published_run_stream_state",
        "cancel_published_flow_run",
        "cancel_published_pending_callback_tasks_for_run",
        "cancel_published_callback_resume_attempts_for_run",
        "append_published_run_event",
    ] {
        for unavailable in [false, true] {
            let run = start
                .start_native_run_for_actor(
                    actor.clone(),
                    native_request("blocking", None),
                    TranslationProtocol::Native,
                )
                .await
                .unwrap();
            if operation == "append_published_run_event" {
                repository.seed_pending_callback_task(run.id);
            }
            repository.set_native_repository_failure(Some(operation), unavailable);
            let error = match operation {
                "find_published_flow_run_by_provider_response_id" => native
                    .get_native_run_by_provider_response_id_for_actor(actor.clone(), "resp-fault")
                    .await
                    .unwrap_err(),
                "get_published_flow_run" | "get_published_run_stream_state" => native
                    .get_native_run_for_actor(actor.clone(), run.id)
                    .await
                    .unwrap_err(),
                _ => native
                    .cancel_native_run_for_actor(actor.clone(), run.id)
                    .await
                    .unwrap_err(),
            };
            assert_service_source(error, operation, unavailable);
            repository.set_native_repository_failure(None, false);
            assert!(native
                .get_native_run_for_actor(actor.clone(), run.id)
                .await
                .is_ok());
            // Post-commit cleanup failures must not undo cancellation or make a repeated
            // state transition legal. Verify a distinct command recovers after the fault.
            let recovered = start
                .start_native_run_for_actor(
                    actor.clone(),
                    native_request("blocking", None),
                    TranslationProtocol::Native,
                )
                .await
                .unwrap();
            assert!(native
                .cancel_native_run_for_actor(actor.clone(), recovered.id)
                .await
                .is_ok());
        }
    }
    let missing: NativeRunError = native
        .get_native_run_for_actor(actor, Uuid::nil())
        .await
        .unwrap_err();
    assert!(matches!(
        missing,
        NativeRunError::Validation(NativeRunValidationError::NotFound)
    ));
}
