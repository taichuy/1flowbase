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

async fn publish_application(
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
}

fn anthropic_request(query: &str) -> NativeRunRequest {
    let mut request: NativeRunRequest = serde_json::from_value(json!({
        "query": query,
        "model": "1flowbase",
        "conversation": {
            "user": "claude-code-user",
            "id": "claude-code-session"
        },
        "response_mode": "streaming",
        "compatibility_mode": "anthropic-messages-v1"
    }))
    .unwrap();
    request.protocol_compatibility_mode = Some("anthropic-messages-v1".to_string());
    request
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
    publish_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let first = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: serde_json::from_value(json!({ "query": "First" })).unwrap(),
        })
        .await
        .unwrap();
    let second = service
        .create_native_run(CreateNativeRunCommand {
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
    publish_application(&harness, first_application.id, actor_user_id()).await;
    publish_application(&harness, second_application.id, other_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let run = service
        .create_native_run(CreateNativeRunCommand {
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
    publish_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let service = ApplicationNativeRunService::new(repository.clone());
    let run = service
        .create_native_run(CreateNativeRunCommand {
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
    publish_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let native_service = ApplicationNativeRunService::new(repository.clone());
    let run = native_service
        .create_native_run(CreateNativeRunCommand {
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
async fn anthropic_agent_callback_result_projects_matching_subagent_terminal_output() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Anthropic Agent Resume App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let repository = harness.repository();
    let run_service = ApplicationPublishedRunService::new(repository.clone());
    let parent = run_service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads/image-1.png 这部分代码在哪里？"),
        })
        .await
        .unwrap();
    let agent_prompt = "Search the 1flowbase frontend for the navigation code.";
    let parent_callback = repository.seed_pending_llm_tool_callback_task(
        parent.id,
        json!({
            "tool_calls": [
                {
                    "id": "call_agent_1",
                    "name": "Agent",
                    "arguments": {
                        "description": "Find navigation code",
                        "prompt": agent_prompt,
                        "subagent_type": "Explore"
                    }
                }
            ]
        }),
    );
    let subagent = run_service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_builtin_agent_request(agent_prompt),
        })
        .await
        .unwrap();
    let subagent_callback = repository.seed_pending_llm_tool_callback_task(
        subagent.id,
        json!({
            "tool_calls": [
                {
                    "id": "call_read_1",
                    "name": "Read",
                    "arguments": { "file_path": "/home/taichu/git/1flowbase/web/app/src/app-shell/Navigation.tsx" }
                }
            ]
        }),
    );
    let agent_report = "Navigation lives in web/app/src/app-shell/Navigation.tsx.";
    let consumer = RecordingCallbackConsumer {
        repository: repository.clone(),
        ..RecordingCallbackConsumer::default()
    };

    ApplicationPublishedCallbackResumeService::new(repository.clone(), consumer)
        .resume_callback(ResumePublishedCallbackCommand {
            bearer_token: token,
            target: PublishedCallbackResumeTarget::FlowRun {
                flow_run_id: parent.id,
                callback_task_id: parent_callback.id,
            },
            source: PublishedCallbackResumeSource::AnthropicMessages,
            response_payload: json!({
                "tool_results": [
                    {
                        "tool_call_id": "call_agent_1",
                        "content": agent_report
                    }
                ]
            }),
            response_mode: Some("streaming".into()),
        })
        .await
        .unwrap();

    let projected = repository
        .get_flow_run(application.id, subagent.id)
        .await
        .unwrap()
        .expect("subagent run should remain durable");
    assert_eq!(projected.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(projected.output_payload["answer"], json!(agent_report));
    assert!(projected.finished_at.is_some());
    let projected_node = repository
        .get_node_run(subagent_callback.node_run_id)
        .expect("subagent waiting node should remain durable");
    assert_eq!(projected_node.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(projected_node.output_payload["answer"], json!(agent_report));
    assert!(projected_node.finished_at.is_some());
    let subagent_callback = repository
        .get_published_callback_task(subagent_callback.id)
        .await
        .unwrap()
        .expect("subagent callback task should remain durable");
    assert_eq!(
        subagent_callback.status,
        domain::CallbackTaskStatus::Cancelled
    );
    let subagent_events = repository.run_event_types(subagent.id);
    assert!(subagent_events.contains(&"public_run_internal_agent_result_projected".to_string()));
    assert!(subagent_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn native_cancel_clears_pending_callback_required_action() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Cancel Pending Callback App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_application(&harness, application.id, actor_user_id()).await;
    let repository = harness.repository();
    let native_service = ApplicationNativeRunService::new(repository.clone());
    let run = native_service
        .create_native_run(CreateNativeRunCommand {
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
