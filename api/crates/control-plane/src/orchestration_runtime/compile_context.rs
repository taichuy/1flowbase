use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{
        ApplicationJsDependencySelectionRepository, ModelProviderRepository,
        NodeContributionRepository, PluginRepository,
    },
};

pub(crate) async fn build_compile_context<R>(
    repository: &R,
    workspace_id: Uuid,
) -> Result<orchestration_runtime::compiler::FlowCompileContext>
where
    R: ModelProviderRepository + NodeContributionRepository + PluginRepository,
{
    let instances = repository.list_instances(workspace_id).await?;
    let contributions = repository.list_node_contributions(workspace_id).await?;
    let assigned_installation_ids = repository
        .list_assignments(workspace_id)
        .await?
        .into_iter()
        .map(|assignment| assignment.installation_id)
        .collect::<BTreeSet<_>>();
    let mut provider_families = BTreeMap::new();
    let mut provider_instances = BTreeMap::new();
    let mut provider_codes = BTreeSet::new();
    let mut node_contributions = BTreeMap::new();

    for instance in instances {
        provider_codes.insert(instance.provider_code.clone());
        let available_models = available_models_for_instance(repository, &instance).await?;
        let allow_custom_models = allow_custom_models(&instance);
        let installation_runnable = installation_is_runnable(
            repository,
            instance.installation_id,
            assigned_installation_ids.contains(&instance.installation_id),
        )
        .await?;
        provider_instances.insert(
            instance.id.to_string(),
            orchestration_runtime::compiler::FlowCompileProviderInstance {
                provider_instance_id: instance.id.to_string(),
                display_name: instance.display_name.clone(),
                provider_code: instance.provider_code.clone(),
                protocol: instance.protocol.clone(),
                is_ready: instance.status == domain::ModelProviderInstanceStatus::Ready,
                is_runnable: installation_runnable,
                included_in_main: instance.included_in_main,
                available_models: available_models.clone(),
                allow_custom_models,
            },
        );

        if !instance.included_in_main || !installation_runnable {
            continue;
        }

        provider_families
            .entry(instance.provider_code.clone())
            .and_modify(
                |family: &mut orchestration_runtime::compiler::FlowCompileProviderFamily| {
                    family.is_ready |=
                        instance.status == domain::ModelProviderInstanceStatus::Ready;
                    family
                        .available_models
                        .extend(available_models.iter().cloned());
                    family.allow_custom_models |= allow_custom_models;
                },
            )
            .or_insert_with(
                || orchestration_runtime::compiler::FlowCompileProviderFamily {
                    provider_code: instance.provider_code.clone(),
                    protocol: instance.protocol.clone(),
                    is_ready: instance.status == domain::ModelProviderInstanceStatus::Ready,
                    available_models,
                    allow_custom_models,
                },
            );
    }

    let mut model_distribution_rules = BTreeMap::new();
    for provider_code in provider_codes {
        let Some(main_instance) = repository
            .get_main_instance(workspace_id, &provider_code)
            .await?
        else {
            continue;
        };
        for rule in main_instance.model_distribution_rules {
            model_distribution_rules.insert(
                (provider_code.clone(), rule.model_id),
                map_llm_distribution_rule(rule.distribution_rule),
            );
        }
    }

    for entry in contributions {
        let key = node_contribution_lookup_key(
            &entry.plugin_id,
            &entry.plugin_version,
            &entry.contribution_code,
            &entry.node_shell,
            &entry.schema_version,
        );
        node_contributions.insert(
            key,
            orchestration_runtime::compiler::FlowCompileNodeContribution {
                installation_id: entry.installation_id,
                plugin_unique_identifier: entry.plugin_unique_identifier,
                package_id: entry.package_id,
                plugin_id: entry.plugin_id,
                plugin_version: entry.plugin_version,
                contribution_code: entry.contribution_code,
                node_shell: entry.node_shell,
                schema_version: entry.schema_version,
                contribution_checksum: entry.contribution_checksum,
                compiled_contribution_hash: entry.compiled_contribution_hash,
                output_schema_snapshot: compile_contribution_outputs(
                    &entry.output_schema_snapshot,
                )?,
                side_effect_policy: entry.side_effect_policy,
                dependency_status: entry.dependency_status.as_str().to_string(),
            },
        );
    }

    Ok(orchestration_runtime::compiler::FlowCompileContext {
        workspace_id: Some(workspace_id),
        provider_families,
        provider_instances,
        model_distribution_rules,
        node_contributions,
        js_dependencies: BTreeMap::new(),
    })
}

fn map_llm_distribution_rule(
    rule: domain::ModelProviderDistributionRule,
) -> orchestration_runtime::compiled_plan::LlmDistributionRule {
    match rule {
        domain::ModelProviderDistributionRule::None => {
            orchestration_runtime::compiled_plan::LlmDistributionRule::None
        }
        domain::ModelProviderDistributionRule::RoundRobin => {
            orchestration_runtime::compiled_plan::LlmDistributionRule::RoundRobin
        }
        domain::ModelProviderDistributionRule::RetryRoundRobin => {
            orchestration_runtime::compiled_plan::LlmDistributionRule::RetryRoundRobin
        }
    }
}

pub(crate) async fn build_application_compile_context<R>(
    repository: &R,
    workspace_id: Uuid,
    application_id: Uuid,
) -> Result<orchestration_runtime::compiler::FlowCompileContext>
where
    R: ModelProviderRepository
        + NodeContributionRepository
        + PluginRepository
        + ApplicationJsDependencySelectionRepository,
{
    let mut context = build_compile_context(repository, workspace_id).await?;
    context.js_dependencies = repository
        .list_application_js_dependency_selections(workspace_id, application_id)
        .await?
        .into_iter()
        .map(|selection| {
            (
                orchestration_runtime::compiler::js_dependency_lookup_key(
                    &selection.target,
                    &selection.alias,
                ),
                orchestration_runtime::compiler::FlowCompileJsDependency {
                    alias: selection.alias,
                    target: selection.target,
                    artifact_path: selection.artifact_path,
                    artifact_hash: selection.artifact_hash,
                    integrity: selection.integrity,
                },
            )
        })
        .collect();
    Ok(context)
}

pub(super) fn ensure_compiled_plan_runnable(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
) -> Result<()> {
    if let Some(issue) = compiled_plan.compile_issues.first() {
        return Err(ControlPlaneError::InvalidInput(compile_issue_field(issue)).into());
    }
    orchestration_runtime::compiler::ensure_plan_execution_contract(compiled_plan)?;

    Ok(())
}

pub(super) fn ensure_compiled_plan_runnable_for_node(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    target_node_id: &str,
) -> Result<()> {
    let target_node_scope = collect_target_node_dependency_scope(compiled_plan, target_node_id);
    let blocking_issue = compiled_plan
        .compile_issues
        .iter()
        .find(|issue| target_node_scope.contains(issue.node_id.as_str()));

    if let Some(issue) = blocking_issue {
        return Err(ControlPlaneError::InvalidInput(compile_issue_field(issue)).into());
    }

    Ok(())
}

fn collect_target_node_dependency_scope(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    target_node_id: &str,
) -> BTreeSet<String> {
    let mut scope = BTreeSet::new();
    let mut stack = vec![target_node_id.to_string()];

    while let Some(node_id) = stack.pop() {
        if !scope.insert(node_id.clone()) {
            continue;
        }

        let Some(node) = compiled_plan.nodes.get(&node_id) else {
            continue;
        };

        stack.extend(node.dependency_node_ids.iter().cloned());
    }

    scope
}

fn compile_issue_field(issue: &orchestration_runtime::compiled_plan::CompileIssue) -> &'static str {
    match issue.code {
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingProviderInstance => {
            missing_provider_field(issue.message.as_str())
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::ProviderInstanceNotFound
        | orchestration_runtime::compiled_plan::CompileIssueCode::ProviderInstanceNotReady => {
            "provider_code"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingModel
        | orchestration_runtime::compiled_plan::CompileIssueCode::ModelNotAvailable => "model",
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingPluginId => "plugin_id",
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingPluginVersion => {
            "plugin_version"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingContributionCode => {
            "contribution_code"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingNodeShell => "node_shell",
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingSchemaVersion => {
            "schema_version"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingPluginUniqueIdentifier => {
            "plugin_unique_identifier"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingPackageId => "package_id",
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingContributionChecksum => {
            "contribution_checksum"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingCompiledContributionHash => {
            "compiled_contribution_hash"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingOutputSchemaSnapshot
        | orchestration_runtime::compiled_plan::CompileIssueCode::PluginContributionOutputSchemaMismatch => {
            "output_schema_snapshot"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::UnsupportedPluginContributionSchemaVersion => {
            "schema_version"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::PluginContributionChecksumMismatch => {
            "contribution_checksum"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingPluginContribution
        | orchestration_runtime::compiled_plan::CompileIssueCode::PluginContributionDependencyNotReady => {
            "contribution_code"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::JsDependencyImportNotEnabled
        | orchestration_runtime::compiled_plan::CompileIssueCode::InvalidJsDependencyImport => "imports",
        orchestration_runtime::compiled_plan::CompileIssueCode::InvalidCodeIsolationProfile => {
            "isolation"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::DuplicateAnswerPresentationReference
        | orchestration_runtime::compiled_plan::CompileIssueCode::InvalidAnswerPresentationOrder => {
            "answer_template"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::InvalidLlmContextSelector
        | orchestration_runtime::compiled_plan::CompileIssueCode::IncompatibleLlmContextSchema => {
            "context_policy"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::InvalidVisibleInternalLlmTool => {
            "visible_internal_llm_tools"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::InvalidSelectorSource
        | orchestration_runtime::compiled_plan::CompileIssueCode::SelectorSourceNotReachable
        | orchestration_runtime::compiled_plan::CompileIssueCode::SelectorOutputNotFound => {
            "bindings"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::UnsupportedNodeType => "node_type",
        orchestration_runtime::compiled_plan::CompileIssueCode::UnresolvedNode => {
            "unresolved_node"
        }
    }
}

fn compile_contribution_outputs(
    output_schema_snapshot: &serde_json::Value,
) -> Result<Vec<orchestration_runtime::compiled_plan::CompiledOutput>> {
    let outputs = output_schema_snapshot
        .get("outputs")
        .and_then(serde_json::Value::as_array)
        .ok_or(ControlPlaneError::InvalidInput("output_schema_snapshot"))?;

    outputs
        .iter()
        .map(|output| {
            let key = required_output_string(output, "key")?;
            Ok(orchestration_runtime::compiled_plan::CompiledOutput {
                selector: read_output_selector(output).unwrap_or_else(|| vec![key.clone()]),
                key,
                title: required_output_string(output, "title")?,
                value_type: required_output_string(output, "valueType")?,
                json_schema: output
                    .get("jsonSchema")
                    .filter(|value| value.is_object())
                    .cloned(),
            })
        })
        .collect()
}

fn read_output_selector(output: &serde_json::Value) -> Option<Vec<String>> {
    let selector = output.get("selector")?.as_array()?;
    let segments = selector
        .iter()
        .filter_map(|segment| segment.as_str().map(str::to_string))
        .collect::<Vec<_>>();

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

fn required_output_string(output: &serde_json::Value, field: &'static str) -> Result<String> {
    output
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ControlPlaneError::InvalidInput(field).into())
}

pub(super) fn node_contribution_lookup_key(
    plugin_id: &str,
    plugin_version: &str,
    contribution_code: &str,
    node_shell: &str,
    schema_version: &str,
) -> String {
    format!("{plugin_id}::{plugin_version}::{contribution_code}::{node_shell}::{schema_version}")
}

pub(super) fn allow_custom_models(instance: &domain::ModelProviderInstanceRecord) -> bool {
    instance.enabled_model_ids.is_empty()
}

async fn available_models_for_instance<R>(
    repository: &R,
    instance: &domain::ModelProviderInstanceRecord,
) -> Result<BTreeSet<String>>
where
    R: ModelProviderRepository,
{
    if !instance.enabled_model_ids.is_empty() {
        return Ok(instance.enabled_model_ids.iter().cloned().collect());
    }

    let catalog_models = repository
        .list_catalog_entries_for_provider_instance(instance.id)
        .await?
        .into_iter()
        .filter(|entry| entry.status == "active")
        .map(|entry| entry.upstream_model_id)
        .collect::<BTreeSet<_>>();

    Ok(catalog_models)
}

async fn installation_is_runnable<R>(
    repository: &R,
    installation_id: Uuid,
    assigned: bool,
) -> Result<bool>
where
    R: PluginRepository,
{
    if !assigned {
        return Ok(false);
    }
    let Some(installation) = repository.get_installation(installation_id).await? else {
        return Ok(false);
    };

    Ok(!matches!(
        installation.desired_state,
        domain::PluginDesiredState::Disabled
    ) && installation.availability_status == domain::PluginAvailabilityStatus::Available)
}

fn missing_provider_field(message: &str) -> &'static str {
    if message.contains("source_instance_id") {
        "source_instance_id"
    } else {
        "provider_code"
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::{
        errors::ControlPlaneError,
        ports::{
            ApplicationJsDependencySelectionRepository, ModelProviderRepository,
            ReplaceApplicationJsDependencySelectionInput,
        },
    };

    fn llm_document(flow_id: Uuid, provider_code: &str, model_id: &str) -> Value {
        let model_provider = json!({
            "provider_code": provider_code,
            "model_id": model_id,
        });

        json!({
            "schemaVersion": "1flowbase.flow/v2",
            "meta": {
                "flowId": flow_id.to_string(),
                "name": "Compile Context Test",
                "description": "",
                "tags": []
            },
            "graph": {
                "nodes": [
                    {
                        "id": "node-start",
                        "type": "start",
                        "alias": "Start",
                        "description": "",
                        "containerId": null,
                        "position": { "x": 0, "y": 0 },
                        "configVersion": 1,
                        "config": {},
                        "bindings": {},
                        "outputs": []
                    },
                    {
                        "id": "node-llm",
                        "type": "llm",
                        "alias": "LLM",
                        "description": "",
                        "containerId": null,
                        "position": { "x": 240, "y": 0 },
                        "configVersion": 1,
                        "config": {
                            "model_provider": model_provider,
                            "temperature": 0.2
                        },
                        "bindings": {
                            "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "{{node-start.query}}" } }] }
                        },
                        "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                    }
                ],
                "edges": [
                    {
                        "id": "edge-start-llm",
                        "source": "node-start",
                        "target": "node-llm",
                        "sourceHandle": null,
                        "targetHandle": null,
                        "containerId": null,
                        "points": []
                    }
                ]
            },
            "editor": {
                "viewport": { "x": 0, "y": 0, "zoom": 1 },
                "annotations": [],
                "activeContainerPath": []
            }
        })
    }

    fn code_js_dependency_document(flow_id: Uuid, imports: Value) -> Value {
        json!({
            "schemaVersion": "1flowbase.flow/v2",
            "meta": {
                "flowId": flow_id.to_string(),
                "name": "Code Import Test",
                "description": "",
                "tags": []
            },
            "graph": {
                "nodes": [
                    {
                        "id": "node-start",
                        "type": "start",
                        "alias": "Start",
                        "description": "",
                        "containerId": null,
                        "position": { "x": 0, "y": 0 },
                        "configVersion": 1,
                        "config": {},
                        "bindings": {},
                        "outputs": []
                    },
                    {
                        "id": "node-code",
                        "type": "code",
                        "alias": "Code",
                        "description": "",
                        "containerId": null,
                        "position": { "x": 240, "y": 0 },
                        "configVersion": 1,
                        "config": { "imports": imports },
                        "bindings": {},
                        "outputs": [{ "key": "result", "title": "Result", "valueType": "json" }]
                    }
                ],
                "edges": [
                    {
                        "id": "edge-start-code",
                        "source": "node-start",
                        "target": "node-code",
                        "sourceHandle": null,
                        "targetHandle": null,
                        "containerId": null,
                        "points": []
                    }
                ]
            },
            "editor": {
                "viewport": { "x": 0, "y": 0, "zoom": 1 },
                "annotations": [],
                "activeContainerPath": []
            }
        })
    }

    async fn compile_error_field(
        repository: &super::super::test_support::InMemoryOrchestrationRuntimeRepository,
        document: &Value,
    ) -> String {
        let compile_context = build_compile_context(repository, Uuid::nil())
            .await
            .expect("compile context should build");
        let compiled_plan = orchestration_runtime::compiler::FlowCompiler::compile(
            Uuid::now_v7(),
            "draft-1",
            document,
            &compile_context,
        )
        .expect("plan should compile");
        let error = ensure_compiled_plan_runnable(&compiled_plan).expect_err("plan should fail");
        match error.downcast_ref::<ControlPlaneError>() {
            Some(ControlPlaneError::InvalidInput(field)) => (*field).to_string(),
            other => panic!("expected invalid input error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn orchestration_runtime_code_js_dependency_context_includes_application_selection() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let application_id = Uuid::now_v7();

        ApplicationJsDependencySelectionRepository::replace_application_js_dependency_selection(
            &repository,
            &ReplaceApplicationJsDependencySelectionInput {
                actor_user_id: Uuid::nil(),
                workspace_id: Uuid::nil(),
                application_id,
                installation_id: Uuid::now_v7(),
                provider_code: "fixture_js_dependency_pack".into(),
                plugin_id: "fixture_js_dependency_pack@3.24.0".into(),
                plugin_version: "3.24.0".into(),
                alias: "zod".into(),
                package: "zod".into(),
                version: "3.24.0".into(),
                target: "backend_code".into(),
                artifact_path: "artifacts/zod-3.24.0.backend.mjs".into(),
                artifact_hash: "sha256-zod-3.24.0".into(),
                integrity: "sha256-zod-3.24.0".into(),
                permissions: domain::JsDependencyPermissions {
                    network: "outbound_only".into(),
                    filesystem: "deny".into(),
                    env: "deny".into(),
                },
            },
        )
        .await
        .expect("selection should be stored");

        let context = build_application_compile_context(&repository, Uuid::nil(), application_id)
            .await
            .expect("application compile context should build");

        assert!(context.js_dependencies.contains_key("backend_code::zod"));
        let dependency = context
            .js_dependencies
            .get("backend_code::zod")
            .expect("zod dependency context should be present");
        assert_eq!(dependency.artifact_path, "artifacts/zod-3.24.0.backend.mjs");
        assert_eq!(dependency.artifact_hash, "sha256-zod-3.24.0");
        assert_eq!(dependency.integrity, "sha256-zod-3.24.0");
    }

    #[tokio::test]
    async fn orchestration_runtime_code_js_dependency_missing_import_maps_to_imports_field() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );

        let field = compile_error_field(
            &repository,
            &code_js_dependency_document(Uuid::now_v7(), json!(["zod"])),
        )
        .await;

        assert_eq!(field, "imports");
    }

    #[tokio::test]
    async fn code_isolation_compile_issue_maps_to_isolation_field() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let mut document = code_js_dependency_document(Uuid::now_v7(), json!([]));
        document["graph"]["nodes"][1]["config"]["isolation"] = serde_json::json!({
            "mode": "process"
        });

        let field = compile_error_field(&repository, &document).await;

        assert_eq!(field, "isolation");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_requires_provider_code() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );

        let field = compile_error_field(
            &repository,
            &llm_document(Uuid::now_v7(), "", "gpt-5.4-mini"),
        )
        .await;

        assert_eq!(field, "provider_code");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_rejects_unknown_provider_code() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );

        let field = compile_error_field(
            &repository,
            &llm_document(Uuid::now_v7(), "missing_provider", "gpt-5.4-mini"),
        )
        .await;

        assert_eq!(field, "provider_code");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_maps_context_selector_issue_to_context_policy() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let mut document = llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini");
        document["graph"]["nodes"][1]["config"]["context_policy"] = json!({
            "integration_context": "enabled",
            "context_selector": ["node-start", "missing_history"]
        });

        let field = compile_error_field(&repository, &document).await;

        assert_eq!(field, "context_policy");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_rejects_non_ready_provider_instance() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let instance_id = repository.default_provider_instance_id();
        repository.set_instance_status(instance_id, domain::ModelProviderInstanceStatus::Disabled);

        let field = compile_error_field(
            &repository,
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
        )
        .await;

        assert_eq!(field, "provider_code");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_ignores_instances_outside_main_aggregation() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let _excluded_instance_id = repository.seed_provider_instance(
            "fixture_provider",
            "Excluded",
            false,
            domain::ModelProviderInstanceStatus::Ready,
            vec!["gpt-5.4-mini"],
        );

        let compile_context = build_compile_context(&repository, Uuid::nil())
            .await
            .expect("compile context should build");
        let compiled_plan = orchestration_runtime::compiler::FlowCompiler::compile(
            Uuid::now_v7(),
            "draft-1",
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
            &compile_context,
        )
        .expect("plan should compile");

        assert!(
            compiled_plan.compile_issues.is_empty(),
            "excluded provider instance should not affect stable provider binding, got {:?}",
            compiled_plan.compile_issues
        );
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_uses_failover_queue_for_multi_instance_stable_model(
    ) {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        repository.seed_provider_instance(
            "fixture_provider",
            "Duplicate Model Set",
            true,
            domain::ModelProviderInstanceStatus::Ready,
            vec!["gpt-5.4-mini"],
        );

        let compile_context = build_compile_context(&repository, Uuid::nil())
            .await
            .expect("compile context should build");
        let compiled_plan = orchestration_runtime::compiler::FlowCompiler::compile(
            Uuid::now_v7(),
            "draft-1",
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
            &compile_context,
        )
        .expect("plan should compile");

        ensure_compiled_plan_runnable(&compiled_plan).expect("multi-instance provider should run");
        let routing = compiled_plan.nodes["node-llm"]
            .llm_runtime
            .as_ref()
            .expect("llm runtime")
            .routing
            .as_ref()
            .expect("llm routing");
        assert_eq!(
            routing.routing_mode,
            orchestration_runtime::compiled_plan::LlmRoutingMode::FailoverQueue
        );
        assert_eq!(routing.queue_targets.len(), 2);
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_validates_model_on_stable_provider() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let instance_id = repository.default_provider_instance_id();
        repository.set_instance_enabled_models(instance_id, vec!["other-model"]);

        let field = compile_error_field(
            &repository,
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
        )
        .await;

        assert_eq!(field, "model");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_does_not_expand_enabled_models_from_catalog_cache(
    ) {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let selected_instance_id = repository.default_provider_instance_id();
        repository.set_instance_enabled_models(selected_instance_id, vec!["other-model"]);
        repository
            .set_instance_catalog_models(selected_instance_id, vec!["other-model", "gpt-5.4-mini"]);

        let field = compile_error_field(
            &repository,
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
        )
        .await;

        assert_eq!(field, "model");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_uses_catalog_entries_as_model_source() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let default_instance_id = repository.default_provider_instance_id();
        repository.set_instance_enabled_models(default_instance_id, vec!["other-model"]);
        let selected_instance_id = repository.seed_provider_instance(
            "fixture_provider",
            "Catalog Entry Source",
            true,
            domain::ModelProviderInstanceStatus::Ready,
            vec![],
        );
        repository.set_instance_catalog_models(selected_instance_id, vec!["cache-only-model"]);
        repository.seed_catalog_entries_for_instance(selected_instance_id, vec!["gpt-5.4-mini"]);

        let compile_context = build_compile_context(&repository, Uuid::nil())
            .await
            .expect("compile context should build");
        let compiled_plan = orchestration_runtime::compiler::FlowCompiler::compile(
            Uuid::now_v7(),
            "draft-1",
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
            &compile_context,
        )
        .expect("plan should compile");

        assert!(
            compiled_plan.compile_issues.is_empty(),
            "catalog entry should be the model source, got {:?}",
            compiled_plan.compile_issues
        );
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_rejects_provider_when_installation_unassigned() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let instance_id = repository.default_provider_instance_id();
        let installation_id =
            ModelProviderRepository::get_instance(&repository, Uuid::nil(), instance_id)
                .await
                .expect("instance lookup should succeed")
                .expect("instance should exist")
                .installation_id;
        repository.remove_assignment_for_installation(Uuid::nil(), installation_id);

        let field = compile_error_field(
            &repository,
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
        )
        .await;

        assert_eq!(field, "provider_code");
    }

    #[tokio::test]
    async fn orchestration_runtime_compile_context_rejects_provider_when_installation_disabled() {
        let repository =
            super::super::test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(
                vec![],
            );
        let instance_id = repository.default_provider_instance_id();
        let installation_id =
            ModelProviderRepository::get_instance(&repository, Uuid::nil(), instance_id)
                .await
                .expect("instance lookup should succeed")
                .expect("instance should exist")
                .installation_id;
        repository.set_installation_state(
            installation_id,
            domain::PluginDesiredState::Disabled,
            domain::PluginAvailabilityStatus::Disabled,
        );

        let field = compile_error_field(
            &repository,
            &llm_document(Uuid::now_v7(), "fixture_provider", "gpt-5.4-mini"),
        )
        .await;

        assert_eq!(field, "provider_code");
    }
}
