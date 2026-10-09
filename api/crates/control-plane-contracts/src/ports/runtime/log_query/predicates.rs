use super::*;
use domain::{ResourceFilterExpr as F, ResourceFilterOperator as O};
use serde_json::Value;

fn invalid(code: &'static str) -> anyhow::Error {
    crate::ControlPlaneContractError::InvalidInput(code).into()
}
fn operator_name(operator: O) -> &'static str {
    match operator {
        O::Eq => "$eq",
        O::Ne => "$ne",
        O::Gt => "$gt",
        O::Gte => "$gte",
        O::Lt => "$lt",
        O::Lte => "$lte",
        O::In => "$in",
        O::Includes => "$includes",
        O::NotIncludes => "$notIncludes",
    }
}
fn valid_value(kind: LogQueryValueType, value: &Value) -> bool {
    use LogQueryValueType::*;
    match kind {
        String => value.is_string(),
        Uuid => value
            .as_str()
            .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok()),
        Number => value.as_f64().is_some_and(f64::is_finite),
        Boolean => value.is_boolean(),
        Datetime => value.as_str().is_some_and(|s| {
            time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).is_ok()
        }),
    }
}
/// Validate every branch before scanning or executing SQL (including empty results).
pub fn validate_log_query_filter(filter: &F, fields: &[LogQueryField]) -> anyhow::Result<()> {
    match filter {
        F::All(items) | F::Any(items) => {
            for item in items {
                validate_log_query_filter(item, fields)?;
            }
        }
        F::Field {
            field,
            operator,
            value,
        } => {
            let field = fields
                .iter()
                .find(|f| f.field == *field)
                .ok_or_else(|| invalid("log_query_unknown_field"))?;
            if !field
                .operators
                .iter()
                .any(|name| name == operator_name(*operator))
            {
                return Err(invalid("log_query_operator"));
            }
            let valid = if *operator == O::In {
                value
                    .as_array()
                    .is_some_and(|values| values.iter().all(|v| valid_value(field.value_type, v)))
            } else if value.is_null() {
                matches!(operator, O::Eq | O::Ne)
            } else {
                valid_value(field.value_type, value)
            };
            if !valid {
                return Err(invalid("log_query_value_type"));
            }
        }
    }
    Ok(())
}
/// SQL-like null semantics; UUIDs and times compare by typed value.
pub fn log_query_filter_matches(filter: &F, record: &Value, fields: &[LogQueryField]) -> bool {
    match filter {
        F::All(items) => items
            .iter()
            .all(|f| log_query_filter_matches(f, record, fields)),
        F::Any(items) => items
            .iter()
            .any(|f| log_query_filter_matches(f, record, fields)),
        F::Field {
            field,
            operator,
            value,
        } => {
            let actual = record.get(field).unwrap_or(&Value::Null);
            if value.is_null() {
                return match operator {
                    O::Eq => actual.is_null(),
                    O::Ne => !actual.is_null(),
                    _ => false,
                };
            }
            if actual.is_null() {
                return false;
            }
            let Some(kind) = fields
                .iter()
                .find(|f| f.field == *field)
                .map(|f| f.value_type)
            else {
                return false;
            };
            let compare = |right: &Value| {
                use LogQueryValueType::*;
                match kind {
                    Number => compare_number(actual, right),
                    Datetime => {
                        let parse = |v: &Value| {
                            time::OffsetDateTime::parse(
                                v.as_str()?,
                                &time::format_description::well_known::Rfc3339,
                            )
                            .ok()
                        };
                        Some(parse(actual)?.cmp(&parse(right)?))
                    }
                    Uuid => Some(
                        uuid::Uuid::parse_str(actual.as_str()?)
                            .ok()?
                            .cmp(&uuid::Uuid::parse_str(right.as_str()?).ok()?),
                    ),
                    Boolean => Some(actual.as_bool()?.cmp(&right.as_bool()?)),
                    String => Some(actual.as_str()?.cmp(right.as_str()?)),
                }
            };
            use std::cmp::Ordering::*;
            match operator {
                O::Eq => compare(value) == Some(Equal),
                O::Ne => compare(value).is_some_and(|c| c != Equal),
                O::Gt => compare(value) == Some(Greater),
                O::Gte => compare(value).is_some_and(|c| c != Less),
                O::Lt => compare(value) == Some(Less),
                O::Lte => compare(value).is_some_and(|c| c != Greater),
                O::In => value
                    .as_array()
                    .is_some_and(|values| values.iter().any(|v| compare(v) == Some(Equal))),
                O::Includes | O::NotIncludes => {
                    let matched = actual
                        .as_str()
                        .zip(value.as_str())
                        .is_some_and(|(a, b)| log_query_text_pattern_matches(a, b));
                    if *operator == O::Includes {
                        matched
                    } else {
                        !matched
                    }
                }
            }
        }
    }
}
pub fn log_query_search_sections(
    query: &RecordClientTrajectoryQuery,
) -> anyhow::Result<Vec<String>> {
    let mut sections = if query.search_sections.is_empty() {
        vec!["result".to_owned()]
    } else {
        query.search_sections.clone()
    };
    if sections
        .iter()
        .any(|s| !matches!(s.as_str(), "overview" | "parameters" | "result"))
    {
        return Err(invalid("log_query_search_section"));
    }
    if query.keyword.as_ref().is_some_and(|s| s.trim().is_empty()) {
        return Err(invalid("log_query_keyword"));
    }
    sections.sort();
    sections.dedup();
    Ok(sections)
}
/// Bounded Unicode-safe context around a case-insensitive literal hit.
pub fn log_query_snippet(text: &str, keyword: &str) -> Option<String> {
    let needle = keyword.to_lowercase();
    if needle.is_empty() {
        return None;
    }
    // Search original character boundaries: Unicode lowercase expansion must
    // not become an offset into the original string. Keep only a bounded window.
    for (start, _) in text.char_indices() {
        let mut window = String::new();
        for (relative, character) in text[start..].char_indices() {
            window.extend(character.to_lowercase());
            if window == needle {
                let end = start + relative + character.len_utf8();
                let context_start = text[..start]
                    .char_indices()
                    .rev()
                    .nth(79)
                    .map_or(0, |(offset, _)| offset);
                let context_end = text[end..]
                    .char_indices()
                    .nth(80)
                    .map_or(text.len(), |(offset, _)| end + offset);
                return Some(text[context_start..context_end].to_owned());
            }
            if !needle.starts_with(&window) {
                break;
            }
        }
    }
    None
}

/// Task TEXT snapshots cannot represent a literal NUL. Trajectory predicates
/// operate on restored JSON originals and deliberately accept it.
pub fn validate_application_log_query_filter(filter: &F) -> anyhow::Result<()> {
    validate_log_query_filter(filter, &application_log_query_fields())?;
    fn has_nul(value: &Value) -> bool {
        match value {
            Value::String(s) => s.contains('\0'),
            Value::Array(values) => values.iter().any(has_nul),
            Value::Object(values) => values.values().any(has_nul),
            _ => false,
        }
    }
    fn visit(filter: &F) -> bool {
        match filter {
            F::All(items) | F::Any(items) => items.iter().any(visit),
            F::Field { value, .. } => has_nul(value),
        }
    }
    if visit(filter) {
        return Err(invalid("log_query_task_text_nul"));
    }
    Ok(())
}

/// New queries impose no business result ceiling; reserve one lookahead item.
pub fn log_query_page_lookahead(limit: i64) -> anyhow::Result<i64> {
    if limit <= 0 {
        return Err(invalid("log_query_limit"));
    }
    limit
        .checked_add(1)
        .ok_or_else(|| invalid("log_query_limit"))
}

/// Same containment-pattern convention as the existing parameterized SQL
/// helper: `%` matches any span, `_` one character and `\\` escapes a wildcard.
/// This filter is distinct from literal semantic keyword search.
pub fn log_query_text_pattern_matches(text: &str, pattern: &str) -> bool {
    #[derive(Clone, Copy)]
    enum Token {
        Any,
        One,
        Literal(char),
    }
    let lowered = pattern.to_lowercase();
    let mut input = lowered.chars();
    let mut tokens = vec![Token::Any];
    while let Some(character) = input.next() {
        tokens.push(match character {
            '%' => Token::Any,
            '_' => Token::One,
            '\\' => Token::Literal(input.next().unwrap_or('%')),
            character => Token::Literal(character),
        });
    }
    // SQL appends `%` to the bound includes pattern. A trailing backslash in
    // the supplied value escapes that appended wildcard into a literal `%`.
    let escaped_suffix = lowered.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1;
    if !escaped_suffix {
        tokens.push(Token::Any);
    }
    let mut previous = vec![false; tokens.len() + 1];
    previous[0] = true;
    for (index, token) in tokens.iter().enumerate() {
        previous[index + 1] = matches!(token, Token::Any) && previous[index];
    }
    let mut current = vec![false; tokens.len() + 1];
    for character in text.chars().flat_map(char::to_lowercase) {
        current.fill(false);
        for (index, token) in tokens.iter().enumerate() {
            current[index + 1] = match token {
                Token::Any => current[index] || previous[index + 1],
                Token::One => previous[index],
                Token::Literal(expected) => *expected == character && previous[index],
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[tokens.len()]
}

// JSON integers must retain their exact order, including values above 2^53.
// Rust float-to-integer conversion saturates; when the truncated integer ties,
// the fractional remainder determines the mathematical mixed-number order.
fn compare_number(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
    fn integer(value: &Value) -> Option<i128> {
        value
            .as_i64()
            .map(i128::from)
            .or_else(|| value.as_u64().map(i128::from))
    }
    fn integer_float(integer: i128, float: f64) -> Option<std::cmp::Ordering> {
        if !float.is_finite() {
            return None;
        }
        let order = integer.cmp(&(float as i128));
        if order == std::cmp::Ordering::Equal {
            0.0_f64.partial_cmp(&float.fract())
        } else {
            Some(order)
        }
    }
    match (integer(left), integer(right)) {
        (Some(left), Some(right)) => Some(left.cmp(&right)),
        (Some(left), None) => integer_float(left, right.as_f64()?),
        (None, Some(right)) => {
            integer_float(right, left.as_f64()?).map(std::cmp::Ordering::reverse)
        }
        (None, None) => left.as_f64()?.partial_cmp(&right.as_f64()?),
    }
}
