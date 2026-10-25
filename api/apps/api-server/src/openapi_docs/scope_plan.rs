use std::borrow::Cow;

use super::*;

/// A source subtree or a shallow object assembled in the original insertion order.
/// Expanding only ancestors of later writes preserves overlapping-pointer behavior
/// without copying component bodies during startup validation.
#[derive(Debug, Clone)]
enum ComponentPlan {
    Source(String),
    Object(Vec<(String, ComponentPlan)>),
}

impl ComponentPlan {
    fn insert(&mut self, canonical: &Value, pointer: &str, source: String) -> Result<()> {
        if pointer.is_empty() {
            *self = Self::Source(source);
            return Ok(());
        }
        let tokens = pointer
            .trim_start_matches('/')
            .split('/')
            .map(unescape_json_pointer_token)
            .collect::<Vec<_>>();
        self.insert_tokens(canonical, &tokens, source)
    }

    fn insert_tokens(
        &mut self,
        canonical: &Value,
        tokens: &[String],
        source: String,
    ) -> Result<()> {
        if let Self::Source(pointer) = self {
            let object = canonical
                .pointer(pointer)
                .expect("validated component source")
                .as_object()
                .context("JSON pointer target must be an object")?;
            *self = Self::Object(
                object
                    .keys()
                    .map(|key| {
                        let child = format!("{pointer}/{}", escape_token(key));
                        (key.clone(), Self::Source(child))
                    })
                    .collect(),
            );
        }
        let Self::Object(fields) = self else {
            unreachable!()
        };
        let token = &tokens[0];
        let position = fields.iter().position(|(key, _)| key == token);
        let index = position.unwrap_or_else(|| {
            fields.push((token.clone(), Self::Object(Vec::new())));
            fields.len() - 1
        });
        if tokens.len() == 1 {
            fields[index].1 = Self::Source(source);
            Ok(())
        } else {
            fields[index]
                .1
                .insert_tokens(canonical, &tokens[1..], source)
        }
    }

    fn materialize(&self, canonical: &Value) -> Value {
        match self {
            Self::Source(pointer) => canonical
                .pointer(pointer)
                .expect("validated component source")
                .clone(),
            Self::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(key, node)| (key.clone(), node.materialize(canonical)))
                    .collect(),
            ),
        }
    }

    fn pointer<'a>(&'a self, canonical: &'a Value, pointer: &str) -> Option<Cow<'a, Value>> {
        match self {
            Self::Source(source) => canonical
                .pointer(&format!("{source}{pointer}"))
                .map(Cow::Borrowed),
            Self::Object(fields) => {
                if pointer.is_empty() {
                    return Some(Cow::Owned(self.materialize(canonical)));
                }
                let (token, tail) = split_pointer(pointer)?;
                fields
                    .iter()
                    .find(|(key, _)| *key == token)?
                    .1
                    .pointer(canonical, tail)
            }
        }
    }
}

fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn split_pointer(pointer: &str) -> Option<(String, &str)> {
    let rest = pointer.strip_prefix('/')?;
    let end = rest.find('/').unwrap_or(rest.len());
    Some((unescape_json_pointer_token(&rest[..end]), &rest[end..]))
}

#[derive(Debug, Clone)]
pub(super) struct ScopePlan {
    paths: Vec<(String, Vec<String>)>,
    components: ComponentPlan,
    security: Option<Value>,
    tags: Vec<usize>,
}

impl ScopePlan {
    pub(super) fn compile(
        canonical: &Value,
        scoped_operations: &[(String, String)],
    ) -> Result<Self> {
        let canonical_map = canonical
            .as_object()
            .context("canonical OpenAPI document must be a JSON object")?;
        let canonical_paths = canonical_map
            .get("paths")
            .and_then(Value::as_object)
            .context("canonical OpenAPI document must contain paths")?;
        let scoped_security =
            collect_scoped_security_requirements(canonical_map, scoped_operations)?;
        let scheme_names = collect_security_scheme_names(&scoped_security);
        let mut paths: Vec<(String, Vec<String>)> = Vec::new();
        let mut operation_tags = BTreeSet::new();
        for (path, method) in scoped_operations {
            let path_item = canonical_paths
                .get(path)
                .and_then(Value::as_object)
                .with_context(|| format!("path `{path}` not found in canonical document"))?;
            let index = paths
                .iter()
                .position(|(key, _)| key == path)
                .unwrap_or_else(|| {
                    paths.push((path.clone(), Vec::new()));
                    paths.len() - 1
                });
            for key in path_item.keys() {
                if (key == method || !HTTP_METHODS.contains(&key.as_str()))
                    && !paths[index].1.contains(key)
                {
                    paths[index].1.push(key.clone());
                }
            }
            let operation = path_item.get(method).with_context(|| {
                format!("operation `{method} {path}` not found in canonical document")
            })?;
            operation_tags.extend(extract_tags(operation));
        }
        let mut refs = BTreeSet::new();
        for (path, fields) in &paths {
            if let Some(reference) = canonical_paths[path].get("$ref").and_then(Value::as_str) {
                refs.insert(reference.to_string());
            }
            for key in fields {
                collect_refs(&canonical_paths[path][key], &mut refs);
            }
        }
        let mut pending_refs = refs.into_iter().collect::<Vec<_>>();
        let mut visited_refs = HashSet::new();
        let mut components = ComponentPlan::Object(Vec::new());
        while let Some(reference) = pending_refs.pop() {
            if !visited_refs.insert(reference.clone()) {
                continue;
            }
            let pointer = reference
                .strip_prefix('#')
                .ok_or_else(|| anyhow!("unsupported external $ref `{reference}`"))?;
            let referenced_value = canonical
                .pointer(pointer)
                .with_context(|| format!("missing referenced node `{reference}`"))?;
            if let Some(component_pointer) = pointer.strip_prefix("/components") {
                components.insert(canonical, component_pointer, pointer.to_string())?;
            } else {
                bail!("unsupported non-components $ref `{reference}`");
            }
            let mut nested_refs = BTreeSet::new();
            collect_refs(referenced_value, &mut nested_refs);
            pending_refs.extend(nested_refs);
        }
        for scheme_name in scheme_names {
            let source = format!("/components/securitySchemes/{}", escape_token(&scheme_name));
            canonical
                .pointer(&source)
                .with_context(|| format!("missing security scheme `{scheme_name}`"))?;
            // Preserve the original asymmetric lookup/insertion for slash/tilde names.
            components.insert(
                canonical,
                &format!("/securitySchemes/{scheme_name}"),
                source,
            )?;
        }
        let tags = canonical_map
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| {
                tags.iter()
                    .enumerate()
                    .filter(|(_, tag)| {
                        tag.get("name")
                            .and_then(Value::as_str)
                            .is_some_and(|name| operation_tags.contains(name))
                    })
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            paths,
            components,
            security: (!scoped_security.is_empty()).then_some(Value::Array(scoped_security)),
            tags,
        })
    }

    pub(super) fn materialize(&self, canonical: &Value) -> Value {
        let mut spec = Map::new();
        spec.insert("openapi".into(), canonical["openapi"].clone());
        spec.insert("info".into(), canonical["info"].clone());
        if let Some(servers) = canonical.get("servers") {
            spec.insert("servers".into(), servers.clone());
        }
        if let Some(security) = &self.security {
            spec.insert("security".into(), security.clone());
        }
        spec.insert(
            "paths".into(),
            Value::Object(
                self.paths
                    .iter()
                    .map(|(path, fields)| {
                        let item = fields
                            .iter()
                            .map(|key| (key.clone(), canonical["paths"][path][key].clone()))
                            .collect();
                        (path.clone(), Value::Object(item))
                    })
                    .collect(),
            ),
        );
        spec.insert("components".into(), self.components.materialize(canonical));
        if !self.tags.is_empty() {
            spec.insert(
                "tags".into(),
                Value::Array(
                    self.tags
                        .iter()
                        .map(|index| canonical["tags"][*index].clone())
                        .collect(),
                ),
            );
        }
        Value::Object(spec)
    }
}

/// Narrow access for catalog consumers: borrow path metadata and resolve against
/// the validated component projection, including legacy overlapping writes.
/// It never retains or constructs a complete operation document.
pub(crate) struct OperationSpecView<'a> {
    pub(super) canonical: &'a Value,
    pub(super) plan: &'a ScopePlan,
}

impl OperationSpecView<'_> {
    pub(crate) fn source(&self) -> &Value {
        self.canonical
    }
    pub(crate) fn security(&self) -> Option<&Value> {
        self.plan.security.as_ref()
    }
    pub(crate) fn resolve_pointer(&self, pointer: &str) -> Option<Cow<'_, Value>> {
        let tail = pointer.strip_prefix("/components")?;
        if !tail.is_empty() && !tail.starts_with('/') {
            return None;
        }
        self.plan.components.pointer(self.canonical, tail)
    }
}
