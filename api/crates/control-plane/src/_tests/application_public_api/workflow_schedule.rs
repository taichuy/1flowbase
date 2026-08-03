use super::*;

#[tokio::test]
async fn workflow_schedule_trigger_service_replaces_config_and_rejects_invalid_values() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Scheduled Workflow");
    let service = WorkflowScheduleTriggerService::new(harness.repository());

    let stored = service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "Asia/Shanghai".into(),
            input_payload: serde_json::json!({
                "node-workflow-start": { "customer_id": "C-42" }
            }),
        })
        .await
        .unwrap();
    let loaded = service
        .get_trigger(GetWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap()
        .expect("schedule trigger should be stored");

    assert!(stored.enabled);
    assert_eq!(stored.cron, "0 9 * * *");
    assert_eq!(loaded.timezone, "Asia/Shanghai");

    assert!(service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "bad cron".into(),
            timezone: "Asia/Shanghai".into(),
            input_payload: serde_json::json!({}),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("cron"));
    assert!(service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "Mars/Olympus".into(),
            input_payload: serde_json::json!({}),
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("timezone"));
}

#[tokio::test]
async fn workflow_schedule_trigger_dispatch_creates_traceable_async_run_and_task() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Scheduled Workflow");
    harness.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Schedule);
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .unwrap();
    repository.set_active_publication_document_snapshot(
        application.id,
        workflow_schedule_start_contract_document(),
    );
    let schedule_trigger = service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({
                "customer_id": "C-42",
                "enabled": "true"
            }),
        })
        .await
        .unwrap();
    let task_queue = RecordingTaskQueue::default();
    let scheduled_at = time::OffsetDateTime::UNIX_EPOCH + Duration::hours(9);

    let outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at: scheduled_at + Duration::seconds(20),
            },
            Some(&task_queue),
        )
        .await
        .unwrap();
    let WorkflowScheduleDispatchOutcome::Dispatched(dispatched) = outcome else {
        panic!("enabled workflow schedule should dispatch");
    };
    let stored = repository
        .get_flow_run(application.id, dispatched.run_id)
        .await
        .unwrap()
        .expect("scheduled run should be durable");
    let enqueued = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .clone();

    assert_eq!(dispatched.status, domain::FlowRunStatus::Queued);
    assert_eq!(dispatched.task_id.as_deref(), Some("task-1"));
    assert_eq!(stored.run_mode, domain::FlowRunMode::WorkflowScheduleRun);
    assert_eq!(stored.api_key_id, None);
    assert_eq!(
        stored.external_trace_id.as_deref(),
        Some(format!("workflow-schedule:{}", schedule_trigger.id).as_str())
    );
    assert_eq!(stored.compatibility_mode, None);
    assert_eq!(
        stored.input_payload["node-workflow-start"]["customer_id"],
        serde_json::json!("C-42")
    );
    assert_eq!(
        stored.input_payload["node-workflow-start"]["attempts"],
        serde_json::json!(3)
    );
    assert_eq!(
        stored.input_payload["node-workflow-start"]["enabled"],
        serde_json::json!(true)
    );
    assert_eq!(
        stored.input_payload["trigger"],
        serde_json::json!({
            "type": "schedule",
            "scheduled_at": "1970-01-01T09:00:00Z",
            "timezone": "UTC"
        })
    );
    assert!(stored.input_payload.get("sys").is_none());
    let schedule_event = repository
        .run_events(dispatched.run_id)
        .into_iter()
        .find(|event| event.event_type == "workflow_schedule_run_enqueued")
        .expect("schedule invocation should append its trigger event");
    assert_eq!(
        schedule_event.payload["trigger_id"],
        serde_json::json!(schedule_trigger.id.to_string())
    );
    assert_eq!(
        schedule_event.payload["operation_id"],
        serde_json::json!(format!("workflow_schedule:{}", schedule_trigger.id))
    );
    assert_eq!(enqueued.len(), 1);
    assert_eq!(enqueued[0].0, WORKFLOW_SCHEDULE_RUN_QUEUE);
    assert_eq!(
        enqueued[0].1["flow_run_id"],
        serde_json::json!(dispatched.run_id.to_string())
    );
    assert_eq!(
        enqueued[0].2.as_deref(),
        Some(
            format!(
                "workflow-schedule:{}:{}",
                application.id,
                scheduled_at.unix_timestamp()
            )
            .as_str()
        )
    );

    let duplicate = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at,
            },
            Some(&task_queue),
        )
        .await
        .unwrap();
    let WorkflowScheduleDispatchOutcome::Dispatched(duplicate) = duplicate else {
        panic!("duplicate schedule dispatch should return existing run");
    };
    let enqueued_after_duplicate = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .clone();

    assert_eq!(duplicate.run_id, dispatched.run_id);
    assert_eq!(duplicate.task_id.as_deref(), Some("task-1"));
    assert_eq!(enqueued_after_duplicate.len(), 1);
}

#[tokio::test]
async fn workflow_schedule_retry_enqueues_existing_run_after_the_first_enqueue_fails() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Recoverable Schedule");
    harness.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Schedule);
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .unwrap();
    repository.set_active_publication_document_snapshot(
        application.id,
        workflow_schedule_start_contract_document(),
    );
    service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "0 9 * * *".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({ "customer_id": "C-42" }),
        })
        .await
        .unwrap();
    let task_queue = RecordingTaskQueue::failing_once();
    let scheduled_at = time::OffsetDateTime::UNIX_EPOCH + Duration::hours(9);

    service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at,
            },
            Some(&task_queue),
        )
        .await
        .expect_err("the injected first enqueue must fail after the run is durable");
    assert_eq!(repository.flow_run_count(), 1);

    let retry = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at,
            },
            Some(&task_queue),
        )
        .await
        .unwrap();
    let WorkflowScheduleDispatchOutcome::Dispatched(retry) = retry else {
        panic!("retry should recover delivery for the existing schedule run");
    };
    let enqueued = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .clone();

    assert_eq!(repository.flow_run_count(), 1);
    assert_eq!(task_queue.attempt_count(), 2);
    assert_eq!(retry.task_id.as_deref(), Some("task-1"));
    assert_eq!(enqueued.len(), 1);
    assert_eq!(
        enqueued[0].2.as_deref(),
        Some(
            format!(
                "workflow-schedule:{}:{}",
                application.id,
                scheduled_at.unix_timestamp()
            )
            .as_str()
        )
    );
}

#[tokio::test]
async fn workflow_schedule_dispatch_skips_invalid_typed_start_defaults() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_workflow_application(actor_user_id(), "Invalid Defaults");
    harness.set_workflow_trigger_type(application.id, domain::WorkflowTriggerType::Schedule);
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: false,
        })
        .await
        .unwrap();
    repository.set_active_publication_document_snapshot(
        application.id,
        workflow_schedule_start_contract_document(),
    );
    service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "* * * * *".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({ "customer_id": "C-42", "enabled": "yes" }),
        })
        .await
        .unwrap();

    let outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            None,
        )
        .await
        .unwrap();

    assert_eq!(
        outcome,
        WorkflowScheduleDispatchOutcome::Skipped {
            reason: "invalid_input_defaults",
        }
    );
    assert_eq!(repository.flow_run_count(), 0);
}

#[test]
fn workflow_schedule_cron_matcher_covers_five_field_expressions() {
    let at = |hour: u8, minute: u8| {
        time::OffsetDateTime::UNIX_EPOCH
            .replace_time(time::Time::from_hms(hour, minute, 0).unwrap())
    };

    // 1970-01-01 is a Thursday (day-of-week 4).
    assert!(workflow_schedule_cron_matches("* * * * *", at(9, 30)));
    assert!(workflow_schedule_cron_matches("0 9 * * *", at(9, 0)));
    assert!(!workflow_schedule_cron_matches("0 9 * * *", at(9, 1)));
    assert!(workflow_schedule_cron_matches("*/15 * * * *", at(3, 45)));
    assert!(!workflow_schedule_cron_matches("*/15 * * * *", at(3, 50)));
    assert!(workflow_schedule_cron_matches("0 9-17 * * *", at(12, 0)));
    assert!(!workflow_schedule_cron_matches("0 9-17 * * *", at(18, 0)));
    assert!(workflow_schedule_cron_matches("0 9,18 * * *", at(18, 0)));
    assert!(workflow_schedule_cron_matches("0 0 1 1 *", at(0, 0)));
    assert!(!workflow_schedule_cron_matches("0 0 2 1 *", at(0, 0)));
    assert!(workflow_schedule_cron_matches("0 0 * * 4", at(0, 0)));
    assert!(!workflow_schedule_cron_matches("0 0 * * 5", at(0, 0)));
    assert!(!workflow_schedule_cron_matches("0 9 * *", at(9, 0)));
}

#[tokio::test]
async fn workflow_schedule_tick_dispatches_only_matching_enabled_triggers() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    let publication_service = ApplicationPublicationService::new(repository.clone());
    let matching = harness.seed_workflow_application(actor_user_id(), "Matching Schedule");
    let wrong_cron = harness.seed_workflow_application(actor_user_id(), "Wrong Cron");
    let disabled = harness.seed_workflow_application(actor_user_id(), "Disabled Schedule");
    let shanghai = harness.seed_workflow_application(actor_user_id(), "Shanghai Schedule");
    let invalid_timezone = harness.seed_workflow_application(actor_user_id(), "Broken Timezone");

    for application_id in [matching.id, wrong_cron.id, shanghai.id, invalid_timezone.id] {
        publication_service
            .publish_active_version(PublishApplicationCommand {
                actor_user_id: actor_user_id(),
                application_id,
                mapping: ApplicationApiMappingConfig::default_native(),
                api_enabled: true,
            })
            .await
            .unwrap();
    }

    let seed_trigger = |application_id, enabled, cron: &str, timezone: &str| {
        service.replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id,
            enabled,
            cron: cron.into(),
            timezone: timezone.into(),
            input_payload: serde_json::json!({}),
        })
    };
    for application_id in [matching.id, wrong_cron.id, shanghai.id, invalid_timezone.id] {
        harness.set_workflow_trigger_type(application_id, domain::WorkflowTriggerType::Schedule);
    }
    seed_trigger(matching.id, true, "0 1 * * *", "UTC")
        .await
        .unwrap();
    seed_trigger(wrong_cron.id, true, "30 12 * * *", "UTC")
        .await
        .unwrap();
    seed_trigger(disabled.id, false, "0 1 * * *", "UTC")
        .await
        .unwrap();
    // 01:00 UTC is 09:00 in Asia/Shanghai.
    seed_trigger(shanghai.id, true, "0 9 * * *", "Asia/Shanghai")
        .await
        .unwrap();
    seed_trigger(
        invalid_timezone.id,
        true,
        "0 1 * * *",
        "America/Nowhere_Fake",
    )
    .await
    .unwrap();

    let task_queue = RecordingTaskQueue::default();
    let now_utc = time::OffsetDateTime::UNIX_EPOCH + Duration::hours(1) + Duration::seconds(17);

    let entries = service
        .dispatch_due_schedules(now_utc, Some(&task_queue))
        .await
        .unwrap();

    let dispatched_ids = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.outcome,
                WorkflowScheduleDispatchOutcome::Dispatched(_)
            )
        })
        .map(|entry| entry.application_id)
        .collect::<Vec<_>>();
    assert!(dispatched_ids.contains(&matching.id));
    assert!(dispatched_ids.contains(&shanghai.id));
    assert_eq!(dispatched_ids.len(), 2);
    assert!(entries.iter().any(|entry| {
        entry.application_id == invalid_timezone.id
            && entry.outcome
                == WorkflowScheduleDispatchOutcome::Skipped {
                    reason: "invalid_timezone",
                }
    }));
    assert!(!entries
        .iter()
        .any(|entry| entry.application_id == wrong_cron.id || entry.application_id == disabled.id));

    let enqueued = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .clone();
    assert_eq!(enqueued.len(), 2);

    // The same minute must not enqueue duplicates on a repeated tick.
    let repeat = service
        .dispatch_due_schedules(now_utc + Duration::seconds(20), Some(&task_queue))
        .await
        .unwrap();
    let repeat_dispatched = repeat
        .iter()
        .filter(|entry| {
            matches!(
                entry.outcome,
                WorkflowScheduleDispatchOutcome::Dispatched(_)
            )
        })
        .count();
    assert_eq!(repeat_dispatched, 2);
    let enqueued_after_repeat = task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .len();
    assert_eq!(enqueued_after_repeat, 2);
}

#[tokio::test]
async fn workflow_schedule_dispatch_skips_extension_trigger_application() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    // seed_workflow_application defaults to the extension trigger type.
    let application = harness.seed_workflow_application(actor_user_id(), "Extension Typed");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    service
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            enabled: true,
            cron: "* * * * *".into(),
            timezone: "UTC".into(),
            input_payload: serde_json::json!({}),
        })
        .await
        .unwrap();
    let task_queue = RecordingTaskQueue::default();

    let outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: application.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            Some(&task_queue),
        )
        .await
        .unwrap();

    assert_eq!(
        outcome,
        WorkflowScheduleDispatchOutcome::Skipped {
            reason: "trigger_type_mismatch",
        }
    );
    assert!(task_queue
        .enqueued
        .lock()
        .expect("recording task queue mutex poisoned")
        .is_empty());
}

#[tokio::test]
async fn workflow_schedule_trigger_dispatch_skips_disabled_or_unpublished_applications() {
    let harness = ApplicationPublicApiTestHarness::new();
    let disabled = harness.seed_workflow_application(actor_user_id(), "Disabled Schedule");
    let unpublished = harness.seed_workflow_application(actor_user_id(), "Unpublished Schedule");
    for application_id in [disabled.id, unpublished.id] {
        harness.set_workflow_trigger_type(application_id, domain::WorkflowTriggerType::Schedule);
    }
    let repository = harness.repository();
    let service = WorkflowScheduleTriggerService::new(repository.clone());
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: disabled.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    for application in [disabled.id, unpublished.id] {
        service
            .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
                actor_user_id: actor_user_id(),
                application_id: application,
                enabled: application == unpublished.id,
                cron: "0 9 * * *".into(),
                timezone: "UTC".into(),
                input_payload: serde_json::json!({}),
            })
            .await
            .unwrap();
    }

    let disabled_outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: disabled.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            None,
        )
        .await
        .unwrap();
    let unpublished_outcome = service
        .dispatch_due_schedule(
            DispatchWorkflowScheduleCommand {
                application_id: unpublished.id,
                scheduled_at: time::OffsetDateTime::UNIX_EPOCH,
            },
            None,
        )
        .await
        .unwrap();

    assert!(matches!(
        disabled_outcome,
        WorkflowScheduleDispatchOutcome::Skipped { reason }
            if reason == "disabled"
    ));
    assert!(matches!(
        unpublished_outcome,
        WorkflowScheduleDispatchOutcome::Skipped { reason }
            if reason == "application_not_published"
    ));
}
