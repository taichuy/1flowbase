use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, bail, Result};
use plugin_framework::provider_contract::NativePromptBlock;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::compiled_plan::{CompiledBinding, CompiledI18nTextRef, CompiledNode, CompiledPlan};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferencedI18nMessage {
    pub key: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingResolutionIssue {
    pub binding_key: String,
    pub selector: Option<Vec<String>>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnswerNodeInputResolution {
    pub resolved_inputs: Map<String, Value>,
    pub issues: Vec<BindingResolutionIssue>,
}

pub fn resolve_node_inputs(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Result<Map<String, Value>> {
    let mut resolved = Map::new();

    for (binding_key, binding) in &node.bindings {
        resolved.insert(
            binding_key.clone(),
            resolve_binding(binding, variable_pool).map_err(|error| {
                anyhow!(
                    "failed to resolve binding {binding_key} for {}: {error}",
                    node.node_id
                )
            })?,
        );
    }

    Ok(resolved)
}

pub fn resolve_answer_node_inputs(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> AnswerNodeInputResolution {
    let mut resolved_inputs = Map::new();
    let mut issues = Vec::new();

    for (binding_key, binding) in &node.bindings {
        let value = resolve_answer_binding(binding_key, binding, variable_pool, &mut issues);
        resolved_inputs.insert(binding_key.clone(), value);
    }

    AnswerNodeInputResolution {
        resolved_inputs,
        issues,
    }
}

pub fn render_templated_bindings(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
) -> Map<String, Value> {
    node.bindings
        .iter()
        .filter_map(|(binding_key, binding)| {
            matches!(binding.kind.as_str(), "templated_text" | "prompt_messages")
                .then(|| {
                    resolved_inputs
                        .get(binding_key)
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .map(|value| (binding_key.clone(), value))
        })
        .collect()
}

pub fn referenced_i18n_text_refs(plan: &CompiledPlan) -> Vec<CompiledI18nTextRef> {
    plan.nodes
        .values()
        .flat_map(|node| node.bindings.values())
        .filter(|binding| binding.kind == "i18n_text")
        .filter_map(|binding| binding.i18n_text_ref.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn project_referenced_i18n_messages(
    plan: &CompiledPlan,
    translations: &BTreeMap<CompiledI18nTextRef, String>,
) -> Vec<ReferencedI18nMessage> {
    referenced_i18n_text_refs(plan)
        .into_iter()
        .map(|reference| ReferencedI18nMessage {
            text: translations
                .get(&reference)
                .cloned()
                .unwrap_or_else(|| reference.key.clone()),
            key: reference.key,
        })
        .collect()
}

fn resolve_binding(binding: &CompiledBinding, variable_pool: &Map<String, Value>) -> Result<Value> {
    match binding.kind.as_str() {
        "i18n_text" => binding
            .i18n_text_ref
            .as_ref()
            .ok_or_else(|| anyhow!("i18n_text binding is missing its typed reference"))
            .and_then(|reference| {
                serde_json::to_value(reference)
                    .map_err(|error| anyhow!("failed to serialize i18n_text reference: {error}"))
            }),
        "selector" => {
            let selector = binding
                .selector_paths
                .first()
                .ok_or_else(|| anyhow!("selector binding is missing selector path"))?;
            lookup_selector_value(variable_pool, selector)
        }
        "selector_list" => binding
            .selector_paths
            .iter()
            .map(|selector| lookup_selector_value(variable_pool, selector))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        "named_bindings" => {
            let entries = binding
                .raw_value
                .as_array()
                .ok_or_else(|| anyhow!("named_bindings raw_value must be an array"))?;
            let mut object = Map::new();

            for entry in entries {
                let name = entry
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("named_bindings entry missing name"))?;
                let value = if let Some(value) = entry.get("value").and_then(Value::as_object) {
                    match value.get("kind").and_then(Value::as_str) {
                        Some("constant") => value.get("value").cloned().unwrap_or(Value::Null),
                        Some("selector") => {
                            let selector = value
                                .get("selector")
                                .and_then(Value::as_array)
                                .ok_or_else(|| anyhow!("named_bindings selector missing path"))?
                                .iter()
                                .map(|segment| {
                                    segment.as_str().map(str::to_string).ok_or_else(|| {
                                        anyhow!("named_bindings selector segment must be a string")
                                    })
                                })
                                .collect::<Result<Vec<_>>>()?;
                            lookup_selector_value(variable_pool, &selector)?
                        }
                        Some("templated_text") => {
                            let template =
                                value.get("value").and_then(Value::as_str).ok_or_else(|| {
                                    anyhow!("named_bindings templated_text value must be a string")
                                })?;
                            if entry.get("valueType").and_then(Value::as_str) == Some("number") {
                                render_numeric_template_expression(template, variable_pool)?
                            } else {
                                Value::String(render_template(template, variable_pool)?)
                            }
                        }
                        _ => {
                            return Err(anyhow!("named_bindings value has unknown kind"));
                        }
                    }
                } else if let Some(content) = entry
                    .get("content")
                    .and_then(|content| content.get("value"))
                    .and_then(Value::as_str)
                {
                    Value::String(render_template(content, variable_pool)?)
                } else {
                    let selector = entry
                        .get("selector")
                        .and_then(Value::as_array)
                        .ok_or_else(|| anyhow!("named_bindings entry missing selector"))?
                        .iter()
                        .map(|segment| {
                            segment.as_str().map(str::to_string).ok_or_else(|| {
                                anyhow!("named_bindings selector segment must be a string")
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    lookup_selector_value(variable_pool, &selector)?
                };

                object.insert(name.to_string(), value);
            }

            Ok(Value::Object(object))
        }
        "templated_text" => binding
            .raw_value
            .as_str()
            .map(|value| render_template(value, variable_pool).map(Value::String))
            .transpose()?
            .ok_or_else(|| anyhow!("templated_text raw_value must be a string")),
        "prompt_messages" => {
            let entries = binding
                .raw_value
                .as_array()
                .ok_or_else(|| anyhow!("prompt_messages raw_value must be an array"))?;
            let mut messages = Vec::with_capacity(entries.len());

            for entry in entries {
                let role = entry.get("role").and_then(Value::as_str).unwrap_or("user");
                let content = entry
                    .get("content")
                    .and_then(|content| content.get("value"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anyhow!("prompt_messages entry content.value must be a string")
                    })?;
                let mut message = Map::new();

                if let Some(id) = entry.get("id").and_then(Value::as_str) {
                    message.insert("id".to_string(), Value::String(id.to_string()));
                }

                message.insert("role".to_string(), Value::String(role.to_string()));
                let rendered_content = render_prompt_content_template(content, variable_pool)?;
                if role != "system" && rendered_content.is_array() {
                    bail!("typed prompt blocks are only supported for system messages");
                }
                message.insert("content".to_string(), rendered_content);
                messages.push(Value::Object(message));
            }

            Ok(Value::Array(messages))
        }
        "data_model_query" => resolve_data_model_query(binding, variable_pool),
        "condition_group" | "if_else_branches" | "state_write" => Ok(binding.raw_value.clone()),
        other => bail!("unsupported binding kind: {other}"),
    }
}

fn resolve_answer_binding(
    binding_key: &str,
    binding: &CompiledBinding,
    variable_pool: &Map<String, Value>,
    issues: &mut Vec<BindingResolutionIssue>,
) -> Value {
    match binding.kind.as_str() {
        "selector" => {
            let Some(selector) = binding.selector_paths.first() else {
                issues.push(BindingResolutionIssue {
                    binding_key: binding_key.to_string(),
                    selector: None,
                    message: "selector binding is missing selector path".to_string(),
                });
                return Value::Null;
            };
            lookup_selector_value(variable_pool, selector).unwrap_or_else(|error| {
                issues.push(BindingResolutionIssue {
                    binding_key: binding_key.to_string(),
                    selector: Some(selector.clone()),
                    message: error.to_string(),
                });
                Value::Null
            })
        }
        "selector_list" => Value::Array(
            binding
                .selector_paths
                .iter()
                .map(|selector| {
                    lookup_selector_value(variable_pool, selector).unwrap_or_else(|error| {
                        issues.push(BindingResolutionIssue {
                            binding_key: binding_key.to_string(),
                            selector: Some(selector.clone()),
                            message: error.to_string(),
                        });
                        Value::Null
                    })
                })
                .collect(),
        ),
        "templated_text" => binding
            .raw_value
            .as_str()
            .map(|value| {
                Value::String(render_template_with_issues(
                    binding_key,
                    value,
                    variable_pool,
                    issues,
                ))
            })
            .unwrap_or_else(|| {
                issues.push(BindingResolutionIssue {
                    binding_key: binding_key.to_string(),
                    selector: None,
                    message: "templated_text raw_value must be a string".to_string(),
                });
                Value::String(String::new())
            }),
        _ => resolve_binding(binding, variable_pool).unwrap_or_else(|error| {
            issues.push(BindingResolutionIssue {
                binding_key: binding_key.to_string(),
                selector: None,
                message: error.to_string(),
            });
            Value::Null
        }),
    }
}

fn resolve_data_model_query(
    binding: &CompiledBinding,
    variable_pool: &Map<String, Value>,
) -> Result<Value> {
    let object = binding
        .raw_value
        .as_object()
        .ok_or_else(|| anyhow!("data_model list query must be object"))?;
    let mut query = Map::new();

    query.insert(
        "filters".to_string(),
        Value::Array(resolve_data_model_query_filters(
            object.get("filters"),
            variable_pool,
        )?),
    );
    query.insert(
        "sorts".to_string(),
        Value::Array(resolve_data_model_query_sorts(object.get("sorts"))?),
    );
    query.insert(
        "expand_relations".to_string(),
        Value::Array(parse_data_model_query_string_array(
            object.get("expand_relations"),
            "expand_relations",
        )?),
    );
    query.insert(
        "page".to_string(),
        resolve_optional_query_value(object.get("page"), variable_pool, 1)?,
    );
    query.insert(
        "page_size".to_string(),
        resolve_optional_query_value(object.get("page_size"), variable_pool, 20)?,
    );

    Ok(Value::Object(query))
}

fn resolve_data_model_query_filters(
    value: Option<&Value>,
    variable_pool: &Map<String, Value>,
) -> Result<Vec<Value>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| anyhow!("data_model list filters must be array"))?;
    let mut filters = Vec::with_capacity(entries.len());

    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| anyhow!("data_model list filter must be object"))?;
        let operator = required_data_model_query_string(object, "operator", "filter")?;

        if !matches!(operator.as_str(), "eq" | "ne" | "gt" | "gte" | "lt" | "lte") {
            bail!("data_model list filter operator is unsupported: {operator}");
        }

        let mut filter = Map::new();
        filter.insert(
            "field_code".to_string(),
            Value::String(required_data_model_query_string(
                object,
                "field_code",
                "filter",
            )?),
        );
        filter.insert("operator".to_string(), Value::String(operator));
        filter.insert(
            "value".to_string(),
            resolve_query_value_input(
                object.get("value").unwrap_or(&Value::Null),
                variable_pool,
                "filter value",
            )?,
        );
        filters.push(Value::Object(filter));
    }

    Ok(filters)
}

fn resolve_data_model_query_sorts(value: Option<&Value>) -> Result<Vec<Value>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| anyhow!("data_model list sorts must be array"))?;
    let mut sorts = Vec::with_capacity(entries.len());

    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| anyhow!("data_model list sort must be object"))?;
        let direction = required_data_model_query_string(object, "direction", "sort")?;

        if !matches!(direction.as_str(), "asc" | "desc") {
            bail!("data_model list sort direction is unsupported: {direction}");
        }

        let mut sort = Map::new();
        sort.insert(
            "field_code".to_string(),
            Value::String(required_data_model_query_string(
                object,
                "field_code",
                "sort",
            )?),
        );
        sort.insert("direction".to_string(), Value::String(direction));
        sorts.push(Value::Object(sort));
    }

    Ok(sorts)
}

fn parse_data_model_query_string_array(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Vec<Value>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| anyhow!("data_model list {field} must be array"))?;

    entries
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(|value| Value::String(value.to_string()))
                .ok_or_else(|| anyhow!("data_model list {field} item must be string"))
        })
        .collect()
}

fn resolve_optional_query_value(
    value: Option<&Value>,
    variable_pool: &Map<String, Value>,
    default: i64,
) -> Result<Value> {
    match value {
        Some(value) => resolve_query_value_input(value, variable_pool, "query value"),
        None => Ok(Value::Number(default.into())),
    }
}

fn resolve_query_value_input(
    value: &Value,
    variable_pool: &Map<String, Value>,
    field: &'static str,
) -> Result<Value> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("data_model list {field} must be object"))?;

    match object.get("kind").and_then(Value::as_str) {
        Some("constant") => Ok(object.get("value").cloned().unwrap_or(Value::Null)),
        Some("selector") => {
            let selector = object
                .get("selector")
                .and_then(Value::as_array)
                .ok_or_else(|| anyhow!("data_model list {field} selector must be array"))?
                .iter()
                .map(|segment| {
                    segment.as_str().map(str::to_string).ok_or_else(|| {
                        anyhow!("data_model list {field} selector segment must be string")
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            lookup_selector_value(variable_pool, &selector)
        }
        Some(kind) => bail!("data_model list {field} kind is unsupported: {kind}"),
        None => bail!("data_model list {field} kind is required"),
    }
}

fn required_data_model_query_string(
    object: &Map<String, Value>,
    key: &'static str,
    context: &'static str,
) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("data_model list {context} {key} is required"))
}

pub fn lookup_selector_value(
    variable_pool: &Map<String, Value>,
    selector: &[String],
) -> Result<Value> {
    let mut segments = selector.iter();
    let first = segments
        .next()
        .ok_or_else(|| anyhow!("selector binding is missing selector path"))?;
    let mut cursor = variable_pool
        .get(first)
        .ok_or_else(|| anyhow!("selector source not found: {}", selector.join(".")))?;

    for segment in segments {
        cursor = cursor
            .get(segment)
            .ok_or_else(|| anyhow!("selector path not found: {}", selector.join(".")))?;
    }

    Ok(cursor.clone())
}

pub(crate) fn render_template(
    template: &str,
    variable_pool: &Map<String, Value>,
) -> Result<String> {
    let mut rendered = String::new();
    let mut cursor = 0;

    while let Some(start_offset) = template[cursor..].find("{{") {
        let start = cursor + start_offset;
        rendered.push_str(&template[cursor..start]);
        let token_start = start + 2;
        let Some(end_offset) = template[token_start..].find("}}") else {
            rendered.push_str(&template[start..]);
            return Ok(rendered);
        };
        let token_end = token_start + end_offset;
        let token = template[token_start..token_end].trim();
        let replacement = token.split('.').map(str::to_string).collect::<Vec<_>>();

        if replacement.len() >= 2 {
            match lookup_selector_value(variable_pool, &replacement) {
                Ok(Value::String(text)) => rendered.push_str(&text),
                Ok(Value::Null) => rendered.push_str("null"),
                Ok(value) => rendered.push_str(&value.to_string()),
                Err(error) => bail!(
                    "unresolved template selector {}: {error}",
                    replacement.join(".")
                ),
            }
        } else {
            bail!("unresolved template selector {token}: selector path must include a source");
        }

        cursor = token_end + 2;
    }

    rendered.push_str(&template[cursor..]);
    Ok(rendered)
}

fn render_prompt_content_template(
    template: &str,
    variable_pool: &Map<String, Value>,
) -> Result<Value> {
    let mut rendered_text = String::new();
    let mut rendered_blocks = Vec::new();
    let mut uses_prompt_blocks = false;
    let mut cursor = 0;

    while let Some(start_offset) = template[cursor..].find("{{") {
        let start = cursor + start_offset;
        append_prompt_template_text(
            &mut rendered_text,
            &mut rendered_blocks,
            uses_prompt_blocks,
            &template[cursor..start],
        );
        let token_start = start + 2;
        let Some(end_offset) = template[token_start..].find("}}") else {
            append_prompt_template_text(
                &mut rendered_text,
                &mut rendered_blocks,
                uses_prompt_blocks,
                &template[start..],
            );
            return prompt_template_value(rendered_text, rendered_blocks, uses_prompt_blocks);
        };
        let token_end = token_start + end_offset;
        let token = template[token_start..token_end].trim();
        let selector = token.split('.').map(str::to_string).collect::<Vec<_>>();
        if selector.len() < 2 {
            bail!("unresolved template selector {token}: selector path must include a source");
        }

        let value = lookup_selector_value(variable_pool, &selector).map_err(|error| {
            anyhow!(
                "unresolved template selector {}: {error}",
                selector.join(".")
            )
        })?;
        match value {
            Value::Array(_) => {
                let blocks: Vec<NativePromptBlock> = serde_json::from_value(value).map_err(|_| {
                    anyhow!(
                        "prompt template selector {} must resolve to typed prompt blocks, not an arbitrary array",
                        selector.join(".")
                    )
                })?;
                if !uses_prompt_blocks {
                    if !rendered_text.is_empty() {
                        rendered_blocks
                            .push(NativePromptBlock::text(std::mem::take(&mut rendered_text)));
                    }
                    uses_prompt_blocks = true;
                }
                rendered_blocks.extend(blocks);
            }
            Value::Object(_) => {
                bail!(
                    "prompt template selector {} cannot implicitly stringify an object",
                    selector.join(".")
                );
            }
            Value::String(text) => append_prompt_template_text(
                &mut rendered_text,
                &mut rendered_blocks,
                uses_prompt_blocks,
                &text,
            ),
            Value::Null => append_prompt_template_text(
                &mut rendered_text,
                &mut rendered_blocks,
                uses_prompt_blocks,
                "null",
            ),
            scalar => append_prompt_template_text(
                &mut rendered_text,
                &mut rendered_blocks,
                uses_prompt_blocks,
                &scalar.to_string(),
            ),
        }

        cursor = token_end + 2;
    }

    append_prompt_template_text(
        &mut rendered_text,
        &mut rendered_blocks,
        uses_prompt_blocks,
        &template[cursor..],
    );
    prompt_template_value(rendered_text, rendered_blocks, uses_prompt_blocks)
}

fn append_prompt_template_text(
    rendered_text: &mut String,
    rendered_blocks: &mut Vec<NativePromptBlock>,
    uses_prompt_blocks: bool,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    if uses_prompt_blocks {
        rendered_blocks.push(NativePromptBlock::text(text));
    } else {
        rendered_text.push_str(text);
    }
}

fn prompt_template_value(
    rendered_text: String,
    mut rendered_blocks: Vec<NativePromptBlock>,
    uses_prompt_blocks: bool,
) -> Result<Value> {
    if !uses_prompt_blocks {
        return Ok(Value::String(rendered_text));
    }
    if !rendered_text.is_empty() {
        rendered_blocks.push(NativePromptBlock::text(rendered_text));
    }
    serde_json::to_value(rendered_blocks).map_err(Into::into)
}

fn render_numeric_template_expression(
    template: &str,
    variable_pool: &Map<String, Value>,
) -> Result<Value> {
    let mut expression = String::new();
    let mut cursor = 0;

    while let Some(start_offset) = template[cursor..].find("{{") {
        let start = cursor + start_offset;
        expression.push_str(&template[cursor..start]);
        let token_start = start + 2;
        let Some(end_offset) = template[token_start..].find("}}") else {
            expression.push_str(&template[start..]);
            break;
        };
        let token_end = token_start + end_offset;
        let token = template[token_start..token_end].trim();
        let selector = token.split('.').map(str::to_string).collect::<Vec<_>>();

        if selector.len() < 2 {
            bail!("numeric expression selector {token} must include a source");
        }

        let value = lookup_selector_value(variable_pool, &selector).map_err(|error| {
            anyhow!(
                "unresolved numeric expression selector {}: {error}",
                selector.join(".")
            )
        })?;
        let number = value.as_f64().ok_or_else(|| {
            anyhow!(
                "numeric expression selector {} is not a number",
                selector.join(".")
            )
        })?;

        expression.push_str(&number.to_string());
        cursor = token_end + 2;
    }

    expression.push_str(&template[cursor..]);

    let result = NumericExpressionParser::new(&expression).parse()?;
    let number = serde_json::Number::from_f64(result)
        .ok_or_else(|| anyhow!("numeric expression result must be finite"))?;

    Ok(Value::Number(number))
}

struct NumericExpressionParser<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> NumericExpressionParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<f64> {
        let value = self.parse_expression()?;
        self.skip_whitespace();

        if self.cursor != self.input.len() {
            bail!("numeric expression contains unsupported syntax");
        }

        if !value.is_finite() {
            bail!("numeric expression result must be finite");
        }

        Ok(value)
    }

    fn parse_expression(&mut self) -> Result<f64> {
        let mut value = self.parse_term()?;

        loop {
            self.skip_whitespace();

            if self.consume(b'+') {
                value += self.parse_term()?;
            } else if self.consume(b'-') {
                value -= self.parse_term()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64> {
        let mut value = self.parse_factor()?;

        loop {
            self.skip_whitespace();

            if self.consume(b'*') {
                value *= self.parse_factor()?;
            } else if self.consume(b'/') {
                value /= self.parse_factor()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_factor(&mut self) -> Result<f64> {
        self.skip_whitespace();

        if self.consume(b'+') {
            return self.parse_factor();
        }

        if self.consume(b'-') {
            return Ok(-self.parse_factor()?);
        }

        if self.consume(b'(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();

            if !self.consume(b')') {
                bail!("numeric expression has an unclosed group");
            }

            return Ok(value);
        }

        self.parse_number()
    }

    fn parse_number(&mut self) -> Result<f64> {
        self.skip_whitespace();

        let start = self.cursor;
        let mut has_digit = false;

        while self.peek().is_some_and(|value| value.is_ascii_digit()) {
            has_digit = true;
            self.cursor += 1;
        }

        if self.consume(b'.') {
            while self.peek().is_some_and(|value| value.is_ascii_digit()) {
                has_digit = true;
                self.cursor += 1;
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            let exponent_start = self.cursor;
            self.cursor += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.cursor += 1;
            }

            let exponent_digit_start = self.cursor;
            while self.peek().is_some_and(|value| value.is_ascii_digit()) {
                self.cursor += 1;
            }

            if exponent_digit_start == self.cursor {
                self.cursor = exponent_start;
            }
        }

        if !has_digit {
            bail!("numeric expression expected a number");
        }

        std::str::from_utf8(&self.input[start..self.cursor])?
            .parse::<f64>()
            .map_err(|error| anyhow!("numeric expression number is invalid: {error}"))
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|value| value.is_ascii_whitespace()) {
            self.cursor += 1;
        }
    }

    fn consume(&mut self, value: u8) -> bool {
        if self.peek() == Some(value) {
            self.cursor += 1;
            return true;
        }

        false
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.cursor).copied()
    }
}

fn render_template_with_issues(
    binding_key: &str,
    template: &str,
    variable_pool: &Map<String, Value>,
    issues: &mut Vec<BindingResolutionIssue>,
) -> String {
    let mut rendered = String::new();
    let mut cursor = 0;

    while let Some(start_offset) = template[cursor..].find("{{") {
        let start = cursor + start_offset;
        rendered.push_str(&template[cursor..start]);
        let token_start = start + 2;
        let Some(end_offset) = template[token_start..].find("}}") else {
            rendered.push_str(&template[start..]);
            return rendered;
        };
        let token_end = token_start + end_offset;
        let token = template[token_start..token_end].trim();
        let selector = token.split('.').map(str::to_string).collect::<Vec<_>>();

        if selector.len() >= 2 {
            match lookup_selector_value(variable_pool, &selector) {
                Ok(Value::String(text)) => rendered.push_str(&text),
                Ok(Value::Null) => rendered.push_str("null"),
                Ok(value) => rendered.push_str(&value.to_string()),
                Err(error) => issues.push(BindingResolutionIssue {
                    binding_key: binding_key.to_string(),
                    selector: Some(selector),
                    message: error.to_string(),
                }),
            }
        } else {
            issues.push(BindingResolutionIssue {
                binding_key: binding_key.to_string(),
                selector: Some(selector),
                message: format!(
                    "unresolved template selector {token}: selector path must include a source"
                ),
            });
        }

        cursor = token_end + 2;
    }

    rendered.push_str(&template[cursor..]);
    rendered
}
