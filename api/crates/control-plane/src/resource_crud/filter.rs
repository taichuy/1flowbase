use serde_json::Value;

use crate::errors::ControlPlaneError;
use domain::{ResourceFilterExpr, ResourceFilterOperator};

pub trait ResourceFilterTarget {
    fn field_value(&self, field: &str) -> Option<String>;
}

pub fn parse_resource_filter(
    filter: Option<&str>,
) -> Result<Option<ResourceFilterExpr>, ControlPlaneError> {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    let filter =
        serde_json::from_str(filter).map_err(|_| ControlPlaneError::InvalidInput("filter"))?;
    parse_resource_filter_expr(&filter).map(Some)
}

pub fn parse_resource_filter_expr(filter: &Value) -> Result<ResourceFilterExpr, ControlPlaneError> {
    parse_filter_object(filter)
}

pub fn matches_resource_filter<T: ResourceFilterTarget>(
    target: &T,
    filter: &Value,
) -> Result<bool, ControlPlaneError> {
    let expr = parse_resource_filter_expr(filter)?;
    matches_filter_expr(target, &expr)
}

pub fn filter_resource_records<T: ResourceFilterTarget>(
    records: Vec<T>,
    filter: Option<&ResourceFilterExpr>,
) -> Result<Vec<T>, ControlPlaneError> {
    let Some(filter) = filter else {
        return Ok(records);
    };
    let mut filtered_records = Vec::with_capacity(records.len());
    for record in records {
        if matches_filter_expr(&record, filter)? {
            filtered_records.push(record);
        }
    }
    Ok(filtered_records)
}

fn parse_filter_object(filter: &Value) -> Result<ResourceFilterExpr, ControlPlaneError> {
    let Some(object) = filter.as_object() else {
        return Err(ControlPlaneError::InvalidInput("filter"));
    };

    let mut expressions = Vec::with_capacity(object.len());
    for (key, value) in object {
        match key.as_str() {
            "$and" => {
                let Some(items) = value.as_array() else {
                    return Err(ControlPlaneError::InvalidInput("filter"));
                };
                let items = items
                    .iter()
                    .map(parse_filter_object)
                    .collect::<Result<Vec<_>, _>>()?;
                expressions.push(ResourceFilterExpr::all(items));
            }
            "$or" => {
                let Some(items) = value.as_array() else {
                    return Err(ControlPlaneError::InvalidInput("filter"));
                };
                let items = items
                    .iter()
                    .map(parse_filter_object)
                    .collect::<Result<Vec<_>, _>>()?;
                expressions.push(ResourceFilterExpr::any(items));
            }
            operator if operator.starts_with('$') => {
                return Err(ControlPlaneError::InvalidInput("filter"));
            }
            field => expressions.push(parse_field_filter(field, value)?),
        }
    }

    Ok(ResourceFilterExpr::all(expressions))
}

fn parse_field_filter(
    field: &str,
    expected: &Value,
) -> Result<ResourceFilterExpr, ControlPlaneError> {
    let (field, dotted_operator) = field
        .rsplit_once('.')
        .filter(|(_, operator)| operator.starts_with('$'))
        .map_or((field, None), |(field, operator)| (field, Some(operator)));

    if let Some(operator) = dotted_operator {
        return parse_field_operator(field, operator, expected);
    }

    if let Some(operators) = expected.as_object() {
        if operators.keys().any(|key| key.starts_with('$')) {
            let mut expressions = Vec::with_capacity(operators.len());
            for (operator, value) in operators {
                expressions.push(parse_field_operator(field, operator, value)?);
            }
            return Ok(ResourceFilterExpr::all(expressions));
        }
    }

    Ok(ResourceFilterExpr::Field {
        field: field.to_string(),
        operator: ResourceFilterOperator::Eq,
        value: expected.clone(),
    })
}

fn parse_field_operator(
    field: &str,
    operator: &str,
    expected: &Value,
) -> Result<ResourceFilterExpr, ControlPlaneError> {
    let Some(operator) = ResourceFilterOperator::parse(operator) else {
        return Err(ControlPlaneError::InvalidInput("filter"));
    };
    if operator == ResourceFilterOperator::In && !expected.is_array() {
        return Err(ControlPlaneError::InvalidInput("filter"));
    }

    Ok(ResourceFilterExpr::Field {
        field: field.to_string(),
        operator,
        value: expected.clone(),
    })
}

fn matches_filter_expr<T: ResourceFilterTarget>(
    target: &T,
    expr: &ResourceFilterExpr,
) -> Result<bool, ControlPlaneError> {
    match expr {
        ResourceFilterExpr::All(items) => {
            for item in items {
                if !matches_filter_expr(target, item)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        ResourceFilterExpr::Any(items) => {
            for item in items {
                if matches_filter_expr(target, item)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => Ok(matches_field_operator(target, field, *operator, value)),
    }
}

fn matches_field_operator<T: ResourceFilterTarget>(
    target: &T,
    field: &str,
    operator: ResourceFilterOperator,
    expected: &Value,
) -> bool {
    let actual = target.field_value(field);
    let expected_text = value_to_filter_text(expected);

    match operator {
        ResourceFilterOperator::Eq => actual.as_deref() == Some(expected_text.as_str()),
        ResourceFilterOperator::Ne => actual.as_deref() != Some(expected_text.as_str()),
        ResourceFilterOperator::Gt
        | ResourceFilterOperator::Gte
        | ResourceFilterOperator::Lt
        | ResourceFilterOperator::Lte => {
            compare_text_operator(actual.as_deref(), operator, expected_text.as_str())
        }
        ResourceFilterOperator::Includes => actual
            .as_deref()
            .is_some_and(|value| value.to_lowercase().contains(&expected_text.to_lowercase())),
        ResourceFilterOperator::NotIncludes => actual
            .as_deref()
            .is_none_or(|value| !value.to_lowercase().contains(&expected_text.to_lowercase())),
        ResourceFilterOperator::In => expected.as_array().is_some_and(|values| {
            values.iter().any(|value| {
                let value = value_to_filter_text(value);
                actual.as_deref() == Some(value.as_str())
            })
        }),
    }
}

fn compare_text_operator(
    actual: Option<&str>,
    operator: ResourceFilterOperator,
    expected: &str,
) -> bool {
    let Some(actual) = actual else {
        return false;
    };
    match operator {
        ResourceFilterOperator::Gt => actual > expected,
        ResourceFilterOperator::Gte => actual >= expected,
        ResourceFilterOperator::Lt => actual < expected,
        ResourceFilterOperator::Lte => actual <= expected,
        _ => false,
    }
}

fn value_to_filter_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
