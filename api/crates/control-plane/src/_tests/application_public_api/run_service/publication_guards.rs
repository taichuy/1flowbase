use std::collections::BTreeSet;

use plugin_framework::provider_contract::ProviderInvocationCapability;

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
async fn generate_end_user_reference_capability_mismatch_fails_closed_without_run_or_provider_spawn(
) {
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
    repository.set_published_generate_manifest_capabilities(BTreeSet::new());
    let mut request = native_request("blocking", None);
    request.request_context.end_user_reference = Some("external-user-123".to_string());

    let error = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request,
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
    assert_eq!(
        repository.published_generate_capability_requirements(),
        vec![BTreeSet::from([
            ProviderInvocationCapability::EndUserReference
        ])]
    );
}

/// D4-AC-002: opaque Responses input cannot create a run unless every reachable target opts in.
#[tokio::test]
async fn d4_ac_002_native_responses_passthrough_requirement_fails_before_run_creation() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Responses Admission App");
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
    repository.set_published_generate_manifest_capabilities(BTreeSet::new());
    let mut request = native_request("blocking", None);
    request.metadata.set_responses_transport_requirement(
        control_plane::application_public_api::native::ResponsesTransportRequirement::NativePassthrough,
    );

    let error = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request,
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
    assert_eq!(
        repository.published_generate_capability_requirements(),
        vec![BTreeSet::from([
            ProviderInvocationCapability::ResponsesNativePassthrough
        ])]
    );
}

/// D4-AC-002: the existing all-target manifest check admits a fully capable provider route.
#[tokio::test]
async fn d4_ac_002_native_responses_passthrough_all_targets_capable_is_admitted() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Responses Native Route App");
    let token = issue_key(&harness, application.id).await;
    publish_runnable_application(&repository, application.id).await;
    repository.set_published_generate_manifest_capabilities(BTreeSet::from([
        ProviderInvocationCapability::ResponsesNativePassthrough,
    ]));
    let mut request = native_request("blocking", None);
    request.metadata.set_responses_transport_requirement(
        control_plane::application_public_api::native::ResponsesTransportRequirement::NativePassthrough,
    );

    ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request,
        })
        .await
        .expect("all reachable targets declare native Responses passthrough");

    assert_eq!(repository.flow_run_count(), 1);
    assert_eq!(
        repository.published_generate_capability_requirements(),
        vec![BTreeSet::from([
            ProviderInvocationCapability::ResponsesNativePassthrough
        ])]
    );
}

#[tokio::test]
async fn d5_ac_005_native_responses_rejects_cross_provider_failover_before_run_creation() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Responses Pinned Route App");
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
    let mut runtime = published_llm_runtime();
    runtime.routing = Some(orchestration_runtime::compiled_plan::CompiledLlmRouting {
        routing_mode: orchestration_runtime::compiled_plan::LlmRoutingMode::FailoverQueue,
        fixed_model_target: None,
        queue_template_id: None,
        queue_snapshot_id: Some("snapshot-native".into()),
        queue_targets: vec![
            orchestration_runtime::compiled_plan::CompiledLlmRouteTarget {
                provider_instance_id: Uuid::now_v7().to_string(),
                provider_instance_display_name: "Different Provider".into(),
                provider_code: "other_provider".into(),
                protocol: "openai_responses".into(),
                upstream_model_id: "other-model".into(),
            },
        ],
        distribution_rule:
            orchestration_runtime::compiled_plan::LlmDistributionRule::RetryRoundRobin,
        distribution_key: None,
        context_policy: json!({"integration_context": "enabled"}),
        stream_policy: json!({}),
    });
    repository.configure_published_generate_route(application.id, "node-frozen-llm", runtime);
    repository.set_published_generate_manifest_capabilities(BTreeSet::from([
        ProviderInvocationCapability::ResponsesNativePassthrough,
    ]));
    let mut request = native_request("blocking", None);
    request.metadata.set_responses_transport_requirement(
        control_plane::application_public_api::native::ResponsesTransportRequirement::NativePassthrough,
    );

    let error = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request,
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
    assert_eq!(repository.published_generate_capability_checks(), 0);
}

#[tokio::test]
async fn d5_ac_005_native_responses_durable_summary_records_the_provider_pin() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Responses Durable Pin App");
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
    let runtime = published_llm_runtime();
    repository.configure_published_generate_route(
        application.id,
        "node-frozen-llm",
        runtime.clone(),
    );
    repository.set_published_generate_manifest_capabilities(BTreeSet::from([
        ProviderInvocationCapability::ResponsesNativePassthrough,
    ]));
    let mut request = native_request("blocking", None);
    request.metadata.set_responses_transport_requirement(
        control_plane::application_public_api::native::ResponsesTransportRequirement::NativePassthrough,
    );
    request.metadata.set_provider_transport_payload(
        control_plane::ports::ProviderTransportPayload::openai_responses(json!({
            "model": "1flowbase",
            "input": "search",
            "tools": [{"type": "web_search", "external_web_access": false}]
        }))
        .unwrap(),
    );

    let result = ApplicationPublishedRunService::new(repository.clone())
        .start_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request,
        })
        .await
        .unwrap();
    let flow_run = repository
        .get_flow_run(application.id, result.id)
        .await
        .unwrap()
        .unwrap();
    let pin = &flow_run.input_payload["sys"]["public_provider_transport"]["provider_pin"];

    assert_eq!(pin["provider_instance_id"], runtime.provider_instance_id);
    assert_eq!(pin["provider_code"], runtime.provider_code);
    assert_eq!(pin["protocol"], runtime.protocol);
    assert_eq!(pin["upstream_model_id"], runtime.model);
    assert!(!flow_run
        .input_payload
        .to_string()
        .contains("external_web_access"));
}

/// Root #1366 AC-003 / AC-006: both Generate profiles use the frozen target and fail closed.
#[tokio::test]
async fn generate_profiles_ignore_draft_mutation_and_fail_closed_on_capability_mismatch() {
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
    let required_semantic_capabilities =
        BTreeSet::from([ProviderInvocationCapability::EndUserReference]);

    let standard = resolver
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::OperationBinding,
            AiNativeGenerateProfile::Standard,
            &required_semantic_capabilities,
        )
        .await
        .unwrap();
    let local_summary = resolver
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::OperationBinding,
            AiNativeGenerateProfile::LocalSummary,
            &required_semantic_capabilities,
        )
        .await
        .unwrap();

    for route in [standard, local_summary] {
        let ResolvedPublishedRoute::Provider(route) = route else {
            panic!("Generate binding must resolve a provider target");
        };
        assert_eq!(route.operation, ProviderWireOperation::Generate);
        assert_eq!(route.target_node_id, "node-frozen-llm");
        assert_eq!(route.llm_runtime, frozen_runtime);
    }
    assert_eq!(
        repository.published_generate_capability_profiles(),
        vec![
            AiNativeGenerateProfile::Standard,
            AiNativeGenerateProfile::LocalSummary,
        ]
    );
    assert_eq!(
        repository.published_generate_capability_requirements(),
        vec![
            required_semantic_capabilities.clone(),
            required_semantic_capabilities.clone(),
        ]
    );

    repository.set_published_generate_manifest_capabilities(BTreeSet::new());
    let mismatch = resolver
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::OperationBinding,
            AiNativeGenerateProfile::LocalSummary,
            &required_semantic_capabilities,
        )
        .await
        .unwrap_err();

    assert_eq!(
        mismatch,
        PublishedRouteResolutionError::ProviderCapabilityMismatch
    );
    assert_eq!(
        repository.published_generate_capability_profiles(),
        vec![
            AiNativeGenerateProfile::Standard,
            AiNativeGenerateProfile::LocalSummary,
            AiNativeGenerateProfile::LocalSummary,
        ]
    );
    assert_eq!(
        repository.published_generate_capability_requirements(),
        vec![
            required_semantic_capabilities.clone(),
            required_semantic_capabilities.clone(),
            required_semantic_capabilities,
        ]
    );
}

/// Root #1366 AC-003 / #1369: CountTokens resolves its frozen binding and never falls back.
#[tokio::test]
async fn count_tokens_uses_the_frozen_bound_provider_route() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Frozen CountTokens Route App");
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
    repository.configure_published_count_tokens_route(
        application.id,
        "node-frozen-count-tokens",
        frozen_runtime.clone(),
    );
    let publication_before_draft_mutation = repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .unwrap();
    ApplicationApiMappingService::new(repository.clone())
        .replace_mapping_draft(
            ReplaceApplicationApiMappingCommand {
                actor_user_id: actor_user_id(),
                application_id: application.id,
                mapping: published_mapping(),
            },
            Some(ApplicationOperationBindings {
                count_tokens: Some(ApplicationOperationTargetBinding {
                    target_node_id: "node-draft-count-tokens".into(),
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
        publication
            .operation_bindings
            .count_tokens
            .as_ref()
            .unwrap()
            .target_node_id,
        "node-frozen-count-tokens"
    );

    let resolver = PublishedRouteResolver::new(&repository);
    let route = resolver
        .resolve_count_tokens(application.workspace_id, &publication, &compiled_plan)
        .await
        .unwrap();
    assert_eq!(route.operation, ProviderWireOperation::CountTokens);
    assert_eq!(route.target_node_id, "node-frozen-count-tokens");
    assert_eq!(route.llm_runtime, frozen_runtime);
    assert_eq!(repository.published_count_tokens_capability_checks(), 1);

    repository.set_published_count_tokens_capability_supported(false);
    let mismatch = resolver
        .resolve_count_tokens(application.workspace_id, &publication, &compiled_plan)
        .await
        .unwrap_err();
    assert_eq!(
        mismatch,
        PublishedRouteResolutionError::ProviderCapabilityMismatch
    );
    assert_eq!(repository.published_count_tokens_capability_checks(), 2);
}

/// Root #1366 / K2b: remote Compact resolves only the frozen profile binding.
#[tokio::test]
async fn compact_uses_the_frozen_bound_provider_route_and_exact_profile_capability() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Frozen Compact Route App");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let profile = ProviderCompactProfile::ResponsesCompactionV2;
    let frozen_runtime = published_llm_runtime();
    repository.configure_published_compact_route(
        application.id,
        profile,
        "node-frozen-compact",
        frozen_runtime.clone(),
    );
    let publication_before_draft_mutation = repository
        .load_active_application_publication(application.id)
        .await
        .unwrap()
        .unwrap();
    ApplicationApiMappingService::new(repository.clone())
        .replace_mapping_draft(
            ReplaceApplicationApiMappingCommand {
                actor_user_id: actor_user_id(),
                application_id: application.id,
                mapping: published_mapping(),
            },
            Some(ApplicationOperationBindings {
                compact: ApplicationCompactOperationBindings {
                    responses_compaction_v2: Some(ApplicationOperationTargetBinding {
                        target_node_id: "node-draft-compact".into(),
                    }),
                    ..ApplicationCompactOperationBindings::default()
                },
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
        publication
            .operation_bindings
            .compact
            .responses_compaction_v2
            .as_ref()
            .unwrap()
            .target_node_id,
        "node-frozen-compact"
    );

    let resolver = PublishedRouteResolver::new(&repository);
    let route = resolver
        .resolve_compact(
            application.workspace_id,
            &publication,
            &compiled_plan,
            profile,
        )
        .await
        .unwrap();
    assert_eq!(route.operation, ProviderWireOperation::Compact);
    assert_eq!(route.profile, profile);
    assert_eq!(route.target_node_id, "node-frozen-compact");
    assert_eq!(route.llm_runtime, frozen_runtime);
    assert_eq!(
        repository.published_compact_capability_profiles(),
        vec![profile]
    );
    assert_eq!(repository.flow_run_count(), 0);

    repository.set_published_compact_capability_supported(false);
    let mismatch = resolver
        .resolve_compact(
            application.workspace_id,
            &publication,
            &compiled_plan,
            profile,
        )
        .await
        .unwrap_err();
    assert_eq!(
        mismatch,
        PublishedRouteResolutionError::ProviderCapabilityMismatch
    );
    assert_eq!(
        repository.published_compact_capability_profiles(),
        vec![profile, profile]
    );
    assert_eq!(repository.flow_run_count(), 0);
}

/// Root #1366 / K2b: invalid Compact targets cannot reach a provider capability check.
#[tokio::test]
async fn compact_unbound_and_invalid_targets_fail_before_provider_dispatch() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Invalid Compact Route App");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
    let profile = ProviderCompactProfile::ResponsesCompact;
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

    let unbound = resolver
        .resolve_compact(
            application.workspace_id,
            &publication,
            &compiled_plan,
            profile,
        )
        .await
        .unwrap_err();
    assert_eq!(unbound, PublishedRouteResolutionError::OperationUnbound);

    repository.configure_published_compact_route(
        application.id,
        profile,
        "node-frozen-compact",
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

    let mut missing_target_publication = publication.clone();
    missing_target_publication
        .operation_bindings
        .compact
        .responses_compact
        .as_mut()
        .unwrap()
        .target_node_id = "missing-node".into();
    let missing = resolver
        .resolve_compact(
            application.workspace_id,
            &missing_target_publication,
            &compiled_plan,
            profile,
        )
        .await
        .unwrap_err();

    let mut incomplete_plan = compiled_plan.clone();
    incomplete_plan.plan["nodes"]["node-frozen-compact"]["llm_runtime"]["model"] =
        serde_json::json!("");
    let incomplete = resolver
        .resolve_compact(
            application.workspace_id,
            &publication,
            &incomplete_plan,
            profile,
        )
        .await
        .unwrap_err();

    let mut non_llm_plan = compiled_plan.clone();
    non_llm_plan.plan["nodes"]["node-frozen-compact"]["node_type"] =
        serde_json::json!("http_request");
    let non_llm = resolver
        .resolve_compact(
            application.workspace_id,
            &publication,
            &non_llm_plan,
            profile,
        )
        .await
        .unwrap_err();

    assert_eq!(missing, PublishedRouteResolutionError::TargetMissing);
    assert_eq!(non_llm, PublishedRouteResolutionError::TargetNotLlm);
    assert_eq!(
        incomplete,
        PublishedRouteResolutionError::IncompleteLlmRuntime
    );
    assert_eq!(repository.published_compact_capability_checks(), 0);
    assert_eq!(repository.flow_run_count(), 0);
}

/// Root #1366 / K2b: ordinary Generate stays on its own resolver seam.
#[tokio::test]
async fn ordinary_generate_never_reaches_the_compact_resolver() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Generate Isolation App");
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
        "node-frozen-generate",
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

    let route = PublishedRouteResolver::new(&repository)
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::OperationBinding,
            AiNativeGenerateProfile::Standard,
            &BTreeSet::new(),
        )
        .await
        .unwrap();

    assert!(matches!(route, ResolvedPublishedRoute::Provider(_)));
    assert!(repository
        .published_compact_capability_profiles()
        .is_empty());
    assert_eq!(repository.flow_run_count(), 0);
}

/// Root #1366 AC-003 / AC-005: stale, non-LLM, and incomplete targets fail before capability.
#[tokio::test]
async fn generate_invalid_targets_fail_before_capability_lookup() {
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
            AiNativeGenerateProfile::Standard,
            &BTreeSet::new(),
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
            AiNativeGenerateProfile::Standard,
            &BTreeSet::new(),
        )
        .await
        .unwrap_err();

    let mut non_llm_plan = compiled_plan.clone();
    non_llm_plan.plan["nodes"]["node-frozen-llm"]["node_type"] = serde_json::json!("http_request");
    let non_llm = resolver
        .resolve_generate(
            application.workspace_id,
            &publication,
            &non_llm_plan,
            PublishedRouteDispatch::OperationBinding,
            AiNativeGenerateProfile::Standard,
            &BTreeSet::new(),
        )
        .await
        .unwrap_err();

    assert_eq!(missing, PublishedRouteResolutionError::TargetMissing);
    assert_eq!(non_llm, PublishedRouteResolutionError::TargetNotLlm);
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
            AiNativeGenerateProfile::Standard,
            &BTreeSet::new(),
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

/// D4-AC-002: application-flow dispatch is semantic-only and rejects opaque Responses pre-run.
#[tokio::test]
async fn d4_ac_002_application_flow_rejects_native_responses_passthrough_requirement() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Semantic Application Flow App");
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

    let error = PublishedRouteResolver::new(&repository)
        .resolve_generate(
            application.workspace_id,
            &publication,
            &compiled_plan,
            PublishedRouteDispatch::ApplicationFlow,
            AiNativeGenerateProfile::Standard,
            &BTreeSet::from([ProviderInvocationCapability::ResponsesNativePassthrough]),
        )
        .await
        .unwrap_err();

    assert_eq!(
        error,
        PublishedRouteResolutionError::ProviderCapabilityMismatch
    );
    assert_eq!(repository.flow_run_count(), 0);
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
