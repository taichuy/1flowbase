use super::*;

#[tokio::test]
async fn start_anthropic_run_does_not_dispatch_by_legacy_protocol_mode() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Anthropic Session App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let first = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("hi"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(first.id);

    let second = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_request("new message"),
        })
        .await
        .unwrap();

    assert_ne!(first.id, second.id);
    let first_run = repository
        .get_flow_run(application.id, first.id)
        .await
        .unwrap()
        .expect("first run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(first_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let first_run_events = repository.run_event_types(first.id);
    assert!(!first_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!first_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_anthropic_subagent_run_keeps_parent_waiting_callback_alive() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Anthropic Subagent App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let parent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads\\test-01.png 找一下这幅图相关代码"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(parent.id);

    let subagent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_subagent_request("Find nav bar code"),
        })
        .await
        .unwrap();

    assert_ne!(parent.id, subagent.id);
    let parent_run = repository
        .get_flow_run(application.id, parent.id)
        .await
        .unwrap()
        .expect("parent run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(parent_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let parent_run_events = repository.run_event_types(parent.id);
    assert!(!parent_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!parent_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_anthropic_builtin_agent_run_keeps_parent_waiting_callback_alive() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Anthropic Builtin Agent App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let parent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads/image-1.png 这部分代码在哪里？"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(parent.id);

    let agent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_builtin_agent_request(
                "在 /home/taichu/git/1flowbase 项目中，找到工作台页面相关的前端代码。",
            ),
        })
        .await
        .unwrap();

    assert_ne!(parent.id, agent.id);
    let parent_run = repository
        .get_flow_run(application.id, parent.id)
        .await
        .unwrap()
        .expect("parent run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(parent_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let parent_run_events = repository.run_event_types(parent.id);
    assert!(!parent_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!parent_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_anthropic_tool_result_continuation_keeps_parent_waiting_callback_alive() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application =
        harness.seed_application(actor_user_id(), "Anthropic Tool Result Continuation App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let parent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads\\test-01.png 找一下这幅图相关代码"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(parent.id);

    let continuation = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_request(
                "-rw-r--r-- 1 Lw 197121 17907 Jun 12 15:25 uploads/test-01.png",
            ),
        })
        .await
        .unwrap();

    assert_ne!(parent.id, continuation.id);
    let parent_run = repository
        .get_flow_run(application.id, parent.id)
        .await
        .unwrap()
        .expect("parent run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(parent_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let parent_run_events = repository.run_event_types(parent.id);
    assert!(!parent_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!parent_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_anthropic_claude_code_control_run_keeps_parent_waiting_callback_alive() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application =
        harness.seed_application(actor_user_id(), "Anthropic Claude Code Control App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let parent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads/image-1.png 这部分代码在哪里？"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(parent.id);

    let control = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_claude_code_control_request(
                "CRITICAL: Respond with TEXT ONLY. Do NOT call any tools.",
                "compact_summary",
            ),
        })
        .await
        .unwrap();

    assert_ne!(parent.id, control.id);
    let parent_run = repository
        .get_flow_run(application.id, parent.id)
        .await
        .unwrap()
        .expect("parent run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(parent_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let parent_run_events = repository.run_event_types(parent.id);
    assert!(!parent_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!parent_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_anthropic_away_summary_run_keeps_parent_waiting_callback_alive() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Anthropic Away Summary App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let parent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads/image-1.png 这部分代码在哪里？"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(parent.id);

    let control = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_away_summary_request(),
        })
        .await
        .unwrap();

    assert_ne!(parent.id, control.id);
    let parent_run = repository
        .get_flow_run(application.id, parent.id)
        .await
        .unwrap()
        .expect("parent run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(parent_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let parent_run_events = repository.run_event_types(parent.id);
    assert!(!parent_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!parent_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_anthropic_compact_resume_run_keeps_parent_waiting_callback_alive() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Anthropic Compact Resume App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let parent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads/image-1.png 这部分代码在哪里？"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(parent.id);

    let control = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_compact_resume_request(),
        })
        .await
        .unwrap();

    assert_ne!(parent.id, control.id);
    let parent_run = repository
        .get_flow_run(application.id, parent.id)
        .await
        .unwrap()
        .expect("parent run should remain durable");
    let callback_task = repository
        .get_published_callback_task(callback_task.id)
        .await
        .unwrap()
        .expect("callback task should remain durable");
    assert_eq!(parent_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
    let parent_run_events = repository.run_event_types(parent.id);
    assert!(!parent_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!parent_run_events.contains(&"public_run_callback_cancelled".to_string()));
}

#[tokio::test]
async fn start_anthropic_compact_resume_run_does_not_dispatch_by_prompt_marker() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application =
        harness.seed_application(actor_user_id(), "Anthropic Compact Control Cleanup App");
    let token = issue_key(&harness, application.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository.clone());

    let parent = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_request("uploads/image-1.png 这部分代码在哪里？"),
        })
        .await
        .unwrap();
    let parent_callback = repository.seed_pending_callback_task(parent.id);
    let old_control = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token.clone(),
            request: anthropic_compact_resume_request(),
        })
        .await
        .unwrap();
    let old_control_callback = repository.seed_pending_callback_task(old_control.id);

    let next_control = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_compact_resume_request(),
        })
        .await
        .unwrap();

    assert_ne!(old_control.id, next_control.id);
    let parent_run = repository
        .get_flow_run(application.id, parent.id)
        .await
        .unwrap()
        .expect("parent run should remain durable");
    let parent_callback = repository
        .get_published_callback_task(parent_callback.id)
        .await
        .unwrap()
        .expect("parent callback task should remain durable");
    assert_eq!(parent_run.status, domain::FlowRunStatus::WaitingCallback);
    assert_eq!(parent_callback.status, domain::CallbackTaskStatus::Pending);
    let old_control_run = repository
        .get_flow_run(application.id, old_control.id)
        .await
        .unwrap()
        .expect("old control run should remain durable");
    let old_control_callback = repository
        .get_published_callback_task(old_control_callback.id)
        .await
        .unwrap()
        .expect("old control callback task should remain durable");
    assert_eq!(
        old_control_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    assert_eq!(
        old_control_callback.status,
        domain::CallbackTaskStatus::Pending
    );
    let parent_run_events = repository.run_event_types(parent.id);
    assert!(!parent_run_events.contains(&"public_run_cancelled".to_string()));
    assert!(!parent_run_events.contains(&"public_run_callback_cancelled".to_string()));
    let old_control_events = repository.run_event_types(old_control.id);
    assert!(!old_control_events.contains(&"public_run_cancelled".to_string()));
    assert!(!old_control_events.contains(&"public_run_callback_cancelled".to_string()));
}
