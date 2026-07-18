use control_plane::application_public_api::{
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingService,
        ApplicationCompactOperationBindings, ApplicationOperationBindings,
        ApplicationOperationTargetBinding, ReplaceApplicationApiMappingCommand,
    },
    operation_bindings::{
        draft_binding_options, ApplicationOperationBindingCapabilitySupport,
        ApplicationOperationBindingOperation, ApplicationOperationBindingProjectionService,
        ApplicationOperationBindingUnsupportedReason, ApplicationPublishedOperationBindingSupport,
        GetApplicationOperationBindingProjectionCommand,
    },
    publications::{ApplicationPublicationService, PublishApplicationCommand},
};
use orchestration_runtime::compiled_plan::{CompiledLlmRuntime, CompiledNode, CompiledPlan};
use plugin_framework::provider_contract::{
    PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY,
    PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY, PROVIDER_COUNT_TOKENS_CAPABILITY,
};
use uuid::Uuid;

use super::*;

fn compiled_plan(nodes: Vec<CompiledNode>) -> CompiledPlan {
    CompiledPlan {
        flow_id: Uuid::now_v7(),
        source_draft_id: Uuid::now_v7().to_string(),
        schema_version: domain::FLOW_SCHEMA_VERSION.to_string(),
        topological_order: nodes.iter().map(|node| node.node_id.clone()).collect(),
        edges: Vec::new(),
        nodes: nodes
            .into_iter()
            .map(|node| (node.node_id.clone(), node))
            .collect(),
        compile_issues: Vec::new(),
    }
}

fn compiled_llm_node(node_id: &str) -> CompiledNode {
    CompiledNode {
        node_id: node_id.to_string(),
        node_type: "llm".to_string(),
        alias: format!("{node_id} alias"),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: Default::default(),
        outputs: Vec::new(),
        config: serde_json::json!({}),
        plugin_runtime: None,
        llm_runtime: Some(complete_llm_runtime()),
        code_runtime: None,
    }
}

fn complete_llm_runtime() -> CompiledLlmRuntime {
    CompiledLlmRuntime {
        provider_instance_id: Uuid::now_v7().to_string(),
        provider_instance_display_name: "Binding fixture provider".to_string(),
        provider_code: "binding_fixture_provider".to_string(),
        protocol: "binding_fixture_protocol".to_string(),
        model: "binding-fixture-model".to_string(),
        routing: None,
    }
}

#[test]
fn b2b_operation_rows_use_the_exact_c1_and_k2_manifest_capabilities() {
    assert_eq!(
        ApplicationOperationBindingOperation::Generate.required_manifest_capability(),
        None
    );
    assert_eq!(
        ApplicationOperationBindingOperation::CountTokens.required_manifest_capability(),
        Some(PROVIDER_COUNT_TOKENS_CAPABILITY)
    );
    assert_eq!(
        ApplicationOperationBindingOperation::CompactResponsesCompact
            .required_manifest_capability(),
        Some(PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY)
    );
    assert_eq!(
        ApplicationOperationBindingOperation::CompactResponsesCompactionV2
            .required_manifest_capability(),
        Some(PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY)
    );
}

fn operation_options(
    options: &[control_plane::application_public_api::operation_bindings::ApplicationOperationBindingOptions],
    operation: ApplicationOperationBindingOperation,
) -> &[control_plane::application_public_api::operation_bindings::ApplicationOperationBindingTargetOption]{
    &options
        .iter()
        .find(|option| option.operation == operation)
        .expect("every operation has one binding option row")
        .targets
}

/// Root #1366 AC-003: draft choices are server-filtered by the exact operation
/// capability and never contain an incomplete or non-LLM compiled target.
#[tokio::test]
async fn b2b_draft_binding_options_cover_zero_one_two_targets_without_client_capability_inference()
{
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();

    let zero = draft_binding_options(&repository, Uuid::now_v7(), &compiled_plan(Vec::new()))
        .await
        .unwrap();
    assert_eq!(zero.len(), 4);
    assert!(zero.iter().all(|option| option.targets.is_empty()));

    let one_plan = compiled_plan(vec![compiled_llm_node("node-one")]);
    let one = draft_binding_options(&repository, Uuid::now_v7(), &one_plan)
        .await
        .unwrap();
    for operation in ApplicationOperationBindingOperation::ALL {
        assert_eq!(operation_options(&one, operation).len(), 1);
        assert_eq!(
            operation_options(&one, operation)[0].target_node_id,
            "node-one"
        );
    }

    repository.set_operation_binding_capability_support(
        ApplicationOperationBindingOperation::CountTokens,
        ApplicationOperationBindingCapabilitySupport::ProviderCapabilityUnsupported,
    );
    let two = draft_binding_options(
        &repository,
        Uuid::now_v7(),
        &compiled_plan(vec![
            compiled_llm_node("node-one"),
            compiled_llm_node("node-two"),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        operation_options(&two, ApplicationOperationBindingOperation::Generate)
            .iter()
            .map(|target| target.target_node_id.as_str())
            .collect::<Vec<_>>(),
        vec!["node-one", "node-two"]
    );
    assert!(operation_options(&two, ApplicationOperationBindingOperation::CountTokens).is_empty());
}

/// Root #1366 AC-003 / AC-006: the publication status stays anchored to its
/// own frozen plan when the editable draft moves to another binding.
#[tokio::test]
async fn b2b_published_binding_projection_reports_supported_unbound_and_typed_unsupported() {
    let harness = ApplicationPublicApiTestHarness::new();
    let repository = harness.repository();
    let application = harness.seed_application(actor_user_id(), "Binding Projection App");
    ApplicationPublicationService::new(repository.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
            mapping: ApplicationApiMappingConfig::default_native(),
            api_enabled: true,
        })
        .await
        .unwrap();
    repository.configure_published_operation_binding(
        application.id,
        ApplicationOperationBindingOperation::Generate,
        "node-frozen-generate",
        complete_llm_runtime(),
    );
    repository.configure_published_operation_binding(
        application.id,
        ApplicationOperationBindingOperation::CountTokens,
        "node-frozen-count",
        complete_llm_runtime(),
    );
    repository.set_operation_binding_capability_support(
        ApplicationOperationBindingOperation::CountTokens,
        ApplicationOperationBindingCapabilitySupport::ProviderCapabilityUnsupported,
    );

    ApplicationApiMappingService::new(repository.clone())
        .replace_mapping_draft(
            ReplaceApplicationApiMappingCommand {
                actor_user_id: actor_user_id(),
                application_id: application.id,
                mapping: ApplicationApiMappingConfig::default_native(),
            },
            Some(ApplicationOperationBindings {
                generate: Some(ApplicationOperationTargetBinding {
                    target_node_id: "node-draft-only".to_string(),
                }),
                count_tokens: None,
                compact: ApplicationCompactOperationBindings::default(),
            }),
        )
        .await
        .unwrap();

    let projection = ApplicationOperationBindingProjectionService::new(repository)
        .get_projection(GetApplicationOperationBindingProjectionCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();
    assert_eq!(
        projection
            .draft
            .operation_bindings
            .generate
            .as_ref()
            .map(|binding| binding.target_node_id.as_str()),
        Some("node-draft-only")
    );

    let published = projection
        .published
        .expect("published projection is present");
    let generate = published
        .bindings
        .iter()
        .find(|binding| binding.operation == ApplicationOperationBindingOperation::Generate)
        .expect("Generate projection is present");
    assert_eq!(
        generate.target_node_id.as_deref(),
        Some("node-frozen-generate")
    );
    assert!(matches!(
        &generate.support,
        ApplicationPublishedOperationBindingSupport::Supported { target }
            if target.target_node_id == "node-frozen-generate"
    ));

    let count_tokens = published
        .bindings
        .iter()
        .find(|binding| binding.operation == ApplicationOperationBindingOperation::CountTokens)
        .expect("CountTokens projection is present");
    assert!(matches!(
        &count_tokens.support,
        ApplicationPublishedOperationBindingSupport::Unsupported {
            reason: ApplicationOperationBindingUnsupportedReason::ProviderCapabilityUnsupported,
            ..
        }
    ));

    let compact = published
        .bindings
        .iter()
        .find(|binding| {
            binding.operation == ApplicationOperationBindingOperation::CompactResponsesCompact
        })
        .expect("remote compact projection is present");
    assert!(matches!(
        &compact.support,
        ApplicationPublishedOperationBindingSupport::Unbound
    ));
}

/// Root #1366 D-004: a viewer may see the frozen projection but cannot infer
/// editability from route access or row ownership.
#[tokio::test]
async fn b2b_projection_uses_existing_application_edit_permission_for_editability() {
    let harness = ApplicationPublicApiTestHarness::new_with_console_policies(vec![
        application_console_policy(vec![
            application_row_operation(
                access_control::APPLICATIONS_VIEW_OPERATION_ID,
                domain::ConsoleOperationRowScope::ScopeAll,
            ),
            application_row_operation(
                access_control::APPLICATIONS_UPDATE_OPERATION_ID,
                domain::ConsoleOperationRowScope::Disabled,
            ),
        ]),
    ]);
    let application = harness.seed_application(other_user_id(), "Read Only Binding Projection");

    let projection = ApplicationOperationBindingProjectionService::new(harness.repository())
        .get_projection(GetApplicationOperationBindingProjectionCommand {
            actor_user_id: actor_user_id(),
            application_id: application.id,
        })
        .await
        .unwrap();

    assert!(!projection.editable);
    assert!(projection.published.is_none());
}
