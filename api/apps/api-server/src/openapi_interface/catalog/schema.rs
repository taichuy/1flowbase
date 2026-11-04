use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::CatalogSpec;

// OpenAPI parameter/requestBody/response references are envelopes, not JSON
// Schema roots. Resolve only the envelope; close schemas after extracting and
// assembling their final roots so wrapper/descriptor projection cannot strand refs.
pub(super) fn resolve_object(spec: &CatalogSpec<'_>, value: &Value) -> Value {
    let mut value = value.clone();
    let mut visited = BTreeSet::new();
    while let Some(reference) = value.get("$ref").and_then(Value::as_str) {
        if !visited.insert(reference.to_owned()) {
            break;
        }
        let Some(target) = reference
            .strip_prefix('#')
            .and_then(|pointer| spec.pointer(pointer))
        else {
            break;
        };
        let mut target = target.into_owned();
        if let (Some(siblings), Some(target)) = (value.as_object(), target.as_object_mut()) {
            for (key, sibling) in siblings {
                if key != "$ref" {
                    target.insert(key.clone(), sibling.clone());
                }
            }
        }
        value = target;
    }
    value
}

pub(super) fn resolve_schema(spec: &CatalogSpec<'_>, value: &Value) -> Value {
    let mut graph = SchemaGraph {
        spec,
        inlined: BTreeSet::new(),
        references: BTreeMap::new(),
        pending: Vec::new(),
    };
    let mut root = graph.project(value, 0, true);
    let mut definitions = Map::new();
    let mut index = 0;
    while index < graph.pending.len() {
        let target = graph.pending[index].clone();
        // Definitions retain edges instead of expanding other definitions. A
        // target is enqueued once, including cycles and arbitrarily long chains.
        definitions.insert(index.to_string(), graph.project(&target, 0, false));
        index += 1;
    }
    let Some(root_object) = root.as_object_mut() else {
        return root;
    };
    let mut namespace = "__catalog_refs".to_string();
    while root_object
        .get("$defs")
        .and_then(Value::as_object)
        .is_some_and(|defs| defs.contains_key(&namespace))
    {
        namespace.push('_');
    }
    let prefix = format!("#/$defs/{namespace}/$defs/");
    rewrite_references(&mut root, &graph.references, &prefix);
    if !definitions.is_empty() {
        let mut definitions = Value::Object(definitions);
        rewrite_references(&mut definitions, &graph.references, &prefix);
        let root = root
            .as_object_mut()
            .expect("schema root is still an object");
        // Do not repair an invalid source $defs into a permissive schema.
        if let Some(defs) = root
            .entry("$defs")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
        {
            defs.insert(namespace, serde_json::json!({"$defs": definitions}));
        }
    }
    root
}

struct SchemaGraph<'a, 's> {
    spec: &'a CatalogSpec<'s>,
    inlined: BTreeSet<String>,
    references: BTreeMap<String, usize>,
    pending: Vec<Value>,
}

impl SchemaGraph<'_, '_> {
    fn project(&mut self, value: &Value, depth: usize, inline: bool) -> Value {
        match value {
            Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                    let spec = self.spec;
                    if let Some(target) = reference
                        .strip_prefix('#')
                        .and_then(|pointer| spec.pointer(pointer))
                    {
                        // Retain ordinary inline contracts where possible, but
                        // expand targets containing edges at most once across
                        // this root. Repeated terminal schemas stay inline. This
                        // bounds shared DAGs as well as cycles; depth limits only
                        // inline work, never the accepted instance or ref graph.
                        if inline
                            && depth < 16
                            && (!contains_reference(&target)
                                || self.inlined.insert(reference.to_owned()))
                        {
                            let mut target = target.into_owned();
                            if let Some(target) = target.as_object_mut() {
                                for (key, sibling) in map {
                                    if key != "$ref" {
                                        target.insert(key.clone(), sibling.clone());
                                    }
                                }
                            }
                            return self.project(&target, depth + 1, true);
                        }
                        if !self.references.contains_key(reference) {
                            self.references
                                .insert(reference.to_owned(), self.pending.len());
                            self.pending.push(target.into_owned());
                        }
                    }
                }
                Value::Object(
                    map.iter()
                        .map(|(key, nested)| (key.clone(), self.project(nested, depth, inline)))
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.project(item, depth, inline))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
}

fn contains_reference(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.contains_key("$ref") || map.values().any(contains_reference),
        Value::Array(items) => items.iter().any(contains_reference),
        _ => false,
    }
}

fn rewrite_references(value: &mut Value, references: &BTreeMap<String, usize>, prefix: &str) {
    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get_mut("$ref") {
                if let Some(source) = reference.as_str() {
                    // Unknown (including unsupported external) refs remain
                    // unresolvable, even if moving the root would accidentally
                    // make their old pointer exist. Numeric keys are the only
                    // emitted graph definitions, so this sentinel never resolves.
                    *reference = Value::String(match references.get(source) {
                        Some(index) => format!("{prefix}{index}"),
                        None => format!("{prefix}unresolved"),
                    });
                }
            }
            for nested in map.values_mut() {
                rewrite_references(nested, references, prefix);
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_references(item, references, prefix);
            }
        }
        _ => {}
    }
}
