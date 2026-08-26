use std::{collections::BTreeSet, sync::Arc, time::Duration};

use anyhow::{bail, Result};
use async_trait::async_trait;
use extension_contracts::{
    extension_bus::{
        compile_extension_graph, Cardinality, ContractDescriptor, ContractVersion,
        ContributionDescriptor, ContributionId, ContributionMode, ContributionOrdering,
        DeliverySemantics, ExtensionBusVersion, ExtensionPointDescriptor, ExtensionPointId,
        ExtensionPointKind, FailureSemantics, LifecycleSemantics, ModuleActivationDeclaration,
        ModuleDescriptor, ModuleId, ModuleKind, ModuleVersion, OrderingSemantics, OverridePolicy,
        PermissionCode, ScopeSemantics,
    },
    provider_contract::{
        NativePromptBlock, NativePromptCacheControl, NativePromptCacheControlType,
        ProviderInvocationCapability, ProviderInvocationInput, ProviderMessage,
        ProviderMessageRole,
    },
};
use serde_json::json;

use crate::provider_input_pipeline::{
    PipelineContributionStatus, ProviderInputContributionFailurePolicy, ProviderInputPipeline,
    ProviderInputPipelineContribution, TrustedProviderInputContributionRegistration,
    PROVIDER_INPUT_PIPELINE_CONTRACT_ID, PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION,
    PROVIDER_INPUT_PIPELINE_OWNER_MODULE_ID, PROVIDER_INPUT_PIPELINE_POINT_ID,
    REWRITE_MESSAGES_PERMISSION, REWRITE_MODEL_PARAMETERS_PERMISSION,
    REWRITE_RESPONSE_FORMAT_PERMISSION, REWRITE_SYSTEM_PERMISSION, REWRITE_TOOLS_PERMISSION,
};

#[derive(Clone, Copy)]
enum Rewrite {
    AppendOrder(&'static str),
    AllVisible,
    ToolsAndCachedSystem,
    ProviderCode,
    ProviderConfig,
    TraceContext,
    InvalidMessages,
    Error,
    Panic,
    Delay,
}

#[async_trait]
impl ProviderInputPipelineContribution for Rewrite {
    async fn rewrite(&self, mut input: ProviderInvocationInput) -> Result<ProviderInvocationInput> {
        match self {
            Self::AppendOrder(value) => {
                let order = input
                    .model_parameters
                    .entry("order".to_string())
                    .or_insert_with(|| json!(""));
                *order = json!(format!("{}{}", order.as_str().unwrap_or_default(), value));
            }
            Self::AllVisible => {
                input.messages.push(message("rewritten"));
                input.system.push(NativePromptBlock::text("system rewrite"));
                input.tools.push(json!({ "name": "pipeline_tool" }));
                input.response_format = Some(json!({ "type": "json_object" }));
                input
                    .model_parameters
                    .insert("temperature".to_string(), json!(0.2));
            }
            Self::ToolsAndCachedSystem => {
                input.tools.push(json!({ "name": "pipeline_tool" }));
                input.system.push(NativePromptBlock::Text {
                    text: "cached system".to_string(),
                    cache_control: Some(NativePromptCacheControl {
                        cache_type: NativePromptCacheControlType::Ephemeral,
                        ttl: None,
                    }),
                });
            }
            Self::ProviderCode => input.provider_code = "crossed-provider".to_string(),
            Self::ProviderConfig => input.provider_config = json!({ "api_key": "forbidden" }),
            Self::TraceContext => {
                input
                    .trace_context
                    .insert("trace_owner".to_string(), "forbidden".to_string());
            }
            Self::InvalidMessages => input.messages[0].content_blocks = Some(json!("invalid")),
            Self::Error => bail!("raw contribution error must not escape"),
            Self::Panic => panic!("raw contribution panic must not escape"),
            Self::Delay => tokio::time::sleep(Duration::from_millis(20)).await,
        }
        Ok(input)
    }
}

fn message(content: &str) -> ProviderMessage {
    ProviderMessage {
        role: ProviderMessageRole::User,
        content: content.to_string(),
        name: None,
        tool_call_id: None,
        is_error: None,
        tool_calls: None,
        content_blocks: None,
    }
}

fn canonical_input() -> ProviderInvocationInput {
    ProviderInvocationInput {
        provider_instance_id: "provider-instance".to_string(),
        provider_code: "provider-code".to_string(),
        protocol: "provider-protocol".to_string(),
        model: "provider-model".to_string(),
        messages: vec![message("original")],
        provider_config: json!({ "api_key": "secret" }),
        trace_context: [("trace_owner".to_string(), "application".to_string())]
            .into_iter()
            .collect(),
        ..ProviderInvocationInput::default()
    }
}

fn permission(value: &str) -> PermissionCode {
    PermissionCode::new(value).unwrap()
}

fn point() -> ExtensionPointDescriptor {
    ExtensionPointDescriptor {
        point_id: ExtensionPointId::new(PROVIDER_INPUT_PIPELINE_POINT_ID).unwrap(),
        owner_module_id: ModuleId::new(PROVIDER_INPUT_PIPELINE_OWNER_MODULE_ID).unwrap(),
        point_kind: ExtensionPointKind::Pipeline,
        contract: ContractDescriptor::new(
            PROVIDER_INPUT_PIPELINE_CONTRACT_ID,
            PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION,
        )
        .unwrap(),
        scope: ScopeSemantics::Global,
        cardinality: Cardinality::Many,
        ordering: OrderingSemantics::Dependency,
        failure: FailureSemantics::FailClosed,
        delivery: DeliverySemantics::Synchronous,
        lifecycle: LifecycleSemantics::Invocation,
        allowed_permissions: [
            REWRITE_MESSAGES_PERMISSION,
            REWRITE_SYSTEM_PERMISSION,
            REWRITE_TOOLS_PERMISSION,
            REWRITE_RESPONSE_FORMAT_PERMISSION,
            REWRITE_MODEL_PARAMETERS_PERMISSION,
        ]
        .into_iter()
        .map(permission)
        .collect(),
        override_policy: OverridePolicy::Sealed,
    }
}

fn core_module() -> ModuleDescriptor {
    ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(PROVIDER_INPUT_PIPELINE_OWNER_MODULE_ID).unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: vec![point()],
        contributions: Vec::new(),
    }
}

fn contributor(
    module_id: &str,
    contribution_id: &str,
    permissions: &[&str],
    after: Option<&str>,
) -> ModuleDescriptor {
    let permissions = permissions
        .iter()
        .copied()
        .map(permission)
        .collect::<BTreeSet<_>>();
    ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new(module_id).unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::TrustedHost,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: permissions.clone(),
        extension_points: Vec::new(),
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new(contribution_id).unwrap(),
            contributor_module_id: ModuleId::new(module_id).unwrap(),
            point_id: ExtensionPointId::new(PROVIDER_INPUT_PIPELINE_POINT_ID).unwrap(),
            contract_version: ContractVersion::new(PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION)
                .unwrap(),
            required_permissions: permissions,
            mode: ContributionMode::Append,
            ordering: ContributionOrdering {
                after: after
                    .map(ContributionId::new)
                    .transpose()
                    .unwrap()
                    .into_iter()
                    .collect(),
                before: BTreeSet::new(),
            },
        }],
    }
}

fn pipeline(
    contributors: Vec<ModuleDescriptor>,
    registrations: Vec<TrustedProviderInputContributionRegistration>,
) -> ProviderInputPipeline {
    let graph =
        compile_extension_graph(std::iter::once(core_module()).chain(contributors).collect())
            .unwrap();
    ProviderInputPipeline::from_graph(Arc::new(graph), registrations).unwrap()
}

fn registration(
    contribution_id: &str,
    rewrite: Rewrite,
    timeout: Duration,
) -> TrustedProviderInputContributionRegistration {
    TrustedProviderInputContributionRegistration {
        contribution_id: contribution_id.to_string(),
        timeout,
        failure_policy: ProviderInputContributionFailurePolicy::FailClosed,
        executor: Arc::new(rewrite),
    }
}

const ALL_VISIBLE_PERMISSIONS: &[&str] = &[
    REWRITE_MESSAGES_PERMISSION,
    REWRITE_SYSTEM_PERMISSION,
    REWRITE_TOOLS_PERMISSION,
    REWRITE_RESPONSE_FORMAT_PERMISSION,
    REWRITE_MODEL_PARAMETERS_PERMISSION,
];

// Root #1688 D3-P1: an empty production graph is a typed identity pipeline.
#[tokio::test]
async fn empty_pipeline_preserves_the_exact_canonical_input() {
    let pipeline = pipeline(Vec::new(), Vec::new());
    let input = canonical_input();

    let output = pipeline.execute(input.clone()).await.unwrap();

    assert_eq!(output.input, input);
    assert!(output.receipt.unwrap().contributions.is_empty());
}

#[tokio::test]
async fn graph_dependency_order_wins_over_registration_order() {
    let contributors = vec![
        contributor(
            "fixture.beta",
            "fixture.beta.rewrite",
            &[REWRITE_MODEL_PARAMETERS_PERMISSION],
            Some("fixture.alpha.rewrite"),
        ),
        contributor(
            "fixture.alpha",
            "fixture.alpha.rewrite",
            &[REWRITE_MODEL_PARAMETERS_PERMISSION],
            None,
        ),
    ];
    let pipeline = pipeline(
        contributors,
        vec![
            registration(
                "fixture.beta.rewrite",
                Rewrite::AppendOrder("beta"),
                Duration::from_secs(1),
            ),
            registration(
                "fixture.alpha.rewrite",
                Rewrite::AppendOrder("alpha-"),
                Duration::from_secs(1),
            ),
        ],
    );

    let output = pipeline.execute(canonical_input()).await.unwrap();
    let receipt = output.receipt.unwrap();

    assert_eq!(output.input.model_parameters["order"], "alpha-beta");
    assert_eq!(
        receipt
            .contributions
            .iter()
            .map(|entry| (entry.order, entry.contribution_id.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, "fixture.alpha.rewrite"), (1, "fixture.beta.rewrite")]
    );
}

#[tokio::test]
async fn permitted_model_visible_rewrites_are_explained_by_the_receipt() {
    let pipeline = pipeline(
        vec![contributor(
            "fixture.visible",
            "fixture.visible.rewrite",
            ALL_VISIBLE_PERMISSIONS,
            None,
        )],
        vec![registration(
            "fixture.visible.rewrite",
            Rewrite::AllVisible,
            Duration::from_secs(1),
        )],
    );

    let output = pipeline.execute(canonical_input()).await.unwrap();
    let receipt = output.receipt.unwrap();

    assert_eq!(output.input.messages.last().unwrap().content, "rewritten");
    assert_eq!(
        output.input.system.last().unwrap().text_content(),
        "system rewrite"
    );
    assert_eq!(output.input.tools, vec![json!({ "name": "pipeline_tool" })]);
    assert_eq!(
        output.input.response_format,
        Some(json!({ "type": "json_object" }))
    );
    assert_eq!(output.input.model_parameters["temperature"], 0.2);
    assert_eq!(
        receipt.contributions[0].changed_fields,
        vec![
            "messages",
            "system",
            "tools",
            "response_format",
            "model_parameters"
        ]
    );
    assert_eq!(
        receipt.contributions[0].status,
        PipelineContributionStatus::Succeeded
    );
    assert_ne!(receipt.before_digest, receipt.after_digest);
}

#[tokio::test]
async fn tools_rewrite_keeps_core_capabilities_synchronized_and_receipt_matches_final_input() {
    let tools_pipeline = pipeline(
        vec![contributor(
            "fixture.tools",
            "fixture.tools.rewrite",
            &[REWRITE_SYSTEM_PERMISSION, REWRITE_TOOLS_PERMISSION],
            None,
        )],
        vec![registration(
            "fixture.tools.rewrite",
            Rewrite::ToolsAndCachedSystem,
            Duration::from_secs(1),
        )],
    );
    let output = tools_pipeline.execute(canonical_input()).await.unwrap();
    let receipt = output.receipt.as_ref().unwrap();

    assert!(output
        .input
        .required_capabilities
        .contains(&ProviderInvocationCapability::SystemPromptBlocks));
    assert!(output
        .input
        .required_capabilities
        .contains(&ProviderInvocationCapability::SystemPromptCacheControl));
    assert_eq!(
        receipt.contributions[0].changed_fields,
        vec!["system", "tools"]
    );
    let after_digest = receipt.after_digest.clone();

    let final_input_receipt = pipeline(Vec::new(), Vec::new())
        .execute(output.input)
        .await
        .unwrap()
        .receipt
        .unwrap();
    assert_eq!(after_digest, final_input_receipt.before_digest);
}

#[tokio::test]
async fn routing_secrets_and_trace_ownership_rewrites_fail_closed() {
    for (module_id, contribution_id, rewrite) in [
        (
            "fixture.identity",
            "fixture.identity.rewrite",
            Rewrite::ProviderCode,
        ),
        (
            "fixture.secret",
            "fixture.secret.rewrite",
            Rewrite::ProviderConfig,
        ),
        (
            "fixture.trace",
            "fixture.trace.rewrite",
            Rewrite::TraceContext,
        ),
    ] {
        let pipeline = pipeline(
            vec![contributor(module_id, contribution_id, &[], None)],
            vec![registration(
                contribution_id,
                rewrite,
                Duration::from_secs(1),
            )],
        );

        let error = pipeline.execute(canonical_input()).await.unwrap_err();

        assert_eq!(error.code, "unauthorized_rewrite");
        assert_eq!(
            error.receipt.contributions[0].status,
            PipelineContributionStatus::Failed
        );
        assert_eq!(
            error.receipt.contributions[0].error.as_deref(),
            Some("unauthorized_rewrite")
        );
    }
}

#[tokio::test]
async fn missing_field_permission_fails_closed() {
    let pipeline = pipeline(
        vec![contributor(
            "fixture.permission",
            "fixture.permission.rewrite",
            &[],
            None,
        )],
        vec![registration(
            "fixture.permission.rewrite",
            Rewrite::AppendOrder("blocked"),
            Duration::from_secs(1),
        )],
    );

    let error = pipeline.execute(canonical_input()).await.unwrap_err();

    assert_eq!(error.code, "unauthorized_rewrite");
}

#[tokio::test]
async fn invalid_rewritten_message_contract_fails_closed_with_attempted_field_receipt() {
    let pipeline = pipeline(
        vec![contributor(
            "fixture.invalid-message",
            "fixture.invalid-message.rewrite",
            &[REWRITE_MESSAGES_PERMISSION],
            None,
        )],
        vec![registration(
            "fixture.invalid-message.rewrite",
            Rewrite::InvalidMessages,
            Duration::from_secs(1),
        )],
    );

    let error = pipeline.execute(canonical_input()).await.unwrap_err();

    assert_eq!(error.code, "invalid_model_visible_input");
    assert_eq!(
        error.receipt.contributions[0].changed_fields,
        vec!["messages"]
    );
    assert_ne!(
        error.receipt.contributions[0].before_digest,
        error.receipt.contributions[0].after_digest
    );
}

#[tokio::test]
async fn contribution_error_panic_and_timeout_fail_closed_with_safe_receipts() {
    for (suffix, rewrite, timeout, expected_code) in [
        (
            "error",
            Rewrite::Error,
            Duration::from_secs(1),
            "contribution_error",
        ),
        (
            "panic",
            Rewrite::Panic,
            Duration::from_secs(1),
            "contribution_panic",
        ),
        (
            "timeout",
            Rewrite::Delay,
            Duration::from_millis(1),
            "contribution_timeout",
        ),
    ] {
        let module_id = format!("fixture.{suffix}");
        let contribution_id = format!("fixture.{suffix}.rewrite");
        let pipeline = pipeline(
            vec![contributor(&module_id, &contribution_id, &[], None)],
            vec![registration(&contribution_id, rewrite, timeout)],
        );

        let error = pipeline.execute(canonical_input()).await.unwrap_err();

        assert_eq!(error.code, expected_code);
        assert_eq!(
            error.receipt.contributions[0].error.as_deref(),
            Some(expected_code)
        );
        assert!(!format!("{error}").contains("raw contribution"));
    }
}

#[tokio::test]
async fn receipt_digest_is_stable_across_execution_durations() {
    let pipeline = pipeline(
        vec![contributor(
            "fixture.stable",
            "fixture.stable.rewrite",
            &[REWRITE_MODEL_PARAMETERS_PERMISSION],
            None,
        )],
        vec![registration(
            "fixture.stable.rewrite",
            Rewrite::AppendOrder("stable"),
            Duration::from_secs(1),
        )],
    );

    let first = pipeline
        .execute(canonical_input())
        .await
        .unwrap()
        .receipt
        .unwrap();
    let second = pipeline
        .execute(canonical_input())
        .await
        .unwrap()
        .receipt
        .unwrap();

    assert_eq!(first.before_digest, second.before_digest);
    assert_eq!(first.after_digest, second.after_digest);
    assert_eq!(first.receipt_digest, second.receipt_digest);
}

#[test]
fn duplicate_trusted_registrations_are_rejected() {
    let graph = compile_extension_graph(vec![
        core_module(),
        contributor("fixture.duplicate", "fixture.duplicate.rewrite", &[], None),
    ])
    .unwrap();
    let registration = registration(
        "fixture.duplicate.rewrite",
        Rewrite::Error,
        Duration::from_secs(1),
    );

    let error = ProviderInputPipeline::from_graph(
        Arc::new(graph),
        vec![registration.clone(), registration],
    )
    .err()
    .unwrap();

    assert!(error
        .to_string()
        .contains("duplicate provider input contribution registration"));
}

#[test]
fn graph_and_trusted_registration_sets_must_match_exactly() {
    let missing_registration_graph = compile_extension_graph(vec![
        core_module(),
        contributor("fixture.missing", "fixture.missing.rewrite", &[], None),
    ])
    .unwrap();
    let missing_error =
        ProviderInputPipeline::from_graph(Arc::new(missing_registration_graph), Vec::new())
            .err()
            .unwrap();
    assert!(missing_error
        .to_string()
        .contains("provider input contribution is not registered"));

    let empty_graph = compile_extension_graph(vec![core_module()]).unwrap();
    let extra_error = ProviderInputPipeline::from_graph(
        Arc::new(empty_graph),
        vec![registration(
            "fixture.extra.rewrite",
            Rewrite::Error,
            Duration::from_secs(1),
        )],
    )
    .err()
    .unwrap();
    assert!(extra_error
        .to_string()
        .contains("provider input contribution is absent from the effective graph"));
}

#[test]
fn pipeline_consumer_has_no_provider_specific_branching() {
    let source = include_str!("../provider_input_pipeline.rs").to_ascii_lowercase();

    assert!(!source.contains("openai"));
    assert!(!source.contains("anthropic"));
    assert!(!source.contains("deepseek"));
}
