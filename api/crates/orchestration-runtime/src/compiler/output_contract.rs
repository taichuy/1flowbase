use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum PathContract {
    Known(Value),
    Dynamic,
    Missing,
}

/// Explicit selectors are authoritative. Node defaults apply only when absent.
pub(super) fn resolve_outputs(node_type: &str, values: &[Value]) -> Result<Vec<CompiledOutput>> {
    let mut keys = BTreeSet::new();
    let outputs = values
        .iter()
        .map(|value| {
            let string = |key: &str| {
                value
                    .get(key)
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("output {key} must be a string"))
            };
            let key = string("key")?.to_owned();
            if key.trim().is_empty() || !keys.insert(key.clone()) {
                bail!("output keys must be unique and nonempty");
            }
            let selector = match value.get("selector") {
                None => {
                    if node_type == "code" {
                        vec!["result".into(), key.clone()]
                    } else {
                        vec![key.clone()]
                    }
                }
                Some(value) => {
                    let array = value.as_array().filter(|v| !v.is_empty()).ok_or_else(|| {
                        anyhow!("explicit output selector must be a nonempty array")
                    })?;
                    array
                        .iter()
                        .map(|segment| {
                            segment
                                .as_str()
                                .filter(|s| !s.trim().is_empty())
                                .map(str::to_owned)
                                .ok_or_else(|| {
                                    anyhow!("output selector segments must be nonempty strings")
                                })
                        })
                        .collect::<Result<Vec<_>>>()?
                }
            };
            let json_schema = value.get("jsonSchema").cloned();
            if json_schema
                .as_ref()
                .is_some_and(|schema| !schema.is_object())
            {
                bail!("output jsonSchema must be an object");
            }
            if let Some(schema) = &json_schema {
                jsonschema::meta::options()
                    .validate(schema)
                    .map_err(|error| anyhow!("invalid output jsonSchema: {error}"))?;
            }
            Ok(CompiledOutput {
                key,
                title: string("title")?.to_owned(),
                value_type: string("valueType")?.to_owned(),
                selector,
                json_schema,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    PublicOutputContract::from_compiled_outputs(&outputs)?;
    Ok(outputs)
}

pub(super) fn output_schema(output: &CompiledOutput) -> Value {
    output
        .json_schema
        .clone()
        .unwrap_or_else(|| match output.value_type.as_str() {
            "string" | "number" | "integer" | "boolean" | "object" | "array" | "null" => {
                serde_json::json!({"type":output.value_type})
            }
            "array[object]" => serde_json::json!({"type":"array","items":{"type":"object"}}),
            _ => Value::Bool(true),
        })
}

pub(super) fn source_outputs(node: &CompiledNode) -> Vec<CompiledOutput> {
    if !matches!(node.node_type.as_str(), "start" | "workflow_start") {
        let mut outputs = node.outputs.clone();
        // Ordinary node variable pools project outputs under their public key.
        // Code alone preserves the result envelope and its explicit paths.
        if node.node_type != "code" {
            for output in &mut outputs {
                output.selector = vec![output.key.clone()];
            }
        }
        return outputs;
    }
    let mut result = Vec::new();
    if let Some(fields) = node.config.get("input_fields").and_then(Value::as_array) {
        for field in fields {
            let Some(key) = field
                .get("key")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            result.push(CompiledOutput {
                key: key.into(),
                title: field
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or(key)
                    .into(),
                value_type: field
                    .get("valueType")
                    .and_then(Value::as_str)
                    .unwrap_or("json")
                    .into(),
                selector: vec![key.into()],
                json_schema: field.get("jsonSchema").cloned(),
            });
        }
    }
    if node.node_type == "start" {
        for (key, kind) in [
            ("query", "string"),
            ("inputs", "object"),
            ("history", "array"),
            ("attachments", "array"),
            ("model", "string"),
            ("system", "string"),
            ("reasoning_effort", "string"),
            ("max_output_tokens", "number"),
            ("file", "json"),
            ("files", "array"),
            ("tools", "array"),
            ("tool_choice", "json"),
            ("protocol_context", "protocol_context"),
        ] {
            if !result.iter().any(|output| output.key == key) {
                result.push(CompiledOutput {
                    key: key.into(),
                    title: key.into(),
                    value_type: kind.into(),
                    selector: vec![key.into()],
                    json_schema: if key == "history" {
                        Some(history_messages_schema())
                    } else {
                        None
                    },
                });
            }
        }
    }
    result
}

pub(super) fn selector_contract(source: &CompiledNode, relative: &[String]) -> PathContract {
    let outputs = source_outputs(source);
    let Some(output) = outputs
        .iter()
        .filter(|output| relative.starts_with(&output.selector))
        .max_by_key(|output| output.selector.len())
    else {
        return PathContract::Missing;
    };
    let schema = output_schema(output);
    schema_path(&schema, &schema, &relative[output.selector.len()..], 0)
}

/// Project the constraints on an object-key path. Sibling keywords are
/// conjunctive; alternatives are conservative unions. Unknown JSON is dynamic.
pub(super) fn schema_path(
    root: &Value,
    schema: &Value,
    path: &[String],
    depth: usize,
) -> PathContract {
    if depth > 64 {
        return PathContract::Dynamic;
    }
    if schema == &Value::Bool(false) {
        return PathContract::Missing;
    }
    if path.is_empty() {
        return PathContract::Known(schema.clone());
    }
    let mut constraints = vec![object_path(root, schema, path, depth)];
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        constraints.push(
            reference
                .strip_prefix('#')
                .and_then(|pointer| root.pointer(pointer))
                .map(|target| schema_path(root, target, path, depth + 1))
                .unwrap_or(PathContract::Dynamic),
        );
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = schema.get(keyword).and_then(Value::as_array) {
            let results = branches
                .iter()
                .map(|branch| schema_path(root, branch, path, depth + 1))
                .collect();
            constraints.push(if keyword == "allOf" {
                conjunction(results)
            } else {
                alternatives(results)
            });
        }
    }
    conjunction(constraints)
}

fn object_path(root: &Value, schema: &Value, path: &[String], depth: usize) -> PathContract {
    if let Some(kind) = schema.get("type") {
        let allows_object = kind.as_str() == Some("object")
            || kind
                .as_array()
                .is_some_and(|types| types.iter().any(|kind| kind.as_str() == Some("object")));
        // The runtime traverses object keys. A union can still admit an object,
        // even if other members are scalars or arrays.
        if !allows_object {
            return PathContract::Missing;
        }
    }
    let mut constraints = Vec::new();
    if let Some(property) = schema
        .get("properties")
        .and_then(|properties| properties.get(&path[0]))
    {
        constraints.push(schema_path(root, property, &path[1..], depth + 1));
    }
    for (pattern, property) in schema
        .get("patternProperties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        if regex::Regex::new(pattern).is_ok_and(|pattern| pattern.is_match(&path[0])) {
            constraints.push(schema_path(root, property, &path[1..], depth + 1));
        }
    }
    if !constraints.is_empty() {
        return conjunction(constraints);
    }
    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => PathContract::Missing,
        Some(property) if property.is_object() => {
            schema_path(root, property, &path[1..], depth + 1)
        }
        _ => PathContract::Dynamic,
    }
}

fn conjunction(results: Vec<PathContract>) -> PathContract {
    let mut schemas = Vec::new();
    for result in results {
        match result {
            PathContract::Missing => return PathContract::Missing,
            PathContract::Known(schema) if schema != Value::Bool(true) => schemas.push(schema),
            _ => {}
        }
    }
    match schemas.len() {
        0 => PathContract::Dynamic,
        1 => PathContract::Known(schemas.remove(0)),
        _ => PathContract::Known(serde_json::json!({"allOf":schemas})),
    }
}

fn alternatives(results: Vec<PathContract>) -> PathContract {
    let mut schemas = Vec::new();
    for result in results {
        match result {
            PathContract::Dynamic => return PathContract::Dynamic,
            PathContract::Known(schema) => schemas.push(schema),
            PathContract::Missing => {}
        }
    }
    match schemas.len() {
        0 => PathContract::Missing,
        1 => PathContract::Known(schemas.remove(0)),
        _ => PathContract::Known(serde_json::json!({"anyOf":schemas})),
    }
}

/// Known property names for editor discovery; final acceptance of each path is
/// always decided by schema_path, including sibling constraints.
pub(super) fn schema_property_names(
    root: &Value,
    schema: &Value,
    seen: &mut BTreeSet<String>,
) -> BTreeSet<String> {
    let mut names = schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|properties| properties.keys().cloned())
        .collect::<BTreeSet<_>>();
    if let Some(reference) = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix('#'))
    {
        if seen.insert(reference.into()) {
            if let Some(target) = root.pointer(reference) {
                names.extend(schema_property_names(root, target, seen));
            }
            seen.remove(reference);
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        for branch in schema
            .get(keyword)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            names.extend(schema_property_names(root, branch, seen));
        }
    }
    names
}

pub(super) fn schema_value_type(schema: &Value) -> String {
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        return if kind == "integer" {
            "number".into()
        } else {
            kind.into()
        };
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        let kinds = types
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| *kind != "null")
            .collect::<BTreeSet<_>>();
        if kinds.len() == 1 {
            let kind = *kinds.first().unwrap();
            return if kind == "integer" {
                "number".into()
            } else {
                kind.into()
            };
        }
    }
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(branches) = schema.get(keyword).and_then(Value::as_array) {
            let mut kinds = branches
                .iter()
                .map(schema_value_type)
                .collect::<BTreeSet<_>>();
            if keyword != "allOf" && kinds.contains("json") {
                return "json".into();
            }
            kinds.remove("json");
            if kinds.len() == 1 {
                return kinds.into_iter().next().unwrap();
            }
        }
    }
    "json".into()
}
