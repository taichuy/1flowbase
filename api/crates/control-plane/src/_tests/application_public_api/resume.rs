use async_trait::async_trait;
use control_plane::application_public_api::native::NativeRunRequest;
use control_plane::application_public_api::{
    api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
    callback_resume::{
        ApplicationPublishedCallbackConsumer, ApplicationPublishedCallbackResumeService,
        CompletePublishedCallbackInput, PublishedCallbackResumeSource,
        PublishedCallbackResumeTarget, ResumePublishedCallbackCommand,
    },
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
    },
    native::{ApplicationNativeRunService, CreateNativeRunCommand},
    publications::{ApplicationPublicationService, PublishApplicationCommand},
    run_service::{ApplicationPublishedRunControlRepository, ApplicationPublishedRunService},
    ApplicationPublicApiTestHarness, ApplicationPublicApiTestRepository,
};
use control_plane::errors::ControlPlaneError;
use plugin_framework::provider_contract::NativePromptBlock;
use serde_json::json;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn actor_user_id() -> Uuid {
    Uuid::from_u128(0x11111111111111111111111111111111)
}

fn other_user_id() -> Uuid {
    Uuid::from_u128(0x22222222222222222222222222222222)
}

fn published_mapping() -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    }
}

async fn issue_key(
    harness: &ApplicationPublicApiTestHarness,
    application_id: Uuid,
    owner_user_id: Uuid,
) -> String {
    ApplicationApiKeyService::new(harness.repository())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: owner_user_id,
            application_id,
            name: "Native runner".into(),
            expires_at: None,
        })
        .await
        .unwrap()
        .token
}

async fn publish_runnable_application(
    harness: &ApplicationPublicApiTestHarness,
    application_id: Uuid,
    owner_user_id: Uuid,
) {
    ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: owner_user_id,
            application_id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    harness
        .repository()
        .configure_runnable_published_generate_route(application_id);
}

fn anthropic_request(query: &str) -> NativeRunRequest {
    serde_json::from_value(json!({
        "query": query,
        "model": "1flowbase",
        "conversation": {
            "user": "claude-code-user",
            "id": "claude-code-session"
        },
        "response_mode": "streaming"
    }))
    .unwrap()
}

fn anthropic_builtin_agent_request(query: &str) -> NativeRunRequest {
    let mut request = anthropic_request(query);
    request.system = vec![NativePromptBlock::text(
        "x-anthropic-billing-header: cc_version=2.1.141; cc_entrypoint=cli; cch=05fc2;\n\n\
You are Claude Code, Anthropic's official CLI for Claude.\n\n\
You are a file search specialist for Claude Code, Anthropic's official CLI for Claude.\n\n\
Notes:\n\
- Agent threads always have their cwd reset between bash calls, as a result please only use absolute file paths.\n\
- Do NOT Write report/summary/findings/analysis .md files. Return findings directly as your final assistant message — the parent agent reads your text output, not files you create."
            .to_string(),
    )];
    request
}

#[tokio::test]
async fn native_resume_rejects_callback_task_from_another_run() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Resume App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let first = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let second = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "Second" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(second.id);

    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };
    let error = ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer)
        .resume_callback(ResumePublishedCallbackCommand {
            bearer_token: token,
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: first.id,
                callback_task_id: callback_task.id,
            },
            source: PublishedCallbackResumeSource::NativeAgent,
            response_payload: json!({ "answer": "approved" }),
            response_mode: Some("blocking".into()),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::PermissionDenied(
            "callback_task_flow_run"
        ))
    );
}

#[tokio::test]
async fn native_resume_validates_ownership_before_execution_continuation_boundary() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_application = harness.seed_application(actor_user_id(), "Owned Resume App");
    let second_application = harness.seed_application(other_user_id(), "Other Resume App");
    let first_token = issue_key(&harness, first_application.id, actor_user_id()).await;
    let second_token = issue_key(&harness, second_application.id, other_user_id()).await;
    publish_runnable_application(&harness, first_application.id, actor_user_id()).await;
    publish_runnable_application(&harness, second_application.id, other_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: first_token,
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(run.id);

    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };
    let error = ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer)
        .resume_callback(ResumePublishedCallbackCommand {
            bearer_token: second_token,
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: run.id,
                callback_task_id: callback_task.id,
            },
            source: PublishedCallbackResumeSource::NativeAgent,
            response_payload: json!({ "answer": "approved" }),
            response_mode: Some("streaming".into()),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(&ControlPlaneError::PermissionDenied(
            "application_public_callback_resume"
        ))
    );
}

#[tokio::test]
async fn native_get_run_exposes_pending_callback_required_action() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Required Action App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let run = service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(run.id);

    let result = service
        .get_native_run(
            control_plane::application_public_api::native::GetNativeRunCommand {
                bearer_token: token,
                run_id: run.id,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        result.status,
        control_plane::application_public_api::native::NativeRunStatus::Waiting
    );
    let required_action = result
        .required_action
        .expect("pending callback should be exposed");
    assert_eq!(required_action.action_type, "callback");
    assert_eq!(
        required_action.payload["callback_task_id"],
        json!(callback_task.id)
    );
    assert_eq!(
        required_action.payload["request_payload"],
        callback_task.request_payload
    );
}

#[derive(Clone, Default)]
struct RecordingCallbackConsumer {
    repository: ApplicationPublicApiTestRepository,
    calls: Arc<Mutex<Vec<CompletePublishedCallbackInput>>>,
}

impl RecordingCallbackConsumer {
    fn calls(&self) -> Vec<CompletePublishedCallbackInput> {
        self.calls
            .lock()
            .expect("recording callback consumer mutex poisoned")
            .clone()
    }
}

#[async_trait]
impl ApplicationPublishedCallbackConsumer for RecordingCallbackConsumer {
    async fn complete_published_callback(
        &self,
        input: CompletePublishedCallbackInput,
    ) -> anyhow::Result<domain::FlowRunRecord> {
        self.calls
            .lock()
            .expect("recording callback consumer mutex poisoned")
            .push(input.clone());
        let callback_task = self
            .repository
            .get_published_callback_task(input.callback_task_id)
            .await?
            .expect("callback task should exist");
        self.repository
            .get_published_flow_run(callback_task.flow_run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("published run should exist"))
    }
}

#[tokio::test]
async fn public_callback_resume_consumes_pending_callback_in_request() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Unified Resume App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let native_service = ApplicationNativeRunService::new(repository.clone());
    let run = native_service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(run.id);
    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };

    let result =
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone())
            .resume_callback(ResumePublishedCallbackCommand {
                bearer_token: token,
                target: PublishedCallbackResumeTarget::FlowRun {
                    flow_run_id: run.id,
                    callback_task_id: callback_task.id,
                },
                source: PublishedCallbackResumeSource::NativeAgent,
                response_payload: json!({ "answer": "approved" }),
                response_mode: Some("blocking".into()),
            })
            .await
            .unwrap();

    let calls = consumer.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].application_id, application.id);
    assert_eq!(calls[0].callback_task_id, callback_task.id);
    assert_eq!(calls[0].response_payload, json!({ "answer": "approved" }));
    assert_eq!(
        result.attempt.status,
        domain::FlowRunCallbackResumeAttemptStatus::Succeeded
    );

    let attempts = repository.callback_resume_attempts();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].flow_run_id, run.id);
    assert_eq!(attempts[0].callback_task_id, callback_task.id);
    assert_eq!(attempts[0].source, "native_agent");
    assert_eq!(
        attempts[0].status,
        domain::FlowRunCallbackResumeAttemptStatus::Succeeded
    );

    let event_types = repository.run_event_types(run.id);
    assert!(event_types.contains(&"public_run_resume_requested".to_string()));
    assert!(event_types.contains(&"public_run_resume_succeeded".to_string()));
    assert!(!event_types.contains(&"public_run_resume_claimed".to_string()));
}

#[tokio::test]
async fn callback_resume_preserves_original_compatibility_mode() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Canonical Callback Resume App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let run_service = ApplicationPublishedRunService::new(repository.clone());
    let agent_prompt = "Search the 1flowbase frontend for the navigation code.";

    let absent_parent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_request("Find the navigation code"),
        })
        .await
        .unwrap();
    let absent_parent_callback = repository.seed_pending_llm_tool_callback_task(
        absent_parent.id,
        json!({
            "tool_calls": [{
                "id": "call_agent_absent",
                "name": "Agent",
                "arguments": {"prompt": agent_prompt}
            }]
        }),
    );
    let absent_subagent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_builtin_agent_request(agent_prompt),
        })
        .await
        .unwrap();
    let absent_subagent_callback = repository.seed_pending_callback_task(absent_subagent.id);

    let legacy_parent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_request("Find the navigation code"),
        })
        .await
        .unwrap();
    let legacy_parent_callback = repository.seed_pending_llm_tool_callback_task(
        legacy_parent.id,
        json!({
            "tool_calls": [{
                "id": "call_agent_legacy",
                "name": "Agent",
                "arguments": {"prompt": agent_prompt}
            }]
        }),
    );
    let legacy_subagent = run_service
        .start_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: anthropic_builtin_agent_request(agent_prompt),
        })
        .await
        .unwrap();
    let legacy_subagent_callback = repository.seed_pending_callback_task(legacy_subagent.id);
    repository
        .set_flow_run_compatibility_mode_for_test(legacy_parent.id, Some("anthropic-messages-v1"));
    repository.set_flow_run_compatibility_mode_for_test(
        legacy_subagent.id,
        Some("anthropic-messages-v1"),
    );

    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };
    for (flow_run_id, callback_task_id, tool_call_id) in [
        (
            absent_parent.id,
            absent_parent_callback.id,
            "call_agent_absent",
        ),
        (
            legacy_parent.id,
            legacy_parent_callback.id,
            "call_agent_legacy",
        ),
    ] {
        ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone())
            .resume_callback(ResumePublishedCallbackCommand {
                bearer_token: token.clone(),
                target: PublishedCallbackResumeTarget::FlowRun {
                    flow_run_id,
                    callback_task_id,
                },
                source: PublishedCallbackResumeSource::AnthropicMessages,
                response_payload: json!({
                    "tool_results": [{
                        "tool_call_id": tool_call_id,
                        "content": "Navigation lives in web/app/src/app-shell/Navigation.tsx."
                    }]
                }),
                response_mode: Some("streaming".into()),
            })
            .await
            .unwrap();
    }

    for (subagent_id, callback_task_id) in [
        (absent_subagent.id, absent_subagent_callback.id),
        (legacy_subagent.id, legacy_subagent_callback.id),
    ] {
        let subagent = repository
            .get_flow_run(application.id, subagent_id)
            .await
            .unwrap()
            .expect("subagent run should remain durable");
        let callback_task = repository
            .get_published_callback_task(callback_task_id)
            .await
            .unwrap()
            .expect("subagent callback task should remain durable");
        assert_eq!(subagent.status, domain::FlowRunStatus::WaitingCallback);
        assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
        let events = repository.run_event_types(subagent_id);
        assert!(!events.contains(&"public_run_internal_agent_result_projected".to_string()));
        assert!(!events.contains(&"public_run_callback_cancelled".to_string()));
    }

    for (flow_run_id, expected) in [
        (absent_parent.id, "native-v1"),
        (legacy_parent.id, "anthropic-messages-v1"),
    ] {
        let resumed = repository
            .get_flow_run(application.id, flow_run_id)
            .await
            .unwrap()
            .expect("resumed parent run should remain durable");
        assert_eq!(resumed.compatibility_mode.as_deref(), Some(expected));
    }
}

#[tokio::test]
async fn native_cancel_clears_pending_callback_required_action() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Cancel Pending Callback App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_runnable_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let native_service = ApplicationNativeRunService::new(repository.clone());
    let run = native_service
        .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(run.id);

    let cancelled = native_service
        .cancel_native_run(
            control_plane::application_public_api::native::CancelNativeRunCommand {
                bearer_token: token.clone(),
                run_id: run.id,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        cancelled.status,
        control_plane::application_public_api::native::NativeRunStatus::Cancelled
    );
    let detail = native_service
        .get_native_run(
            control_plane::application_public_api::native::GetNativeRunCommand {
                bearer_token: token,
                run_id: run.id,
            },
        )
        .await
        .unwrap();
    assert!(detail.required_action.is_none());
    let task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should still be durable");
    assert_eq!(task.status, domain::CallbackTaskStatus::Cancelled);
}
#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::Result;
    use serde_json::{json, Value};

    use super::*;
    use crate::application_public_api::{
        api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
        mapping::{
            ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
        },
        native::{ApplicationNativeRunService, CreateNativeRunCommand, NativeRunResult},
        publications::{ApplicationPublicationService, PublishApplicationCommand},
        ApplicationPublicApiTestHarness, ApplicationPublicApiTestRepository,
    };

    const ACTOR_USER_ID: Uuid = Uuid::from_u128(0x11111111111111111111111111111111);

    #[derive(Clone, Default)]
    struct CompletingCallbackConsumer {
        repository: ApplicationPublicApiTestRepository,
        calls: Arc<Mutex<Vec<CompletePublishedCallbackInput>>>,
    }

    impl CompletingCallbackConsumer {
        fn call_count(&self) -> usize {
            self.calls
                .lock()
                .expect("callback consumer observation lock poisoned")
                .len()
        }
    }

    #[async_trait]
    impl ApplicationPublishedCallbackConsumer for CompletingCallbackConsumer {
        async fn complete_published_callback(
            &self,
            input: CompletePublishedCallbackInput,
        ) -> Result<domain::FlowRunRecord> {
            self.calls
                .lock()
                .expect("callback consumer observation lock poisoned")
                .push(input.clone());
            let callback_task = self
                .repository
                .get_published_callback_task(input.callback_task_id)
                .await?
                .expect("callback task fixture should exist");
            self.repository
                .complete_callback_task_for_test(input.callback_task_id);
            self.repository
                .get_published_flow_run(callback_task.flow_run_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("published run fixture should exist"))
        }
    }

    async fn callback_fixture() -> (
        ApplicationPublicApiTestRepository,
        CompletingCallbackConsumer,
        String,
        NativeRunResult,
    ) {
        let harness = ApplicationPublicApiTestHarness::new();
        let application = harness.seed_application(ACTOR_USER_ID, "Callback Contract App");
        let repository = harness.repository();
        let token = ApplicationApiKeyService::new(repository.clone())
            .create_api_key(CreateApplicationApiKeyCommand {
                actor_user_id: ACTOR_USER_ID,
                application_id: application.id,
                name: "Callback contract key".to_string(),
                expires_at: None,
            })
            .await
            .expect("create callback contract API key")
            .token;
        ApplicationPublicationService::new(repository.clone())
            .publish_active_version(PublishApplicationCommand {
                actor_user_id: ACTOR_USER_ID,
                application_id: application.id,
                mapping: ApplicationApiMappingConfig {
                    input: ApplicationApiMappingInput {
                        query_target: "node-start.query".to_string(),
                        model_target: None,
                        inputs_target: None,
                        history_target: None,
                        attachments_target: None,
                    },
                    output: ApplicationApiMappingOutput::default(),
                    extension: None,
                },
                api_enabled: true,
            })
            .await
            .expect("publish callback contract application");
        repository.configure_runnable_published_generate_route(application.id);
        let run = ApplicationNativeRunService::new(repository.clone())
            .create_native_run(CreateNativeRunCommand {
            protocol: control_plane::application_public_api::protocol_translation::TranslationProtocol::Native,
                bearer_token: token.clone(),
                request: serde_json::from_value(json!({ "query": "First" }))
                    .expect("native request fixture should deserialize"),
            })
            .await
            .expect("create callback contract run");
        let consumer = CompletingCallbackConsumer {
            repository: repository.clone(),
            ..CompletingCallbackConsumer::default()
        };
        (repository, consumer, token, run)
    }

    fn resume_command(
        token: &str,
        run_id: Uuid,
        callback_task_id: Uuid,
        response_payload: Value,
    ) -> ResumePublishedCallbackCommand {
        ResumePublishedCallbackCommand {
            bearer_token: token.to_string(),
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: run_id,
                callback_task_id,
            },
            source: PublishedCallbackResumeSource::NativeAgent,
            response_payload,
            response_mode: Some("blocking".to_string()),
        }
    }

    #[tokio::test]
    async fn ac_007_same_callback_payload_replays_without_a_second_consumer_call() {
        let (repository, consumer, token, run) = callback_fixture().await;
        let callback = repository.seed_pending_callback_task(run.id);
        let service =
            ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
        let command = resume_command(&token, run.id, callback.id, json!({ "answer": "yes" }));

        let first = service
            .resume_callback(command.clone())
            .await
            .expect("first callback should complete");
        let replay = service
            .resume_callback(command)
            .await
            .expect("same callback payload should replay idempotently");

        assert_eq!(consumer.call_count(), 1);
        assert_eq!(replay.attempt.id, first.attempt.id);
        assert_eq!(repository.callback_resume_attempts().len(), 1);
    }

    #[tokio::test]
    async fn ac_007_changed_callback_payload_is_a_conflict() {
        let (repository, consumer, token, run) = callback_fixture().await;
        let callback = repository.seed_pending_callback_task(run.id);
        let service = ApplicationPublishedCallbackResumeService::new(repository, consumer.clone());
        service
            .resume_callback(resume_command(
                &token,
                run.id,
                callback.id,
                json!({ "answer": "yes" }),
            ))
            .await
            .expect("first callback should complete");

        let error = service
            .resume_callback(resume_command(
                &token,
                run.id,
                callback.id,
                json!({ "answer": "no" }),
            ))
            .await
            .expect_err("changed callback payload must conflict");

        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::Conflict(
                "callback_resume_payload_conflict"
            ))
        ));
        assert_eq!(consumer.call_count(), 1);
    }

    #[tokio::test]
    async fn ac_009_sequential_callbacks_have_distinct_attempts_and_terminals() {
        let (repository, consumer, token, run) = callback_fixture().await;
        let service =
            ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer.clone());
        let first_callback = repository.seed_pending_callback_task(run.id);
        let first = service
            .resume_callback(resume_command(
                &token,
                run.id,
                first_callback.id,
                json!({ "step": 1 }),
            ))
            .await
            .expect("first callback should complete");
        let second_callback = repository.seed_pending_callback_task(run.id);
        let second = service
            .resume_callback(resume_command(
                &token,
                run.id,
                second_callback.id,
                json!({ "step": 2 }),
            ))
            .await
            .expect("second callback should complete");

        assert_eq!(consumer.call_count(), 2);
        assert_ne!(first.attempt.id, second.attempt.id);
        assert_ne!(
            first.attempt.callback_task_id,
            second.attempt.callback_task_id
        );
        assert_eq!(
            repository
                .run_event_types(run.id)
                .iter()
                .filter(|event| event.as_str() == "public_run_resume_succeeded")
                .count(),
            2
        );
    }
}
