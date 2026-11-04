use super::output_contract::{
    output_schema, schema_value_type, selector_contract, source_outputs, PathContract,
};
use super::*;
use domain::flow::{
    FlowAnalysisDiagnostic, FlowDocumentAnalysis, FlowNodeOutputs, FlowResolvedOutput,
    FlowVariableCatalog, FlowVariableOption,
};

mod variables;

pub fn validate_flow_output_contracts(document: &Value) -> Result<()> {
    for node in document
        .pointer("/graph/nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let node_type = node["type"].as_str().unwrap_or_default();
        if let Some(outputs) = node["outputs"].as_array() {
            super::output_contract::resolve_outputs(node_type, outputs).map_err(|error| {
                FlowValidationError::at_node(
                    node["id"].as_str().unwrap_or_default(),
                    "/outputs",
                    error,
                )
            })?;
        }
    }
    Ok(())
}

/// Analyze an unsaved draft without running nodes or changing repository state.
/// Parsing/output/scope rules are shared with compilation and execution.
pub fn analyze_flow_document(
    document: &Value,
    context: &FlowCompileContext,
) -> FlowDocumentAnalysis {
    analyze_document(document, context, None, None)
}

/// Application-bound consumers use the owning type, never the client document,
/// to choose the compiler's node-family contract.
pub fn analyze_flow_document_for_application(
    document: &Value,
    context: &FlowCompileContext,
    application_type: domain::ApplicationType,
) -> FlowDocumentAnalysis {
    analyze_document(document, context, Some(application_type), None)
}

/// Whole-document diagnostics with a catalog only for the requested editor node.
pub fn analyze_flow_document_for_node(
    document: &Value,
    context: &FlowCompileContext,
    application_type: domain::ApplicationType,
    node_id: &str,
) -> FlowDocumentAnalysis {
    analyze_document(document, context, Some(application_type), Some(node_id))
}

fn analyze_document(
    document: &Value,
    context: &FlowCompileContext,
    application_type: Option<domain::ApplicationType>,
    target_node_id: Option<&str>,
) -> FlowDocumentAnalysis {
    let mut report = FlowDocumentAnalysis::default();
    if let Err(error) = domain::flow::validate_flow_draft_structure(document) {
        append_error(&mut report, FlowValidationError::from(error).into());
        return report;
    }
    let mut nodes = BTreeMap::new();
    for value in document["graph"]["nodes"].as_array().unwrap() {
        let id = value["id"].as_str().unwrap();
        let node_type = value["type"].as_str().unwrap();
        let outputs = match super::output_contract::resolve_outputs(
            node_type,
            value["outputs"].as_array().unwrap(),
        ) {
            Ok(outputs) => outputs,
            Err(error) => {
                append_error(
                    &mut report,
                    FlowValidationError::at_node(id, "/outputs", error),
                );
                Vec::new()
            }
        };
        let values = super::node_compilation::active_binding_values(
            node_type,
            value["bindings"].as_object().unwrap(),
        );
        let mut bindings = BTreeMap::new();
        for (key, binding) in values {
            match super::node_compilation::compile_bindings(id, &BTreeMap::from([(key, binding)])) {
                Ok(parsed) => bindings.extend(parsed),
                Err(error) => append_error(&mut report, error),
            }
        }
        nodes.insert(
            id.to_owned(),
            CompiledNode {
                node_id: id.into(),
                node_type: node_type.into(),
                alias: value["alias"].as_str().unwrap_or(id).into(),
                container_id: value["containerId"].as_str().map(str::to_owned),
                dependency_node_ids: Vec::new(),
                downstream_node_ids: Vec::new(),
                bindings,
                outputs,
                config: value["config"].clone(),
                plugin_runtime: None,
                llm_runtime: None,
                code_runtime: None,
            },
        );
    }
    for edge in document["graph"]["edges"].as_array().unwrap() {
        let source = edge["source"].as_str().unwrap();
        let target = edge["target"].as_str().unwrap();
        if nodes.contains_key(source) && nodes.contains_key(target) {
            nodes
                .get_mut(target)
                .unwrap()
                .dependency_node_ids
                .push(source.into());
            nodes
                .get_mut(source)
                .unwrap()
                .downstream_node_ids
                .push(target.into());
        }
    }
    for node in nodes.values() {
        let outputs = source_outputs(node);
        report.node_outputs.push(FlowNodeOutputs {
            node_id: node.node_id.clone(),
            outputs: outputs
                .iter()
                .map(|output| FlowResolvedOutput {
                    key: output.key.clone(),
                    title: output.title.clone(),
                    value_type: output.value_type.clone(),
                    selector: output.selector.clone(),
                    json_schema: output.json_schema.clone(),
                })
                .collect(),
        });
        if target_node_id == Some(node.node_id.as_str()) {
            let mut fields = BTreeSet::from([String::new()]);
            for key in document["graph"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|value| value["id"].as_str() == Some(&node.node_id))
                .and_then(|value| value["bindings"].as_object())
                .into_iter()
                .flat_map(|bindings| bindings.keys())
            {
                fields.insert(format!("/bindings/{}", diagnostics::pointer_segment(key)));
            }
            if matches!(node.node_type.as_str(), "loop" | "iteration") {
                fields.extend(
                    [
                        "/bindings/output",
                        "/bindings/variable_updates",
                        "/bindings/break_condition",
                    ]
                    .map(str::to_owned),
                );
            }
            for field_path in fields {
                report.variable_catalogs.push(FlowVariableCatalog {
                    node_id: node.node_id.clone(),
                    variables: variables::visible_variables(
                        document,
                        &nodes,
                        node,
                        &field_path,
                        context,
                    ),
                    field_path,
                });
            }
        }
        for (key, binding) in &node.bindings {
            let path = format!("/bindings/{}", diagnostics::pointer_segment(key));
            for selector in &binding.selector_paths {
                let Some(source) = selector.first().and_then(|id| nodes.get(id)) else {
                    continue;
                };
                if super::containers::selector_is_visible(&nodes, node, source, selector, &path)
                    && selector_contract(source, &selector[1..]) == PathContract::Dynamic
                {
                    report.diagnostics.push(FlowAnalysisDiagnostic {
                        node_id: Some(node.node_id.clone()),
                        code: "dynamic_selector_path".into(),
                        level: "warning".into(),
                        field_path: path.clone(),
                        message: format!(
                            "{} traverses dynamic JSON; its value will be checked during execution",
                            selector.join(".")
                        ),
                        selector: Some(selector.clone()),
                    });
                }
            }
        }
    }
    if target_node_id.is_some_and(|node_id| !nodes.contains_key(node_id)) {
        report.diagnostics.push(FlowAnalysisDiagnostic {
            node_id: None,
            code: "unknown_analysis_target".into(),
            level: "error".into(),
            field_path: String::new(),
            message: "The requested analysis node does not exist in this draft".into(),
            selector: None,
        });
    }
    // Keep the full compiler authoritative for graph/container structure, binding
    // families and runtime configuration. An error does not discard editor catalogs.
    let family = application_type.unwrap_or_else(|| {
        if nodes
            .values()
            .any(|node| node.node_type == "workflow_start")
        {
            domain::ApplicationType::Workflow
        } else {
            domain::ApplicationType::AgentFlow
        }
    });
    let compiled = match family {
        domain::ApplicationType::Workflow => {
            FlowCompiler::compile_workflow(uuid::Uuid::nil(), "analysis", document, context)
        }
        domain::ApplicationType::AgentFlow => {
            FlowCompiler::compile(uuid::Uuid::nil(), "analysis", document, context)
        }
        domain::ApplicationType::AgentLogs => Err(anyhow!(
            "log applications do not own orchestration documents"
        )),
    };
    match compiled {
        Ok(plan) => {
            for issue in &plan.compile_issues {
                let diagnostic = diagnostics::NodeDiagnostic::from(issue);
                report.diagnostics.push(FlowAnalysisDiagnostic {
                    node_id: diagnostic.node_id,
                    code: diagnostic.code,
                    level: "error".into(),
                    field_path: diagnostic.field_path,
                    message: diagnostic.message,
                    selector: None,
                });
            }
        }
        Err(error) => append_error(&mut report, error),
    }
    let mut seen = BTreeSet::new();
    report.diagnostics.retain(|diagnostic| {
        seen.insert((
            diagnostic.node_id.clone(),
            diagnostic.field_path.clone(),
            diagnostic.code.clone(),
            diagnostic.message.clone(),
        ))
    });
    report
}

pub fn ensure_flow_analysis_valid(analysis: &FlowDocumentAnalysis) -> Result<()> {
    let diagnostics = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.level == "error")
        .map(|diagnostic| diagnostics::NodeDiagnostic {
            node_id: diagnostic.node_id.clone(),
            code: diagnostic.code.clone(),
            field_path: diagnostic.field_path.clone(),
            message: diagnostic.message.clone(),
            expected: None,
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(FlowValidationError { diagnostics }.into())
    }
}

fn append_error(report: &mut FlowDocumentAnalysis, error: anyhow::Error) {
    if let Some(error) = error.downcast_ref::<FlowValidationError>() {
        report
            .diagnostics
            .extend(
                error
                    .diagnostics
                    .iter()
                    .map(|diagnostic| FlowAnalysisDiagnostic {
                        node_id: diagnostic.node_id.clone(),
                        code: diagnostic.code.clone(),
                        level: "error".into(),
                        field_path: diagnostic.field_path.clone(),
                        message: diagnostic.message.clone(),
                        selector: None,
                    }),
            );
    } else {
        report.diagnostics.push(FlowAnalysisDiagnostic {
            node_id: None,
            code: "invalid_flow_document".into(),
            level: "error".into(),
            field_path: String::new(),
            message: format!("{error:#}"),
            selector: None,
        });
    }
}
