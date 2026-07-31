use std::collections::{BTreeMap, HashMap};

use sha2::{Digest, Sha256};

use super::code_runtime_config::{
    code_import_aliases, compile_code_isolation_profile, trimmed_config_string,
    validate_code_imports,
};
use super::selector_paths::{compile_variable_reference, extract_selector_paths};
use super::*;

const NODE_CONTRIBUTION_SCHEMA_VERSION: &str = "1flowbase.node-contribution/v2";

pub(super) fn compile_node(
    node: &Value,
    context: &FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) -> Result<CompiledNode> {
    let node_id = required_string(node, "id")?.to_string();
    let node_type = required_string(node, "type")?.to_string();
    let alias = required_string(node, "alias")?.to_string();
    let container_id = optional_string(node, "containerId")?.map(str::to_string);
    let mut config = node
        .get("config")
        .cloned()
        .unwrap_or(Value::Object(Default::default()));
    if node_type == "llm" {
        let protocol_context = match config.get("protocol_context") {
            None => Some(crate::compiled_plan::VariableReference::system_protocol_context()),
            value => {
                compile_variable_reference("config.protocol_context", value).with_context(|| {
                    format!("failed to compile protocol context reference for node {node_id}")
                })?
            }
        };
        config
            .as_object_mut()
            .ok_or_else(|| anyhow!("node {node_id} config must be an object"))?
            .insert(
                "protocol_context".to_string(),
                serde_json::to_value(protocol_context)?,
            );
    }
    let raw_bindings = node
        .get("bindings")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("node {node_id} missing bindings"))?;
    let active_bindings = active_binding_values(&node_type, raw_bindings);
    let bindings = compile_bindings(&active_bindings)
        .with_context(|| format!("failed to compile bindings for node {node_id}"))?;
    let mut outputs = compile_outputs(
        node.get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("node {node_id} missing outputs"))?,
    )
    .with_context(|| format!("failed to compile outputs for node {node_id}"))?;
    if node_type == "code" {
        for output in &mut outputs {
            if output.selector.len() == 1 && output.selector[0] == output.key {
                output.selector = vec!["result".to_string(), output.key.clone()];
            }
        }
    }
    if node_type == "start" && !outputs.is_empty() {
        bail!("start node {node_id} outputs must be empty");
    }
    if node_type == "unresolved_node" {
        compile_issues.push(CompileIssue {
            node_id: node_id.clone(),
            code: CompileIssueCode::UnresolvedNode,
            message: unresolved_node_message(&node_id, &config),
        });
    }
    if node_type == "sql" {
        validate_native_sql_config(&node_id, &config, &bindings, compile_issues);
    }
    let llm_runtime = (node_type == "llm")
        .then(|| compile_llm_runtime(&node_id, &config, context, compile_issues))
        .flatten();
    let plugin_runtime = (node_type == "plugin_node")
        .then(|| compile_plugin_runtime(&node_id, node, &outputs, context, compile_issues))
        .flatten();
    let code_runtime = (node_type == "code").then(|| {
        validate_code_imports(&node_id, &config, context, compile_issues);
        compile_code_runtime(&node_id, &config, context, compile_issues)
    });

    Ok(CompiledNode {
        node_id,
        node_type,
        alias,
        container_id,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings,
        outputs,
        config,
        plugin_runtime,
        llm_runtime,
        code_runtime,
    })
}

fn validate_native_sql_config(
    node_id: &str,
    config: &Value,
    bindings: &BTreeMap<String, CompiledBinding>,
    compile_issues: &mut Vec<CompileIssue>,
) {
    let data_source_instance_id = config
        .get("data_source_instance_id")
        .and_then(Value::as_str);
    match data_source_instance_id {
        None | Some("") => compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingDataSourceInstance,
            message: format!("node {node_id} is missing config.data_source_instance_id"),
        }),
        Some("main") => {}
        Some(value) if uuid::Uuid::parse_str(value).is_ok() => {}
        Some(_) => compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::InvalidDataSourceInstance,
            message: format!("node {node_id} has an invalid config.data_source_instance_id"),
        }),
    }

    if !bindings
        .get("sql")
        .is_some_and(|binding| binding.kind == "templated_text" && binding.raw_value.is_string())
    {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingNativeSql,
            message: format!("node {node_id} is missing bindings.sql templated_text"),
        });
    }
}

fn unresolved_node_message(node_id: &str, config: &Value) -> String {
    let unresolved = config.get("unresolved");
    let reason = unresolved
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or("missing_dependency");
    let original_type = unresolved
        .and_then(|value| value.get("original_type"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    format!("node {node_id} is unresolved ({original_type}): {reason}")
}

fn compile_code_runtime(
    node_id: &str,
    config: &Value,
    context: &FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) -> CompiledCodeRuntime {
    let language = trimmed_config_string(config, "language").unwrap_or("javascript");
    let source = trimmed_config_string(config, "source").map(str::to_string);
    let source_ref = trimmed_config_string(config, "source_ref")
        .or_else(|| trimmed_config_string(config, "sourceRef"))
        .map(str::to_string);
    let entrypoint = trimmed_config_string(config, "entrypoint").unwrap_or("main");
    let imports = code_import_aliases(config);
    let dependencies = imports
        .iter()
        .filter_map(|alias| {
            let key = js_dependency_lookup_key("backend_code", alias);
            context
                .js_dependencies
                .get(&key)
                .map(|dependency| CompiledCodeDependency {
                    alias: dependency.alias.clone(),
                    target: dependency.target.clone(),
                    artifact_path: dependency.artifact_path.clone(),
                    artifact_hash: dependency.artifact_hash.clone(),
                    integrity: dependency.integrity.clone(),
                })
        })
        .collect();

    CompiledCodeRuntime {
        language: language.to_string(),
        source,
        source_ref,
        entrypoint: entrypoint.to_string(),
        imports,
        dependencies,
        isolation_profile: compile_code_isolation_profile(node_id, config, compile_issues),
    }
}

fn compile_llm_runtime(
    node_id: &str,
    config: &Value,
    context: &FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) -> Option<CompiledLlmRuntime> {
    let provider_config = config.get("model_provider");
    if provider_config
        .and_then(|value| value.get("routing_mode"))
        .and_then(Value::as_str)
        .is_some_and(|value| value == "failover_queue")
    {
        return compile_failover_queue_runtime(
            node_id,
            provider_config,
            config,
            context,
            compile_issues,
        );
    }

    let provider_code = provider_config
        .and_then(|value| value.get("provider_code"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let model = provider_config
        .and_then(|value| value.get("model_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let Some(provider_code) = provider_code else {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingProviderInstance,
            message: format!("node {node_id} is missing config.model_provider.provider_code"),
        });
        if model.is_none() {
            compile_issues.push(CompileIssue {
                node_id: node_id.to_string(),
                code: CompileIssueCode::MissingModel,
                message: format!("node {node_id} is missing config.model_provider.model_id"),
            });
        }
        return None;
    };

    let Some(model) = model else {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingModel,
            message: format!("node {node_id} is missing config.model_provider.model_id"),
        });
        return None;
    };

    let routing_policy = context
        .model_routing_policies
        .get(&(provider_code.clone(), model.clone()));
    let provider_instances = resolve_fixed_model_provider_instances(
        node_id,
        &provider_code,
        &model,
        routing_policy,
        context,
        compile_issues,
    )?;
    let provider_instance = provider_instances.first().copied()?;

    let context_policy = compile_llm_context_policy(config);
    let distribution_rule = routing_policy
        .map(|policy| policy.distribution_rule)
        .unwrap_or_default();

    Some(CompiledLlmRuntime {
        provider_instance_id: provider_instance.provider_instance_id.clone(),
        provider_instance_display_name: provider_instance.display_name.clone(),
        provider_code: provider_instance.provider_code.clone(),
        protocol: provider_instance.protocol.clone(),
        model: model.clone(),
        routing: Some(fixed_model_routing(
            &provider_instances,
            context.workspace_id,
            &provider_code,
            &model,
            distribution_rule,
            context_policy,
        )),
    })
}

fn compile_llm_context_policy(config: &Value) -> Value {
    config
        .get("context_policy")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({ "integration_context": "enabled" }))
}

fn resolve_fixed_model_provider_instances<'a>(
    node_id: &str,
    provider_code: &str,
    model: &str,
    routing_policy: Option<&FlowCompileModelRoutingPolicy>,
    context: &'a FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) -> Option<Vec<&'a FlowCompileProviderInstance>> {
    if !context.provider_families.contains_key(provider_code) {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::ProviderInstanceNotFound,
            message: format!("provider {provider_code} was not found"),
        });
        return None;
    }

    let candidates = context
        .provider_instances
        .values()
        .filter(|instance| instance.provider_code == provider_code && instance.included_in_main)
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::ProviderInstanceNotFound,
            message: format!("provider {provider_code} has no included runtime instance"),
        });
        return None;
    }

    let model_candidates = candidates
        .into_iter()
        .filter(|instance| provider_instance_supports_model(instance, model))
        .collect::<Vec<_>>();

    if model_candidates.is_empty() {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::ModelNotAvailable,
            message: format!("model {model} is not available for provider {provider_code}"),
        });
        return None;
    }

    let mut runnable_candidates = model_candidates
        .iter()
        .copied()
        .filter(|instance| instance.is_ready && instance.is_runnable)
        .collect::<Vec<_>>();

    if !runnable_candidates.is_empty() {
        let configured_positions = routing_policy
            .map(|policy| {
                policy
                    .provider_instance_ids
                    .iter()
                    .enumerate()
                    .map(|(position, id)| (id.as_str(), position))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        runnable_candidates.sort_by(|left, right| {
            configured_positions
                .get(left.provider_instance_id.as_str())
                .copied()
                .unwrap_or(usize::MAX)
                .cmp(
                    &configured_positions
                        .get(right.provider_instance_id.as_str())
                        .copied()
                        .unwrap_or(usize::MAX),
                )
                .then(left.provider_instance_id.cmp(&right.provider_instance_id))
        });
        return Some(runnable_candidates);
    }

    compile_issues.push(CompileIssue {
        node_id: node_id.to_string(),
        code: CompileIssueCode::ProviderInstanceNotReady,
        message: format!("provider {provider_code} has no runnable instance for model {model}"),
    });
    None
}

fn provider_instance_supports_model(instance: &FlowCompileProviderInstance, model: &str) -> bool {
    instance.allow_custom_models
        || instance.available_models.is_empty()
        || instance.available_models.contains(model)
}

fn fixed_model_routing(
    provider_instances: &[&FlowCompileProviderInstance],
    workspace_id: Option<uuid::Uuid>,
    provider_code: &str,
    model: &str,
    distribution_rule: LlmDistributionRule,
    context_policy: Value,
) -> CompiledLlmRouting {
    if provider_instances.len() > 1 {
        let queue_targets = provider_instances
            .iter()
            .map(|provider_instance| CompiledLlmRouteTarget {
                provider_instance_id: provider_instance.provider_instance_id.clone(),
                provider_instance_display_name: provider_instance.display_name.clone(),
                provider_code: provider_instance.provider_code.clone(),
                protocol: provider_instance.protocol.clone(),
                upstream_model_id: model.to_string(),
            })
            .collect::<Vec<_>>();
        let distribution_key = (distribution_rule == LlmDistributionRule::RoundRobin)
            .then(|| llm_distribution_key(workspace_id, provider_code, model, &queue_targets));

        return CompiledLlmRouting {
            routing_mode: LlmRoutingMode::FailoverQueue,
            fixed_model_target: None,
            queue_template_id: None,
            queue_snapshot_id: None,
            queue_targets,
            distribution_rule,
            distribution_key,
            context_policy,
            stream_policy: serde_json::json!({}),
        };
    }

    let provider_instance = provider_instances[0];

    CompiledLlmRouting {
        routing_mode: LlmRoutingMode::FixedModel,
        fixed_model_target: Some(serde_json::json!({
            "provider_instance_id": provider_instance.provider_instance_id.clone(),
            "provider_code": provider_instance.provider_code.clone(),
            "protocol": provider_instance.protocol.clone(),
            "upstream_model_id": model,
        })),
        queue_template_id: None,
        queue_snapshot_id: None,
        queue_targets: Vec::new(),
        distribution_rule: LlmDistributionRule::None,
        distribution_key: None,
        context_policy,
        stream_policy: serde_json::json!({}),
    }
}

fn llm_distribution_key(
    workspace_id: Option<uuid::Uuid>,
    provider_code: &str,
    model: &str,
    targets: &[CompiledLlmRouteTarget],
) -> String {
    let workspace_segment = workspace_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unscoped".to_string());
    let mut target_fingerprint = Sha256::new();
    for target in targets {
        target_fingerprint.update(target.provider_instance_id.as_bytes());
        target_fingerprint.update(b"\0");
        target_fingerprint.update(target.provider_code.as_bytes());
        target_fingerprint.update(b"\0");
        target_fingerprint.update(target.protocol.as_bytes());
        target_fingerprint.update(b"\0");
        target_fingerprint.update(target.upstream_model_id.as_bytes());
        target_fingerprint.update(b"\0");
    }
    let target_fingerprint = format!("{:x}", target_fingerprint.finalize());

    format!(
        "llm-router:workspace:{workspace_segment}:provider:{provider_code}:model:{model}:targets:{target_fingerprint}"
    )
}

fn compile_failover_queue_runtime(
    node_id: &str,
    provider_config: Option<&Value>,
    config: &Value,
    context: &FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) -> Option<CompiledLlmRuntime> {
    let queue_template_id = provider_config
        .and_then(|value| value.get("queue_template_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let queue_snapshot_id = provider_config
        .and_then(|value| value.get("queue_snapshot_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let raw_targets = provider_config
        .and_then(|value| value.get("queue_targets"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if queue_template_id.is_none() {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingProviderInstance,
            message: format!("node {node_id} is missing config.model_provider.queue_template_id"),
        });
    }
    let mut targets = Vec::new();
    for (index, target) in raw_targets.iter().enumerate() {
        let Some(compiled_target) =
            compile_failover_queue_target(node_id, index, target, context, compile_issues)
        else {
            continue;
        };
        targets.push(compiled_target);
    }
    let first_target = targets.first().cloned().unwrap_or(CompiledLlmRouteTarget {
        provider_instance_id: String::new(),
        provider_instance_display_name: String::new(),
        provider_code: String::new(),
        protocol: String::new(),
        upstream_model_id: String::new(),
    });

    Some(CompiledLlmRuntime {
        provider_instance_id: first_target.provider_instance_id.clone(),
        provider_instance_display_name: first_target.provider_instance_display_name.clone(),
        provider_code: first_target.provider_code.clone(),
        protocol: first_target.protocol.clone(),
        model: first_target.upstream_model_id.clone(),
        routing: Some(CompiledLlmRouting {
            routing_mode: LlmRoutingMode::FailoverQueue,
            fixed_model_target: None,
            queue_template_id,
            queue_snapshot_id,
            queue_targets: targets,
            distribution_rule: LlmDistributionRule::None,
            distribution_key: None,
            context_policy: compile_llm_context_policy(config),
            stream_policy: serde_json::json!({}),
        }),
    })
}

fn compile_failover_queue_target(
    node_id: &str,
    index: usize,
    target: &Value,
    context: &FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) -> Option<CompiledLlmRouteTarget> {
    let provider_instance_id = target
        .get("provider_instance_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let upstream_model_id = target
        .get("upstream_model_id")
        .or_else(|| target.get("model_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let Some(provider_instance_id) = provider_instance_id else {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingProviderInstance,
            message: format!(
                "node {node_id} failover target {index} is missing provider_instance_id"
            ),
        });
        return None;
    };
    let Some(upstream_model_id) = upstream_model_id else {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingModel,
            message: format!("node {node_id} failover target {index} is missing upstream_model_id"),
        });
        return None;
    };
    let Some(provider_instance) = context.provider_instances.get(&provider_instance_id) else {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::ProviderInstanceNotFound,
            message: format!(
                "failover target provider_instance_id {provider_instance_id} was not found"
            ),
        });
        return None;
    };

    if !provider_instance.is_ready
        || !provider_instance.is_runnable
        || !provider_instance.included_in_main
    {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::ProviderInstanceNotReady,
            message: format!(
                "failover target provider_instance_id {provider_instance_id} is not runnable"
            ),
        });
    }
    if !provider_instance.allow_custom_models
        && !provider_instance.available_models.is_empty()
        && !provider_instance
            .available_models
            .contains(&upstream_model_id)
    {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::ModelNotAvailable,
            message: format!(
                "model {upstream_model_id} is not available for failover target provider_instance_id {provider_instance_id}"
            ),
        });
    }

    Some(CompiledLlmRouteTarget {
        provider_instance_id: provider_instance.provider_instance_id.clone(),
        provider_instance_display_name: provider_instance.display_name.clone(),
        provider_code: target
            .get("provider_code")
            .and_then(Value::as_str)
            .unwrap_or(&provider_instance.provider_code)
            .to_string(),
        protocol: target
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or(&provider_instance.protocol)
            .to_string(),
        upstream_model_id,
    })
}

fn compile_plugin_runtime(
    node_id: &str,
    node: &Value,
    compiled_outputs: &[CompiledOutput],
    context: &FlowCompileContext,
    compile_issues: &mut Vec<CompileIssue>,
) -> Option<CompiledPluginRuntime> {
    let schema_version = required_plugin_string(
        node_id,
        node,
        "schema_version",
        CompileIssueCode::MissingSchemaVersion,
        compile_issues,
    )?;
    if schema_version != NODE_CONTRIBUTION_SCHEMA_VERSION {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::UnsupportedPluginContributionSchemaVersion,
            message: format!(
                "node {node_id} uses unsupported plugin contribution schema_version {schema_version}"
            ),
        });
        return None;
    }

    let plugin_unique_identifier = required_plugin_string(
        node_id,
        node,
        "plugin_unique_identifier",
        CompileIssueCode::MissingPluginUniqueIdentifier,
        compile_issues,
    )?;
    let package_id = required_plugin_string(
        node_id,
        node,
        "package_id",
        CompileIssueCode::MissingPackageId,
        compile_issues,
    )?;
    let plugin_id = required_plugin_string(
        node_id,
        node,
        "plugin_id",
        CompileIssueCode::MissingPluginId,
        compile_issues,
    )?;
    let plugin_version = required_plugin_string(
        node_id,
        node,
        "plugin_version",
        CompileIssueCode::MissingPluginVersion,
        compile_issues,
    )?;
    let contribution_code = required_plugin_string(
        node_id,
        node,
        "contribution_code",
        CompileIssueCode::MissingContributionCode,
        compile_issues,
    )?;
    let node_shell = required_plugin_string(
        node_id,
        node,
        "node_shell",
        CompileIssueCode::MissingNodeShell,
        compile_issues,
    )?;
    let contribution_checksum = required_plugin_string(
        node_id,
        node,
        "contribution_checksum",
        CompileIssueCode::MissingContributionChecksum,
        compile_issues,
    )?;
    let compiled_contribution_hash = required_plugin_string(
        node_id,
        node,
        "compiled_contribution_hash",
        CompileIssueCode::MissingCompiledContributionHash,
        compile_issues,
    )?;
    let output_schema_snapshot = compile_output_schema_snapshot(node_id, node, compile_issues)?;

    let lookup_key = node_contribution_lookup_key(
        &plugin_id,
        &plugin_version,
        &contribution_code,
        &node_shell,
        &schema_version,
    );
    let Some(contribution) = context.node_contributions.get(&lookup_key) else {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingPluginContribution,
            message: format!(
                "node {node_id} missing workspace contribution for {plugin_id}:{plugin_version}:{contribution_code}"
            ),
        });
        return None;
    };

    if contribution.plugin_unique_identifier != plugin_unique_identifier
        || contribution.package_id != package_id
    {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingPluginContribution,
            message: format!("node {node_id} contribution identity no longer matches registry"),
        });
    }

    if contribution.dependency_status != "ready" {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::PluginContributionDependencyNotReady,
            message: format!(
                "node {node_id} contribution {contribution_code} is not ready: {}",
                contribution.dependency_status
            ),
        });
    }

    if contribution.contribution_checksum != contribution_checksum
        || contribution.compiled_contribution_hash != compiled_contribution_hash
    {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::PluginContributionChecksumMismatch,
            message: format!(
                "node {node_id} contribution checksum changed for {contribution_code}"
            ),
        });
    }

    if contribution.output_schema_snapshot != output_schema_snapshot
        || compiled_outputs != output_schema_snapshot
    {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::PluginContributionOutputSchemaMismatch,
            message: format!(
                "node {node_id} output schema snapshot changed for {contribution_code}"
            ),
        });
    }

    Some(CompiledPluginRuntime {
        installation_id: contribution.installation_id,
        plugin_unique_identifier: contribution.plugin_unique_identifier.clone(),
        package_id: contribution.package_id.clone(),
        plugin_id: contribution.plugin_id.clone(),
        plugin_version: contribution.plugin_version.clone(),
        contribution_code: contribution.contribution_code.clone(),
        node_shell: contribution.node_shell.clone(),
        schema_version: contribution.schema_version.clone(),
        contribution_checksum: contribution.contribution_checksum.clone(),
        compiled_contribution_hash: contribution.compiled_contribution_hash.clone(),
        output_schema_snapshot: contribution.output_schema_snapshot.clone(),
        side_effect_policy: contribution.side_effect_policy.clone(),
    })
}

fn compile_output_schema_snapshot(
    node_id: &str,
    node: &Value,
    compile_issues: &mut Vec<CompileIssue>,
) -> Option<Vec<CompiledOutput>> {
    let Some(outputs) = node
        .get("output_schema_snapshot")
        .and_then(|snapshot| snapshot.get("outputs"))
        .and_then(Value::as_array)
    else {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code: CompileIssueCode::MissingOutputSchemaSnapshot,
            message: format!("node {node_id} missing output_schema_snapshot.outputs"),
        });
        return None;
    };

    match compile_outputs(outputs) {
        Ok(outputs) => Some(outputs),
        Err(error) => {
            compile_issues.push(CompileIssue {
                node_id: node_id.to_string(),
                code: CompileIssueCode::PluginContributionOutputSchemaMismatch,
                message: format!("node {node_id} has invalid output_schema_snapshot: {error}"),
            });
            None
        }
    }
}

fn required_plugin_string(
    node_id: &str,
    node: &Value,
    field: &str,
    code: CompileIssueCode,
    compile_issues: &mut Vec<CompileIssue>,
) -> Option<String> {
    let value = node
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if value.is_none() {
        compile_issues.push(CompileIssue {
            node_id: node_id.to_string(),
            code,
            message: format!("node {node_id} missing {field}"),
        });
    }

    value
}

fn node_contribution_lookup_key(
    plugin_id: &str,
    plugin_version: &str,
    contribution_code: &str,
    node_shell: &str,
    schema_version: &str,
) -> String {
    format!("{plugin_id}::{plugin_version}::{contribution_code}::{node_shell}::{schema_version}")
}

pub fn js_dependency_lookup_key(target: &str, alias: &str) -> String {
    format!("{target}::{alias}")
}

fn compile_bindings(
    binding_values: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, CompiledBinding>> {
    let mut bindings = BTreeMap::new();

    for (binding_key, binding_value) in binding_values {
        let kind = required_string(binding_value, "kind")
            .with_context(|| format!("binding {binding_key} missing kind"))?;
        let raw_value = binding_value.get("value").cloned().unwrap_or(Value::Null);
        let i18n_text_ref =
            if kind == "i18n_text" {
                Some(compile_i18n_text_ref(binding_value).with_context(|| {
                    format!("binding {binding_key} has invalid i18n_text payload")
                })?)
            } else {
                None
            };
        let selector_paths = extract_selector_paths(kind, &raw_value)
            .with_context(|| format!("binding {binding_key} has invalid selector payload"))?;

        bindings.insert(
            binding_key.clone(),
            CompiledBinding {
                i18n_text_ref,
                kind: kind.to_string(),
                raw_value,
                selector_paths,
            },
        );
    }

    Ok(bindings)
}

fn compile_i18n_text_ref(binding: &Value) -> Result<CompiledI18nTextRef> {
    let binding_object = binding
        .as_object()
        .ok_or_else(|| anyhow!("i18n_text binding must be an object"))?;
    if binding_object.len() != 2
        || !binding_object.contains_key("kind")
        || !binding_object.contains_key("value")
    {
        bail!("i18n_text binding must contain only kind and value");
    }

    let value = binding_object["value"]
        .as_object()
        .ok_or_else(|| anyhow!("i18n_text value must be an object"))?;
    if value.len() != 1 || !value.contains_key("key") {
        bail!("i18n_text value must contain only key");
    }

    let key = value["key"]
        .as_str()
        .ok_or_else(|| anyhow!("i18n_text key must be a string"))?;
    validate_i18n_english_key(key)?;

    Ok(CompiledI18nTextRef {
        key: key.to_string(),
    })
}

fn validate_i18n_english_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        bail!("i18n_text key must be nonempty English text");
    }
    if key.contains("{{") || key.contains("}}") {
        bail!("i18n_text key must not contain workflow template delimiters");
    }

    let lowercase_key = key.to_ascii_lowercase();
    if key.contains('<')
        || key.contains('>')
        || lowercase_key.contains("javascript:")
        || lowercase_key.contains("data:text/html")
    {
        bail!("i18n_text key must be plain text without HTML or JavaScript content");
    }

    let mut cursor = 0;
    while let Some(start_offset) = key[cursor..].find('{') {
        let start = cursor + start_offset;
        if key[cursor..start].contains('}') {
            bail!("i18n_text key contains an invalid named placeholder");
        }
        let Some(end_offset) = key[start + 1..].find('}') else {
            bail!("i18n_text key contains an invalid named placeholder");
        };
        let end = start + 1 + end_offset;
        let placeholder = &key[start + 1..end];
        let mut characters = placeholder.chars();
        let valid = matches!(characters.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
            && characters.all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !valid {
            bail!("i18n_text key contains an invalid named placeholder");
        }
        cursor = end + 1;
    }
    if key[cursor..].contains('}') {
        bail!("i18n_text key contains an invalid named placeholder");
    }

    Ok(())
}

fn active_binding_values(
    node_type: &str,
    binding_values: &serde_json::Map<String, Value>,
) -> BTreeMap<String, Value> {
    if node_type == "llm" {
        return binding_values
            .iter()
            .filter(|(key, _)| key.as_str() == "prompt_messages")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
    }

    let Some(active_keys) = active_data_model_binding_keys(node_type) else {
        return binding_values
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
    };

    binding_values
        .iter()
        .filter(|(key, _)| active_keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn active_data_model_binding_keys(node_type: &str) -> Option<&'static [&'static str]> {
    let action = match node_type {
        "data_model_list" => "list",
        "data_model_get" => "get",
        "data_model_create" => "create",
        "data_model_update" => "update",
        "data_model_delete" => "delete",
        _ => return None,
    };

    Some(match action {
        "get" => &["record_id"],
        "create" => &["payload"],
        "update" => &["record_id", "payload"],
        "delete" => &["record_id"],
        _ => &["query"],
    })
}

fn compile_outputs(output_values: &[Value]) -> Result<Vec<CompiledOutput>> {
    let outputs: Vec<CompiledOutput> = output_values
        .iter()
        .map(|output| {
            let key = required_string(output, "key")?.to_string();
            Ok(CompiledOutput {
                selector: read_output_selector(output).unwrap_or_else(|| vec![key.clone()]),
                key,
                title: required_string(output, "title")?.to_string(),
                value_type: required_string(output, "valueType")?.to_string(),
                json_schema: output
                    .get("jsonSchema")
                    .filter(|value| value.is_object())
                    .cloned(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    PublicOutputContract::from_compiled_outputs(&outputs)?;

    Ok(outputs)
}

fn read_output_selector(output: &Value) -> Option<Vec<String>> {
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

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("{key} missing"))
}

fn optional_string<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>> {
    match value.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.as_str())),
        Some(_) => bail!("{key} must be a string or null"),
    }
}
