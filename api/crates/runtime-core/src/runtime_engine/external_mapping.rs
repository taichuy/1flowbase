use super::*;

pub(super) fn external_data_source_instance_id(metadata: &ModelMetadata) -> Result<Uuid> {
    metadata
        .data_source_instance_id
        .ok_or_else(|| anyhow!("external data source instance is not configured"))
}

pub(super) fn external_resource_key(metadata: &ModelMetadata) -> Result<String> {
    metadata
        .external_resource_key
        .clone()
        .ok_or_else(|| anyhow!("external resource key is not configured"))
}

pub(super) fn external_filters(
    metadata: &ModelMetadata,
    filter: &domain::ResourceFilterExpr,
) -> Result<Vec<DataSourceRecordFilter>> {
    let mut filters = Vec::new();
    collect_external_filters(metadata, filter, &mut filters)?;
    Ok(filters)
}

pub(super) fn collect_external_filters(
    metadata: &ModelMetadata,
    filter: &domain::ResourceFilterExpr,
    filters: &mut Vec<DataSourceRecordFilter>,
) -> Result<()> {
    match filter {
        domain::ResourceFilterExpr::All(items) => {
            for item in items {
                collect_external_filters(metadata, item, filters)?;
            }
            Ok(())
        }
        domain::ResourceFilterExpr::Any(_) => Err(anyhow!(
            "external data source filters do not support $or expressions"
        )),
        domain::ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => {
            filters.push(DataSourceRecordFilter {
                field_key: external_field_key(metadata, field)?,
                operator: runtime_filter_operator_code(*operator)?.to_string(),
                value: value.clone(),
            });
            Ok(())
        }
    }
}

pub(super) fn runtime_filter_operator_code(
    operator: domain::ResourceFilterOperator,
) -> Result<&'static str> {
    match operator {
        domain::ResourceFilterOperator::Eq => Ok("eq"),
        domain::ResourceFilterOperator::Ne => Ok("ne"),
        domain::ResourceFilterOperator::Gt => Ok("gt"),
        domain::ResourceFilterOperator::Gte => Ok("gte"),
        domain::ResourceFilterOperator::Lt => Ok("lt"),
        domain::ResourceFilterOperator::Lte => Ok("lte"),
        domain::ResourceFilterOperator::Includes => Ok("includes"),
        domain::ResourceFilterOperator::NotIncludes => Ok("notIncludes"),
        domain::ResourceFilterOperator::In => Ok("in"),
    }
}

pub(super) fn external_sorts(
    metadata: &ModelMetadata,
    sorts: Vec<RuntimeSortInput>,
) -> Result<Vec<DataSourceRecordSort>> {
    sorts
        .into_iter()
        .map(|sort| {
            let direction = sort.direction.to_ascii_lowercase();
            let descending = match direction.as_str() {
                "asc" => false,
                "desc" => true,
                _ => return Err(anyhow!("unsupported sort direction")),
            };

            Ok(DataSourceRecordSort {
                field_key: external_field_key(metadata, &sort.field_code)?,
                descending,
            })
        })
        .collect()
}

pub(super) fn runtime_payload_to_external(
    metadata: &ModelMetadata,
    payload: Value,
) -> Result<Value> {
    let Value::Object(object) = payload else {
        return Err(anyhow!("runtime payload must be object"));
    };
    let mut mapped = serde_json::Map::new();

    for (field_code, value) in object {
        let field_key = external_field_key(metadata, &field_code)?;
        mapped.insert(field_key, value);
    }

    Ok(Value::Object(mapped))
}

pub(super) fn external_record_to_runtime(metadata: &ModelMetadata, record: Value) -> Value {
    let Value::Object(object) = record else {
        return record;
    };
    let mut mapped = serde_json::Map::new();

    for (field_key, value) in object {
        if let Some(field_code) = runtime_field_code(metadata, &field_key) {
            mapped.insert(field_code, value);
        }
    }

    Value::Object(mapped)
}

pub(super) fn external_field_key(metadata: &ModelMetadata, field_code: &str) -> Result<String> {
    if let Some(field) = metadata.field_by_code(field_code) {
        return Ok(field
            .external_field_key
            .clone()
            .unwrap_or_else(|| field.code.clone()));
    }

    if is_platform_runtime_field(field_code) {
        Ok(field_code.to_string())
    } else {
        Err(anyhow!("unknown runtime field: {field_code}"))
    }
}

pub(super) fn runtime_field_code(metadata: &ModelMetadata, field_key: &str) -> Option<String> {
    if let Some(field) = metadata
        .fields
        .iter()
        .find(|field| field.external_field_key.as_deref().unwrap_or(&field.code) == field_key)
    {
        return Some(field.code.clone());
    }

    is_platform_runtime_field(field_key).then(|| field_key.to_string())
}

pub(super) fn is_platform_runtime_field(field_code: &str) -> bool {
    matches!(
        field_code,
        "id" | "created_by" | "created_at" | "updated_by" | "updated_at" | "deleted_at"
    )
}

pub(super) fn data_source_context(
    scope_id: Option<Uuid>,
    owner_user_id: Option<Uuid>,
) -> DataSourceRecordScopeContext {
    DataSourceRecordScopeContext {
        owner_id: owner_user_id.map(|id| id.to_string()),
        scope_id: scope_id.map(|id| id.to_string()),
    }
}

pub(super) fn data_source_page(page: i64, page_size: i64) -> DataSourceRecordPage {
    let page = page.max(1) as u64;
    let page_size = page_size.max(1) as u64;
    DataSourceRecordPage {
        limit: Some(page_size.min(u32::MAX as u64) as u32),
        cursor: None,
        offset: Some((page - 1) * page_size),
    }
}
