use serde_json::Value;

use crate::errors::ControlPlaneError;

use super::{filter_resource_records, parse_resource_filter_expr, ResourceFilterTarget};

#[derive(Debug, Clone)]
pub struct ResourceBatchSelection {
    pub filter_by_tk: Option<Value>,
    pub filter: Option<Value>,
}

impl ResourceBatchSelection {
    pub fn new(filter_by_tk: Option<Value>, filter: Option<Value>) -> Self {
        Self {
            filter_by_tk,
            filter,
        }
    }
}

pub fn filter_by_tk_values(filter_by_tk: Value) -> Result<Vec<String>, ControlPlaneError> {
    match filter_by_tk {
        Value::Array(items) => items
            .into_iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or(ControlPlaneError::InvalidInput("filterByTk"))
            })
            .collect(),
        Value::String(value) => Ok(vec![value]),
        _ => Err(ControlPlaneError::InvalidInput("filterByTk")),
    }
}

pub fn select_batch_ids<T, Id, ParseId, GetId>(
    records: Vec<T>,
    selection: ResourceBatchSelection,
    parse_id: ParseId,
    get_id: GetId,
) -> Result<Vec<Id>, ControlPlaneError>
where
    T: ResourceFilterTarget,
    ParseId: Fn(String) -> Result<Id, ControlPlaneError>,
    GetId: Fn(&T) -> Id,
{
    if let Some(filter_by_tk) = selection.filter_by_tk {
        return filter_by_tk_values(filter_by_tk)?
            .into_iter()
            .map(parse_id)
            .collect();
    }

    let Some(filter) = selection.filter else {
        return Err(ControlPlaneError::InvalidInput("filterByTk"));
    };
    if filter.as_object().is_none_or(|object| object.is_empty()) {
        return Err(ControlPlaneError::InvalidInput("filter"));
    }

    let expr = parse_resource_filter_expr(&filter)?;
    let filtered_records = filter_resource_records(records, Some(&expr))?;
    Ok(filtered_records.iter().map(get_id).collect())
}
