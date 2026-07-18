use super::*;

/// Root #1366 AC-003 / AC-005: unbound Generate fails before capability lookup or run creation.
#[tokio::test]
async fn generate_unbound_fails_closed_without_run_or_provider_capability_lookup() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Unbound Generate App");
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

    let error = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: native_request("blocking", None),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error,
        NativeRunValidationError::RouteUnavailable(PublishedRouteResolutionError::OperationUnbound)
    );
    assert_eq!(repository.flow_run_count(), 0);
    assert_eq!(repository.published_generate_capability_checks(), 0);
}

/// Root #1366 AC-003 / AC-005: capability mismatch is rejected before any durable run exists.
#[tokio::test]
async fn generate_capability_mismatch_fails_closed_without_run_or_provider_spawn() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Capability Mismatch App");
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
    repository.configure_published_generate_route(
        application.id,
        "node-frozen-llm",
        published_llm_runtime(),
    );
    repository.set_published_generate_capability_supported(false);

    let error = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: native_request("blocking", None),
        })
        .await
        .unwrap_err();

    assert_eq!(
        error,
        NativeRunValidationError::RouteUnavailable(
            PublishedRouteResolutionError::ProviderCapabilityMismatch
        )
    );
    assert_eq!(repository.flow_run_count(), 0);
    assert_eq!(repository.published_generate_capability_checks(), 1);
}

/// Root #1366 AC-003 / AC-006: standard and local-summary Generate resolve one frozen target.
#[tokio::test]
async fn generate_profiles_ignore_draft_mutation_and_resolve_the_same_frozen_target() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Frozen Generate Route App");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let frozen_runtime = published_llm_runtime();
    repository.configure_published_generate_route(
        application.id,
        "node-frozen-llm",
        frozen_runtime.clone(),
    );
    let publication_before_draft_mutation = repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .unwrap();
    let mut changed_draft_mapping = published_mapping();
    changed_draft_mapping.input.query_target = "node-draft.query".into();
    ApplicationApiMappingService::new(repository.clone())
        .replace_mapping_draft(
            ReplaceApplicationApiMappingCommand {
                actor_user_id: actor_user_id(),
                application_id: application.id,
                mapping: changed_draft_mapping,
            },
            Some(ApplicationOperationBindings {
                generate: Some(ApplicationOperationTargetBinding {
                    target_node_id: "node-draft-llm".into(),
                }),
                ..ApplicationOperationBindings::default()
            }),
        )
        .await
        .unwrap();
    let publication = repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .unwrap();
    let compiled_plan = repository
        .get_application_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(publication, publication_before_draft_mutation);
    assert_eq!(
        publication.mapping_snapshot.input.query_target,
        "node-start.query"
    );
    assert_eq!(
        publication
            .operation_bindings
            .generate
            .as_ref()
            .unwrap()
            .target_node_id,
        "node-frozen-llm"
    );
    let resolver = PublishedRouteResolver::new(&repository);

    let standard = resolver
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::OperationBinding,
            GenerateExecutionProfile::Standard,
        )
        .await
        .unwrap();
    let local_summary = resolver
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::OperationBinding,
            GenerateExecutionProfile::LocalSummary,
        )
        .await
        .unwrap();

    for route in [standard, local_summary] {
        let ResolvedPublishedRoute::Provider(route) = route else {
            panic!("Generate binding must resolve a provider target");
        };
        assert_eq!(route.target_node_id, "node-frozen-llm");
        assert_eq!(route.llm_runtime, frozen_runtime);
    }
}

/// Root #1366 AC-003 / AC-005: stale target identity and incomplete runtime fail before capability.
#[tokio::test]
async fn generate_target_mismatch_and_incomplete_runtime_fail_before_capability_lookup() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Invalid Frozen Route App");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    repository.configure_published_generate_route(
        application.id,
        "node-frozen-llm",
        published_llm_runtime(),
    );
    let publication = repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .unwrap();
    let compiled_plan = repository
        .get_application_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .unwrap();
    let resolver = PublishedRouteResolver::new(&repository);

    let mut missing_target_publication = publication.clone();
    missing_target_publication
        .operation_bindings
        .generate
        .as_mut()
        .unwrap()
        .target_node_id = "missing-node".into();
    let missing = resolver
        .resolve_generate(
            application.workspace_id,
            &missing_target_publication,
            &compiled_plan,
            PublishedRouteDispatch::OperationBinding,
            GenerateExecutionProfile::Standard,
        )
        .await
        .unwrap_err();

    let mut incomplete_plan = compiled_plan.clone();
    incomplete_plan.plan["nodes"]["node-frozen-llm"]["llm_runtime"]["model"] =
        serde_json::json!("");
    let incomplete = resolver
        .resolve_generate(
            application.workspace_id,
            &publication,
            &incomplete_plan,
            PublishedRouteDispatch::OperationBinding,
            GenerateExecutionProfile::Standard,
        )
        .await
        .unwrap_err();

    assert_eq!(missing, PublishedRouteResolutionError::TargetMissing);
    assert_eq!(
        incomplete,
        PublishedRouteResolutionError::IncompleteLlmRuntime
    );
    assert_eq!(repository.published_generate_capability_checks(), 0);
}

/// Root #1366 AC-003: an explicitly selected application-flow route is typed and binding-free.
#[tokio::test]
async fn explicit_application_flow_dispatch_returns_the_frozen_compiled_plan() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Application Flow Dispatch App");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let publication = repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .unwrap();
    let compiled_plan = repository
        .get_application_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .unwrap();

    let route = PublishedRouteResolver::new(&repository)
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::ApplicationFlow,
            GenerateExecutionProfile::Standard,
        )
        .await
        .unwrap();

    assert_eq!(
        route,
        ResolvedPublishedRoute::ApplicationFlow {
            compiled_plan_id: publication.compiled_plan_id,
        }
    );
    assert_eq!(repository.published_generate_capability_checks(), 0);
}

#[tokio::test]
async fn native_execution_compatibility_mode_is_rejected_without_mutating_waiting_callback() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Native Forged Compat App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    let service = ApplicationPublishedRunService::new(repository.clone());

    let first = service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: anthropic_request("hi"),
        })
        .await
        .unwrap();
    let callback_task = repository.seed_pending_callback_task(first.id);

    let flow_runs_before_rejection = repository.flow_run_count();
    let rejection = translate_native_run_request(json!({
        "query": "Native caller should not own Anthropic cancellation policy",
        "model": "public-model/pass-through",
        "conversation": {
            "id": "3e7058c2-3120-4222-bb14-c99ec85e1c0f",
            "user": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "response_mode": "blocking",
        "execution": {
            "compatibility_mode": "anthropic-messages-v1"
        }
    }))
    .expect_err("Native execution compatibility_mode must be rejected by the protocol adapter");

    assert_eq!(rejection.code, "compatibility_mode");
    assert!(rejection.report.has_decision(
        "$.execution.compatibility_mode",
        control_plane::application_public_api::protocol_translation::TranslationDecisionKind::Unsupported
    ));
    assert_eq!(repository.flow_run_count(), flow_runs_before_rejection);
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
async fn start_native_run_does_not_read_editor_state_after_publication() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Frozen Native App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    repository.reset_editor_state_read_count();
    let service = ApplicationPublishedRunService::new(repository.clone());

    service
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: native_request("streaming", None),
        })
        .await
        .unwrap();

    assert_eq!(repository.editor_state_read_count(), 0);
}

#[tokio::test]
async fn start_native_run_returns_application_not_published_for_unpublished_or_disabled_application(
) {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let unpublished = harness.seed_application(actor_user_id(), "Unpublished App");
    let unpublished_token = issue_key(&harness, unpublished.id).await;
    let disabled = harness.seed_application(actor_user_id(), "Disabled App");
    let disabled_token = issue_key(&harness, disabled.id).await;
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: disabled.id,
            mapping: published_mapping(),
            api_enabled: false,
        })
        .await
        .unwrap();
    let service = ApplicationPublishedRunService::new(repository);

    for token in [unpublished_token, disabled_token] {
        let error = service
            .start_native_run(CreateNativeRunCommand {
                bearer_token: token,
                request: native_request("blocking", None),
            })
            .await
            .unwrap_err();

        assert_eq!(error, NativeRunValidationError::ApplicationNotPublished);
    }
}
