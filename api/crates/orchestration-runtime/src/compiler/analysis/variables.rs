use super::*;

pub(super) fn visible_variables(
    document: &Value,
    nodes: &BTreeMap<String, CompiledNode>,
    target: &CompiledNode,
    field_path: &str,
    context: &FlowCompileContext,
) -> Vec<FlowVariableOption> {
    let mut options = Vec::new();
    for source in nodes.values() {
        let mut outputs = source_outputs(source);
        for selector in [
            vec![source.node_id.clone(), "item".into()],
            vec![source.node_id.clone(), "index".into()],
            vec![source.node_id.clone(), "variables".into()],
        ] {
            if let Some(output) =
                super::super::containers::container_local_output(source, &selector)
            {
                outputs.push(output);
            }
        }
        if source.node_type == "loop" {
            for variable in source.config["loop_variables"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(key) = variable["key"].as_str() {
                    if let Some(output) = super::super::containers::container_local_output(
                        source,
                        &[source.node_id.clone(), "variables".into(), key.into()],
                    ) {
                        outputs.push(output);
                    }
                }
            }
        }
        for output in outputs {
            let mut selector = vec![source.node_id.clone()];
            selector.extend(output.selector.iter().cloned());
            let local = source.node_id == target.node_id
                || super::super::containers::is_enclosing_container(nodes, target, &source.node_id);
            if !local
                && super::super::containers::container_local_output(source, &selector).is_some()
                && selector_contract(source, &selector[1..]) == PathContract::Missing
            {
                continue;
            }
            if super::super::containers::selector_is_visible(
                nodes, target, source, &selector, field_path,
            ) {
                let schema = output_schema(&output);
                push_option(
                    &mut options,
                    &source.node_id,
                    &source.alias,
                    &output.key,
                    &output.title,
                    &output.value_type,
                    output.json_schema.clone(),
                    selector.clone(),
                );
                expand_properties(
                    &mut options,
                    &source.node_id,
                    &source.alias,
                    &schema,
                    &schema,
                    &selector,
                    0,
                    &mut BTreeSet::new(),
                );
            }
        }
    }
    // Preserve a schema-valid dynamic or patterned path already bound in this
    // draft. This is metadata derived from the contract, never a guessed value.
    for (key, binding) in &target.bindings {
        let binding_path = format!("/bindings/{}", diagnostics::pointer_segment(key));
        if !field_path.is_empty() && field_path != binding_path {
            continue;
        }
        for selector in &binding.selector_paths {
            let Some(source) = selector.first().and_then(|id| nodes.get(id)) else {
                continue;
            };
            if !super::super::containers::selector_is_visible(
                nodes, target, source, selector, field_path,
            ) {
                continue;
            }
            let schema = match selector_contract(source, &selector[1..]) {
                PathContract::Missing => continue,
                PathContract::Dynamic => None,
                PathContract::Known(schema) => Some(schema),
            };
            let label = selector[1..].join(".");
            let value_type = schema
                .as_ref()
                .map(schema_value_type)
                .unwrap_or_else(|| "json".into());
            push_option(
                &mut options,
                &source.node_id,
                &source.alias,
                &label,
                &label,
                &value_type,
                schema,
                selector.clone(),
            );
        }
    }
    for declaration in &context.run_level_variables {
        if let Some((node_id, key)) = declaration.selector.split_first() {
            let key = key.join(".");
            push_option(
                &mut options,
                node_id,
                node_id,
                &key,
                &key,
                if declaration
                    .selector
                    .iter()
                    .map(String::as_str)
                    .eq(crate::compiled_plan::SYSTEM_PROTOCOL_CONTEXT_SELECTOR)
                {
                    extension_contracts::provider_contract::PROTOCOL_CONTEXT_VALUE_TYPE
                } else {
                    &declaration.value_type
                },
                None,
                declaration.selector.clone(),
            );
        }
    }
    if let Some(variables) = document
        .pointer("/variables/conversation")
        .and_then(Value::as_array)
    {
        for variable in variables {
            if let Some(name) = variable["name"].as_str() {
                push_option(
                    &mut options,
                    "conversation",
                    "conversation",
                    name,
                    name,
                    variable["valueType"].as_str().unwrap_or("json"),
                    None,
                    vec!["conversation".into(), name.into()],
                );
            }
        }
    }
    // Internal tool argument scope is declared by the upstream LLM connector.
    for edge in document["graph"]["edges"].as_array().into_iter().flatten() {
        let Some(source) = edge["source"].as_str().and_then(|id| nodes.get(id)) else {
            continue;
        };
        if source.node_type != "llm"
            || !super::super::topology::visible_internal_llm_tools_enabled(source)
        {
            continue;
        }
        let Some(edge_target) = edge["target"].as_str() else {
            continue;
        };
        if edge_target != target.node_id
            && !super::super::topology::node_depends_on(nodes, &target.node_id, edge_target)
        {
            continue;
        }
        let Some(handle) = edge["sourceHandle"].as_str() else {
            continue;
        };
        for tool in source.config["visible_internal_llm_tools"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let connector = tool["connector_id"]
                .as_str()
                .or_else(|| tool["tool_name"].as_str())
                .unwrap_or_default();
            if connector.is_empty()
                || super::super::topology::visible_internal_llm_tool_connector_id_from_source_handle(
                    Some(handle),
                ) != Some(connector)
            {
                continue;
            }
            for (key, property) in tool
                .pointer("/input_schema/properties")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                if property == &Value::Bool(false) {
                    continue;
                }
                push_option(
                    &mut options,
                    "visible_internal_llm_tool",
                    "tool",
                    key,
                    key,
                    &schema_value_type(property),
                    property.is_object().then(|| property.clone()),
                    vec![
                        "visible_internal_llm_tool".into(),
                        "arguments".into(),
                        key.into(),
                    ],
                );
            }
        }
    }
    let mut seen = BTreeSet::new();
    options.retain(|option| seen.insert(option.selector.clone()));
    options
}

fn push_option(
    options: &mut Vec<FlowVariableOption>,
    node_id: &str,
    node_label: &str,
    key: &str,
    title: &str,
    value_type: &str,
    schema: Option<Value>,
    selector: Vec<String>,
) {
    options.push(FlowVariableOption {
        node_id: node_id.into(),
        node_label: node_label.into(),
        output_key: key.into(),
        output_label: title.into(),
        value_type: value_type.into(),
        json_schema: schema,
        selector,
    });
}

fn expand_properties(
    options: &mut Vec<FlowVariableOption>,
    node_id: &str,
    node_label: &str,
    root: &Value,
    schema: &Value,
    parent: &[String],
    depth: usize,
    references: &mut BTreeSet<String>,
) {
    if depth > 32 {
        return;
    }
    let reference = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix('#'));
    if let Some(reference) = reference {
        if !references.insert(reference.into()) {
            return;
        }
    }
    for key in
        super::super::output_contract::schema_property_names(root, schema, &mut BTreeSet::new())
    {
        let property =
            match super::super::output_contract::schema_path(root, schema, &[key.clone()], 0) {
                PathContract::Missing => continue,
                PathContract::Dynamic => Value::Bool(true),
                PathContract::Known(property) => property,
            };
        let mut selector = parent.to_vec();
        selector.push(key.clone());
        push_option(
            options,
            node_id,
            node_label,
            &key,
            &key,
            &schema_value_type(&property),
            property.is_object().then(|| property.clone()),
            selector.clone(),
        );
        expand_properties(
            options,
            node_id,
            node_label,
            root,
            &property,
            &selector,
            depth + 1,
            references,
        );
    }
    if let Some(reference) = reference {
        references.remove(reference);
    }
}
