use crate::binding_runtime::lookup_selector_value;
use anyhow::{anyhow, bail, Result};
use serde_json::{Map, Value};

/// Container conditions are strict. Ordinary if/else retains its historical
/// missing-value behavior; a failed loop condition must consume a failed round.
pub(super) fn strict_condition(group: &Value, pool: &Map<String, Value>) -> Result<bool> {
    let conditions = group
        .get("conditions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("condition group must include conditions"))?;
    let operator = group
        .get("operator")
        .and_then(Value::as_str)
        .unwrap_or("and");
    if !matches!(operator, "and" | "or") {
        bail!("unknown condition group operator");
    }
    if conditions.is_empty() {
        return Ok(false);
    }
    let mut values = Vec::with_capacity(conditions.len());
    // Validate all terms, including short-circuited ones, so malformed selectors
    // cannot silently change meaning when another term changes value.
    for condition in conditions {
        if condition.get("conditions").is_some() {
            values.push(strict_condition(condition, pool)?);
            continue;
        }
        let left = lookup_selector_value(
            pool,
            &selector(
                condition
                    .get("left")
                    .ok_or_else(|| anyhow!("condition left missing"))?,
            )?,
        )?;
        let comparator = condition
            .get("comparator")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("condition comparator missing"))?;
        if comparator == "exists" {
            values.push(!left.is_null());
            continue;
        }
        if comparator == "empty" {
            values.push(
                left.is_null()
                    || left.as_str().is_some_and(str::is_empty)
                    || left.as_array().is_some_and(Vec::is_empty),
            );
            continue;
        }
        let right = condition
            .get("right")
            .ok_or_else(|| anyhow!("condition right missing"))?;
        let right = if right.is_array() {
            lookup_selector_value(pool, &selector(right)?)?
        } else {
            match right.get("kind").and_then(Value::as_str) {
                Some("selector") => lookup_selector_value(
                    pool,
                    &selector(
                        right
                            .get("selector")
                            .ok_or_else(|| anyhow!("condition selector missing"))?,
                    )?,
                )?,
                Some("constant") => right
                    .get("value")
                    .cloned()
                    .ok_or_else(|| anyhow!("condition constant missing"))?,
                Some(_) => bail!("condition value kind unsupported"),
                None => right.clone(),
            }
        };
        let value = match comparator {
            "equals" => left == right,
            "not_equals" => left != right,
            "greater_than" => number(&left)? > number(&right)?,
            "greater_than_or_equals" => number(&left)? >= number(&right)?,
            "less_than" => number(&left)? < number(&right)?,
            "less_than_or_equals" => number(&left)? <= number(&right)?,
            "contains" => string(&left)?.contains(string(&right)?),
            "starts_with" => string(&left)?.starts_with(string(&right)?),
            "ends_with" => string(&left)?.ends_with(string(&right)?),
            "matches_regex" => regex::Regex::new(string(&right)?)?.is_match(string(&left)?),
            _ => bail!("condition comparator unsupported: {comparator}"),
        };
        values.push(value);
    }
    Ok(if operator == "and" {
        values.into_iter().all(|value| value)
    } else {
        values.into_iter().any(|value| value)
    })
}
fn selector(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .ok_or_else(|| anyhow!("condition selector must be an array"))?
        .iter()
        .map(|segment| {
            segment
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("condition selector segment must be a string"))
        })
        .collect()
}
fn number(value: &Value) -> Result<f64> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| anyhow!("numeric condition requires finite numbers"))
}
fn string(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| anyhow!("text condition requires strings"))
}
