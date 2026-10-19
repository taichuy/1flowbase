use std::{cmp::Ordering, collections::HashMap, sync::Mutex};

use super::*;

#[derive(Default)]
pub(super) struct InMemoryRuntimeRecordRepository {
    records: Mutex<HashMap<String, HashMap<Uuid, Vec<Value>>>>,
}

#[async_trait]
impl RuntimeRecordRepository for InMemoryRuntimeRecordRepository {
    async fn list_records(
        &self,
        metadata: &ModelMetadata,
        query: RuntimeListQuery,
    ) -> Result<RuntimeListResult> {
        let page = query.page.max(1);
        let page_size = query.page_size.max(1);
        let records = self.records.lock().expect("runtime record lock poisoned");
        let mut items = records
            .get(&metadata.model_code)
            .map(|scopes| match query.scope_id {
                Some(scope_id) => scopes.get(&scope_id).cloned().unwrap_or_default(),
                None => scopes
                    .values()
                    .flat_map(|items| items.iter().cloned())
                    .collect(),
            })
            .unwrap_or_default();
        items.retain(|item| owner_matches(item, query.owner_user_id));
        items.retain(|item| filter_matches(item, &query.filter));
        items.sort_by(|left, right| compare_records(left, right, &query.sorts));
        let total = items.len() as i64;
        let offset = ((page - 1) * page_size) as usize;
        let items = items
            .into_iter()
            .skip(offset)
            .take(page_size as usize)
            .collect();

        Ok(RuntimeListResult { items, total })
    }

    async fn get_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
    ) -> Result<Option<Value>> {
        let records = self.records.lock().expect("runtime record lock poisoned");
        let Some(scopes) = records.get(&metadata.model_code) else {
            return Ok(None);
        };
        let find_record = |items: &Vec<Value>| {
            items
                .iter()
                .find(|item| {
                    item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id)
                })
                .cloned()
        };

        Ok(match scope_id {
            Some(scope_id) => scopes.get(&scope_id).and_then(find_record),
            None => scopes.values().find_map(find_record),
        })
    }

    async fn create_record(
        &self,
        metadata: &ModelMetadata,
        actor_user_id: Uuid,
        scope_id: Uuid,
        payload: Value,
    ) -> Result<Value> {
        let mut record = object_payload(payload);
        record
            .entry("id".to_string())
            .or_insert_with(|| serde_json::json!(Uuid::now_v7()));
        record.insert(
            "created_by".to_string(),
            nullable_actor_user_id(actor_user_id)
                .map(|user_id| serde_json::Value::String(user_id.to_string()))
                .unwrap_or(serde_json::Value::Null),
        );
        let value = Value::Object(record);

        let mut records = self.records.lock().expect("runtime record lock poisoned");
        records
            .entry(metadata.model_code.clone())
            .or_default()
            .entry(scope_id)
            .or_default()
            .push(value.clone());

        Ok(value)
    }

    async fn update_record(
        &self,
        metadata: &ModelMetadata,
        _actor_user_id: Uuid,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
        payload: Value,
    ) -> Result<Value> {
        let patch = object_payload(payload);
        let mut records = self.records.lock().expect("runtime record lock poisoned");
        let scopes = records.entry(metadata.model_code.clone()).or_default();
        let scoped_records = match scope_id {
            Some(scope_id) => scopes.entry(scope_id).or_default(),
            None => scopes
                .values_mut()
                .find(|items| {
                    items.iter().any(|item| {
                        item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id)
                    })
                })
                .ok_or_else(|| anyhow!("runtime record not found"))?,
        };
        let record = scoped_records
            .iter_mut()
            .find(|item| {
                item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id)
            })
            .ok_or_else(|| anyhow!("runtime record not found"))?;
        let object = record
            .as_object_mut()
            .ok_or_else(|| anyhow!("runtime record must be object"))?;

        for (key, value) in patch {
            object.insert(key, value);
        }

        Ok(record.clone())
    }

    async fn delete_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
    ) -> Result<bool> {
        let mut records = self.records.lock().expect("runtime record lock poisoned");
        let mut deleted = false;
        let mut retain_record = |item: &Value| {
            let matches =
                item["id"].as_str() == Some(record_id) && owner_matches(item, owner_user_id);
            if matches {
                deleted = true;
                false
            } else {
                true
            }
        };
        let scopes = records.entry(metadata.model_code.clone()).or_default();
        match scope_id {
            Some(scope_id) => {
                scopes
                    .entry(scope_id)
                    .or_default()
                    .retain(&mut retain_record);
            }
            None => {
                for records in scopes.values_mut() {
                    records.retain(&mut retain_record);
                }
            }
        }

        Ok(deleted)
    }
}

pub(super) struct NoopRecordValidator;

#[async_trait]
impl RecordValidator for NoopRecordValidator {
    async fn validate(
        &self,
        _actor_user_id: Uuid,
        _model_code: &str,
        _payload: &Value,
    ) -> Result<()> {
        Ok(())
    }
}

pub(super) struct PassthroughValueResolver;

#[async_trait]
impl DefaultValueResolver for PassthroughValueResolver {
    async fn apply(
        &self,
        _actor_user_id: Uuid,
        _model_code: &str,
        payload: Value,
    ) -> Result<Value> {
        Ok(Value::Object(object_payload(payload)))
    }
}

fn object_payload(payload: Value) -> serde_json::Map<String, Value> {
    match payload {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("value".to_string(), other);
            map
        }
    }
}

fn nullable_actor_user_id(actor_user_id: Uuid) -> Option<Uuid> {
    (!actor_user_id.is_nil()).then_some(actor_user_id)
}

fn owner_matches(record: &Value, owner_user_id: Option<Uuid>) -> bool {
    match owner_user_id {
        None => true,
        Some(owner_user_id) => {
            record
                .get("created_by")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                == Some(owner_user_id)
        }
    }
}

fn filter_matches(record: &Value, filter: &domain::ResourceFilterExpr) -> bool {
    match filter {
        domain::ResourceFilterExpr::All(items) => {
            items.iter().all(|item| filter_matches(record, item))
        }
        domain::ResourceFilterExpr::Any(items) => {
            items.iter().any(|item| filter_matches(record, item))
        }
        domain::ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => {
            let current = &record[field];
            match operator {
                domain::ResourceFilterOperator::Eq => current == value,
                domain::ResourceFilterOperator::Ne => current != value,
                domain::ResourceFilterOperator::Gt => {
                    compare_json_values(current, value) == Ordering::Greater
                }
                domain::ResourceFilterOperator::Gte => matches!(
                    compare_json_values(current, value),
                    Ordering::Greater | Ordering::Equal
                ),
                domain::ResourceFilterOperator::Lt => {
                    compare_json_values(current, value) == Ordering::Less
                }
                domain::ResourceFilterOperator::Lte => matches!(
                    compare_json_values(current, value),
                    Ordering::Less | Ordering::Equal
                ),
                domain::ResourceFilterOperator::Includes => current
                    .as_str()
                    .zip(value.as_str())
                    .is_some_and(|(current, value)| {
                        current.to_lowercase().contains(&value.to_lowercase())
                    }),
                domain::ResourceFilterOperator::NotIncludes => current
                    .as_str()
                    .zip(value.as_str())
                    .is_none_or(|(current, value)| {
                        !current.to_lowercase().contains(&value.to_lowercase())
                    }),
                domain::ResourceFilterOperator::In => value
                    .as_array()
                    .is_some_and(|values| values.iter().any(|value| current == value)),
            }
        }
    }
}

fn compare_records(left: &Value, right: &Value, sorts: &[RuntimeSortInput]) -> Ordering {
    for sort in sorts {
        let ordering = compare_json_values(&left[&sort.field_code], &right[&sort.field_code]);
        if ordering != Ordering::Equal {
            return if sort.direction.eq_ignore_ascii_case("desc") {
                ordering.reverse()
            } else {
                ordering
            };
        }
    }

    Ordering::Equal
}

fn compare_json_values(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.cmp(right),
        (Value::Number(left), Value::Number(right)) => left
            .as_f64()
            .partial_cmp(&right.as_f64())
            .unwrap_or(Ordering::Equal),
        (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
        _ => left.to_string().cmp(&right.to_string()),
    }
}

pub(super) fn test_model_metadata() -> ModelMetadata {
    ModelMetadata {
        model_id: Uuid::nil(),
        model_code: "orders".into(),
        status: domain::DataModelStatus::Published,
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: Uuid::nil(),
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_capability_snapshot: None,
        template_provider: domain::CORE_DATA_MODEL_TEMPLATE_PROVIDER.to_owned(),
        template_code: domain::GENERAL_DATA_MODEL_TEMPLATE_CODE.to_owned(),
        template_version: domain::GENERAL_DATA_MODEL_TEMPLATE_VERSION.to_owned(),
        physical_table_name: "rtm_workspace_demo_orders".into(),
        scope_column_name: "scope_id".into(),
        fields: vec![
            domain::ModelFieldRecord {
                id: Uuid::nil(),
                data_model_id: Uuid::nil(),
                code: "title".into(),
                title: "Title".into(),
                description: None,
                physical_column_name: "title".into(),
                external_field_key: None,
                field_kind: domain::ModelFieldKind::String,
                is_system: false,
                is_writable: true,
                is_required: true,
                api_required: true,
                is_unique: false,
                default_value: None,
                display_interface: Some("input".into()),
                display_options: serde_json::json!({}),
                relation_target_model_id: None,
                relation_options: serde_json::json!({}),
                sort_order: 0,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
            domain::ModelFieldRecord {
                id: Uuid::nil(),
                data_model_id: Uuid::nil(),
                code: "status".into(),
                title: "Status".into(),
                description: None,
                physical_column_name: "status".into(),
                external_field_key: None,
                field_kind: domain::ModelFieldKind::Enum,
                is_system: false,
                is_writable: true,
                is_required: true,
                api_required: true,
                is_unique: false,
                default_value: None,
                display_interface: Some("select".into()),
                display_options: serde_json::json!({ "options": ["draft", "paid"] }),
                relation_target_model_id: None,
                relation_options: serde_json::json!({}),
                sort_order: 1,
                availability_status: domain::MetadataAvailabilityStatus::Available,
            },
        ],
        record_capabilities: domain::DataModelRecordCapabilities::read_write(),
        resource: crate::resource_descriptor::ResourceDescriptor::runtime_model(
            "orders",
            domain::DataModelScopeKind::Workspace,
        ),
    }
}

