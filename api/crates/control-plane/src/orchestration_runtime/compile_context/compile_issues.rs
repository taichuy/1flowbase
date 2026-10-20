use super::*;

pub(in super::super) fn ensure_compiled_plan_runnable(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
) -> Result<()> {
    if let Some(issue) = compiled_plan.compile_issues.first() {
        return Err(
            anyhow::Error::new(ControlPlaneError::InvalidInput(compile_issue_field(issue)))
                .context(
                    orchestration_runtime::compiler::FlowValidationError::from_issues(
                        &compiled_plan.compile_issues,
                    ),
                ),
        );
    }
    orchestration_runtime::compiler::ensure_plan_execution_contract(compiled_plan)?;

    Ok(())
}

pub(in super::super) fn ensure_compiled_plan_runnable_for_node(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    target_node_id: &str,
) -> Result<()> {
    let target_node_scope = collect_target_node_dependency_scope(compiled_plan, target_node_id);
    let blocking_issue = compiled_plan
        .compile_issues
        .iter()
        .find(|issue| target_node_scope.contains(issue.node_id.as_str()));

    if let Some(issue) = blocking_issue {
        return Err(
            anyhow::Error::new(ControlPlaneError::InvalidInput(compile_issue_field(issue)))
                .context(
                    orchestration_runtime::compiler::FlowValidationError::from_issues(
                        compiled_plan
                            .compile_issues
                            .iter()
                            .filter(|issue| target_node_scope.contains(issue.node_id.as_str())),
                    ),
                ),
        );
    }

    Ok(())
}

pub(super) fn collect_target_node_dependency_scope(
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

pub(super) fn compile_issue_field(
    issue: &orchestration_runtime::compiled_plan::CompileIssue,
) -> &'static str {
    match issue.code {
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingDataSourceInstance
        | orchestration_runtime::compiled_plan::CompileIssueCode::InvalidDataSourceInstance => {
            "data_source_instance_id"
        }
        orchestration_runtime::compiled_plan::CompileIssueCode::MissingNativeSql => "sql",
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
