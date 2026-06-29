use std::{collections::HashSet, future::Future};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::ports::ModelDefinitionRepository;
use runtime_core::{
    model_metadata::ModelMetadata,
    runtime_engine::ensure_runtime_model_available,
    runtime_model_registry::RuntimeDataModelAvailability,
    runtime_record_repository::{
        RuntimeListQuery, RuntimeListResult, RuntimeRecordRepository, RuntimeSortInput,
    },
};
use serde_json::Value;
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

impl PgControlPlaneStore {
    pub async fn list_runtime_model_metadata(&self) -> Result<Vec<ModelMetadata>> {
        let models = ModelDefinitionRepository::list_model_definitions(self, Uuid::nil()).await?;
        let mut metadata = Vec::with_capacity(models.len());
        for model in models {
            if let Some(model) = self.refresh_runtime_model_health(model).await? {
                metadata.push(to_runtime_model_metadata(model));
            }
        }
        Ok(metadata)
    }

    async fn runtime_model_metadata_by_id(&self, model_id: Uuid) -> Result<Option<ModelMetadata>> {
        let model =
            ModelDefinitionRepository::get_model_definition(self, Uuid::nil(), model_id).await?;
        let Some(model) = model else {
            return Ok(None);
        };
        Ok(self
            .refresh_runtime_model_health(model)
            .await?
            .map(to_runtime_model_metadata))
    }

    async fn available_relation_target_metadata(
        &self,
        model_id: Uuid,
    ) -> Result<Option<ModelMetadata>> {
        let Some(metadata) = self.runtime_model_metadata_by_id(model_id).await? else {
            return Ok(None);
        };
        ensure_runtime_model_available(
            &metadata.model_code,
            RuntimeDataModelAvailability::from_status(metadata.status),
        )?;
        Ok(Some(metadata))
    }

    async fn refresh_runtime_model_health(
        &self,
        mut model: domain::ModelDefinitionRecord,
    ) -> Result<Option<domain::ModelDefinitionRecord>> {
        if model.source_kind == domain::DataModelSourceKind::ExternalSource {
            let model_metadata_available = model.data_source_instance_id.is_some()
                && required_text(model.external_resource_key.as_deref());
            let mut next_model_status = if model_metadata_available {
                domain::MetadataAvailabilityStatus::Available
            } else {
                domain::MetadataAvailabilityStatus::Unavailable
            };
            let mut fields = Vec::with_capacity(model.fields.len());
            for mut field in model.fields {
                let next_field_status = if !model_metadata_available
                    || (field_requires_external_mapping(&field)
                        && !required_text(field.external_field_key.as_deref()))
                {
                    next_model_status = domain::MetadataAvailabilityStatus::Unavailable;
                    domain::MetadataAvailabilityStatus::Unavailable
                } else {
                    domain::MetadataAvailabilityStatus::Available
                };
                if field.availability_status != next_field_status {
                    self.update_model_field_availability(field.id, next_field_status)
                        .await?;
                }
                field.availability_status = next_field_status;
                fields.push(field);
            }
            if model.availability_status != next_model_status {
                self.update_model_availability(model.id, next_model_status)
                    .await?;
            }
            model.availability_status = next_model_status;
            model.fields = fields;
            return Ok(model.availability_status.is_healthy().then_some(model));
        }

        let table_exists = self
            .runtime_table_exists(&model.physical_table_name)
            .await?;
        let mut next_model_status = domain::MetadataAvailabilityStatus::Available;
        let columns = if table_exists {
            self.runtime_table_columns(&model.physical_table_name)
                .await?
        } else {
            next_model_status = domain::MetadataAvailabilityStatus::Unavailable;
            HashSet::new()
        };
        let mut fields = Vec::with_capacity(model.fields.len());

        for mut field in model.fields {
            let next_field_status = if !table_exists {
                domain::MetadataAvailabilityStatus::Unavailable
            } else if field_requires_physical_column(&field)
                && !columns.contains(&field.physical_column_name)
            {
                next_model_status = domain::MetadataAvailabilityStatus::Unavailable;
                domain::MetadataAvailabilityStatus::Unavailable
            } else {
                domain::MetadataAvailabilityStatus::Available
            };
            if field.availability_status != next_field_status {
                self.update_model_field_availability(field.id, next_field_status)
                    .await?;
            }
            field.availability_status = next_field_status;
            fields.push(field);
        }

        if model.availability_status != next_model_status {
            self.update_model_availability(model.id, next_model_status)
                .await?;
        }
        model.availability_status = next_model_status;
        model.fields = fields;

        Ok(model.availability_status.is_healthy().then_some(model))
    }

    async fn runtime_table_exists(&self, table_name: &str) -> Result<bool> {
        sqlx::query_scalar(
            r#"
            select exists(
                select 1
                from information_schema.tables
                where table_schema = current_schema()
                  and table_name = $1
            )
            "#,
        )
        .bind(table_name)
        .fetch_one(self.pool())
        .await
        .map_err(Into::into)
    }

    async fn runtime_table_columns(&self, table_name: &str) -> Result<HashSet<String>> {
        let columns: Vec<String> = sqlx::query_scalar(
            r#"
            select column_name
            from information_schema.columns
            where table_schema = current_schema()
              and table_name = $1
            "#,
        )
        .bind(table_name)
        .fetch_all(self.pool())
        .await?;
        Ok(columns.into_iter().collect())
    }

    async fn update_model_availability(
        &self,
        model_id: Uuid,
        availability_status: domain::MetadataAvailabilityStatus,
    ) -> Result<()> {
        sqlx::query("update model_definitions set availability_status = $2, updated_at = now() where id = $1")
            .bind(model_id)
            .bind(availability_status.as_str())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn update_model_field_availability(
        &self,
        field_id: Uuid,
        availability_status: domain::MetadataAvailabilityStatus,
    ) -> Result<()> {
        sqlx::query(
            "update model_fields set availability_status = $2, updated_at = now() where id = $1",
        )
        .bind(field_id)
        .bind(availability_status.as_str())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn mark_runtime_model_unavailable(&self, metadata: &ModelMetadata) -> Result<()> {
        self.update_model_availability(
            metadata.model_id,
            domain::MetadataAvailabilityStatus::Unavailable,
        )
        .await?;
        sqlx::query(
            "update model_fields set availability_status = 'unavailable', updated_at = now() where data_model_id = $1",
        )
        .bind(metadata.model_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn map_runtime_storage_error(
        &self,
        metadata: &ModelMetadata,
        error: sqlx::Error,
    ) -> anyhow::Error {
        if is_runtime_object_missing_error(&error) {
            if let Err(mark_error) = self.mark_runtime_model_unavailable(metadata).await {
                return mark_error;
            }
            return runtime_core::runtime_engine::RuntimeModelError::unavailable(
                &metadata.model_code,
            )
            .into();
        }

        error.into()
    }

    async fn run_runtime_query<T, F>(&self, metadata: &ModelMetadata, query: F) -> Result<T>
    where
        F: Future<Output = std::result::Result<T, sqlx::Error>>,
    {
        match query.await {
            Ok(value) => Ok(value),
            Err(error) => Err(self.map_runtime_storage_error(metadata, error).await),
        }
    }
}

#[async_trait]
impl RuntimeRecordRepository for PgControlPlaneStore {
    async fn list_records(
        &self,
        metadata: &ModelMetadata,
        query: RuntimeListQuery,
    ) -> Result<RuntimeListResult> {
        let page = query.page.max(1);
        let page_size = query.page_size.max(1);
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let scope_column_name = quote_identifier(&metadata.scope_column_name)?;
        let offset = (page - 1) * page_size;

        let mut count_builder = QueryBuilder::<Postgres>::new(format!(
            "select count(*)::bigint from {table_name} where true"
        ));
        append_scope_clause(&mut count_builder, &scope_column_name, query.scope_id);
        append_owner_scope_clause(&mut count_builder, query.owner_user_id);
        append_filter_clause(&mut count_builder, metadata, &query.filter)?;
        let total = self
            .run_runtime_query(
                metadata,
                count_builder
                    .build_query_scalar::<i64>()
                    .fetch_one(self.pool()),
            )
            .await?;

        let mut list_builder = QueryBuilder::<Postgres>::new(format!(
            "select row_to_json(t) from (select {} from {table_name} where true",
            projected_select_list(metadata)?
        ));
        append_scope_clause(&mut list_builder, &scope_column_name, query.scope_id);
        append_owner_scope_clause(&mut list_builder, query.owner_user_id);
        append_filter_clause(&mut list_builder, metadata, &query.filter)?;
        append_sort_clause(&mut list_builder, metadata, &query.sorts)?;
        list_builder.push(" limit ");
        list_builder.push_bind(page_size);
        list_builder.push(" offset ");
        list_builder.push_bind(offset);
        list_builder.push(") t");

        let rows = self
            .run_runtime_query(
                metadata,
                list_builder
                    .build_query_scalar::<Value>()
                    .fetch_all(self.pool()),
            )
            .await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let normalized = normalize_record(metadata, row);
            items.push(
                self.expand_relations(
                    metadata,
                    query.scope_id,
                    query.owner_user_id,
                    normalized,
                    &query.expand_relations,
                )
                .await?,
            );
        }

        Ok(RuntimeListResult { items, total })
    }

    async fn get_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
    ) -> Result<Option<Value>> {
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let scope_column_name = quote_identifier(&metadata.scope_column_name)?;
        let record_id = parse_record_id(record_id)?;
        let mut builder = QueryBuilder::<Postgres>::new(format!(
            "select row_to_json(t) from (select {} from {table_name} where true",
            projected_select_list(metadata)?
        ));
        append_scope_clause(&mut builder, &scope_column_name, scope_id);
        append_owner_scope_clause(&mut builder, owner_user_id);
        builder.push(" and id = ");
        builder.push_bind(record_id);
        builder.push(" limit 1) t");

        let row = self
            .run_runtime_query(
                metadata,
                builder
                    .build_query_scalar::<Value>()
                    .fetch_optional(self.pool()),
            )
            .await?;

        Ok(row.map(|value| normalize_record(metadata, value)))
    }

    async fn create_record(
        &self,
        metadata: &ModelMetadata,
        actor_user_id: Uuid,
        scope_id: Uuid,
        payload: Value,
    ) -> Result<Value> {
        let payload = payload_object(payload)?;
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let generated_record_id = Uuid::now_v7();
        let actor_user_id = nullable_actor_user_id(actor_user_id);
        let mut declared_physical_columns = HashSet::with_capacity(payload.len());
        let mut payload_record_id = None;
        let mut payload_scope_id = None;
        let mut declared_fields = Vec::with_capacity(payload.len());
        for (field_code, value) in &payload {
            let field = metadata
                .field_by_code(field_code)
                .ok_or_else(|| anyhow!("undeclared field code: {field_code}"))?;
            if !declared_physical_columns.insert(field.physical_column_name.clone()) {
                return Err(anyhow!(
                    "duplicate physical field column: {}",
                    field.physical_column_name
                ));
            }
            if field.physical_column_name == "id" {
                payload_record_id = Some(json_uuid(value)?);
            }
            if field.physical_column_name == metadata.scope_column_name {
                payload_scope_id = Some(json_uuid(value)?);
            }
            declared_fields.push((field, value));
        }
        let record_id = payload_record_id.unwrap_or(generated_record_id);
        let uses_scope_column = metadata_uses_scope_column(metadata);
        let lookup_scope_id = uses_scope_column.then_some(payload_scope_id.unwrap_or(scope_id));
        let mut insert_columns = Vec::with_capacity(declared_fields.len() + 4);
        if !declared_physical_columns.contains("id") {
            insert_columns.push(RuntimeInsertColumn::GeneratedRecordId(generated_record_id));
        }
        if uses_scope_column && !declared_physical_columns.contains(&metadata.scope_column_name) {
            insert_columns.push(RuntimeInsertColumn::ScopeId {
                physical_column_name: metadata.scope_column_name.clone(),
                scope_id,
            });
        }
        if !declared_physical_columns.contains("created_by") {
            insert_columns.push(RuntimeInsertColumn::CreatedBy(actor_user_id));
        }
        if !declared_physical_columns.contains("updated_by") {
            insert_columns.push(RuntimeInsertColumn::UpdatedBy(actor_user_id));
        }
        insert_columns.extend(
            declared_fields
                .into_iter()
                .map(|(field, value)| RuntimeInsertColumn::PayloadField { field, value }),
        );

        let mut builder = QueryBuilder::<Postgres>::new(format!("insert into {table_name} ("));
        for (index, column) in insert_columns.iter().enumerate() {
            if index > 0 {
                builder.push(", ");
            }
            builder.push(quote_identifier(runtime_insert_column_name(column))?);
        }
        builder.push(") values (");
        for (index, column) in insert_columns.iter().enumerate() {
            if index > 0 {
                builder.push(", ");
            }
            push_runtime_insert_column_value(&mut builder, column)?;
        }
        builder.push(")");
        self.run_runtime_query(metadata, builder.build().execute(self.pool()))
            .await?;

        self.get_record(metadata, lookup_scope_id, None, &record_id.to_string())
            .await?
            .ok_or_else(|| anyhow!("runtime record not found after create"))
    }

    async fn update_record(
        &self,
        metadata: &ModelMetadata,
        actor_user_id: Uuid,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
        payload: Value,
    ) -> Result<Value> {
        let payload = payload_object(payload)?;
        if payload.is_empty() {
            return self
                .get_record(
                    metadata,
                    metadata_scope_filter(metadata, scope_id),
                    owner_user_id,
                    record_id,
                )
                .await?
                .ok_or_else(|| anyhow!("runtime record not found"));
        }

        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let scope_column_name = quote_identifier(&metadata.scope_column_name)?;
        let record_id = parse_record_id(record_id)?;
        let actor_user_id = nullable_actor_user_id(actor_user_id);
        let mut declared_physical_columns = HashSet::with_capacity(payload.len());
        let mut lookup_record_id = record_id;
        let mut payload_scope_id = None;
        let mut declared_fields = Vec::with_capacity(payload.len());
        for (field_code, value) in &payload {
            let field = metadata
                .field_by_code(field_code)
                .ok_or_else(|| anyhow!("undeclared field code: {field_code}"))?;
            if !declared_physical_columns.insert(field.physical_column_name.clone()) {
                return Err(anyhow!(
                    "duplicate physical field column: {}",
                    field.physical_column_name
                ));
            }
            if field.physical_column_name == "id" {
                lookup_record_id = json_uuid(value)?;
            }
            if field.physical_column_name == metadata.scope_column_name {
                payload_scope_id = Some(json_uuid(value)?);
            }
            declared_fields.push((field, value));
        }

        let mut builder = QueryBuilder::<Postgres>::new(format!("update {table_name} set "));
        let mut assignment_count = 0;
        if !declared_physical_columns.contains("updated_by") {
            builder.push("updated_by = ");
            builder.push_bind(actor_user_id);
            assignment_count += 1;
        }
        if !declared_physical_columns.contains("updated_at") {
            if assignment_count > 0 {
                builder.push(", ");
            }
            builder.push("updated_at = now()");
            assignment_count += 1;
        }
        for (field, value) in declared_fields {
            if assignment_count > 0 {
                builder.push(", ");
            }
            builder.push(quote_identifier(&field.physical_column_name)?);
            builder.push(" = ");
            push_field_value(&mut builder, field, value)?;
            assignment_count += 1;
        }
        builder.push(" where true");
        append_scope_clause(
            &mut builder,
            &scope_column_name,
            metadata_scope_filter(metadata, scope_id),
        );
        append_owner_scope_clause(&mut builder, owner_user_id);
        builder.push(" and id = ");
        builder.push_bind(record_id);
        self.run_runtime_query(metadata, builder.build().execute(self.pool()))
            .await?;

        let lookup_scope_id =
            payload_scope_id.or_else(|| metadata_scope_filter(metadata, scope_id));
        self.get_record(
            metadata,
            lookup_scope_id,
            owner_user_id,
            &lookup_record_id.to_string(),
        )
        .await?
        .ok_or_else(|| anyhow!("runtime record not found after update"))
    }

    async fn delete_record(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record_id: &str,
    ) -> Result<bool> {
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let scope_column_name = quote_identifier(&metadata.scope_column_name)?;
        let record_id = parse_record_id(record_id)?;
        let mut builder =
            QueryBuilder::<Postgres>::new(format!("delete from {table_name} where true"));
        append_scope_clause(
            &mut builder,
            &scope_column_name,
            metadata_scope_filter(metadata, scope_id),
        );
        append_owner_scope_clause(&mut builder, owner_user_id);
        builder.push(" and id = ");
        builder.push_bind(record_id);
        let result = self
            .run_runtime_query(metadata, builder.build().execute(self.pool()))
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl PgControlPlaneStore {
    async fn expand_relations(
        &self,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        record: Value,
        expand_relations: &[String],
    ) -> Result<Value> {
        if expand_relations.is_empty() {
            return Ok(record);
        }

        let mut object = match record {
            Value::Object(object) => object,
            other => return Ok(other),
        };

        for relation_code in expand_relations {
            let field = metadata
                .field_by_code(relation_code)
                .ok_or_else(|| anyhow!("undeclared relation code: {relation_code}"))?;
            match field.field_kind {
                domain::ModelFieldKind::ManyToOne => {
                    let Some(target_model_id) = field.relation_target_model_id else {
                        continue;
                    };
                    let Some(target_record_id) = object.get(&field.code).and_then(Value::as_str)
                    else {
                        continue;
                    };
                    let Some(target_metadata) = self
                        .available_relation_target_metadata(target_model_id)
                        .await?
                    else {
                        continue;
                    };
                    let expanded = RuntimeRecordRepository::get_record(
                        self,
                        &target_metadata,
                        scope_id,
                        owner_user_id,
                        target_record_id,
                    )
                    .await?
                    .unwrap_or(Value::Null);
                    object.insert(field.code.clone(), expanded);
                }
                domain::ModelFieldKind::OneToMany => {
                    let Some(target_model_id) = field.relation_target_model_id else {
                        continue;
                    };
                    let Some(target_metadata) = self
                        .available_relation_target_metadata(target_model_id)
                        .await?
                    else {
                        continue;
                    };
                    let Some(mapped_by) = field
                        .relation_options
                        .get("mapped_by")
                        .and_then(Value::as_str)
                    else {
                        continue;
                    };
                    let Some(record_id) = object.get("id").cloned() else {
                        continue;
                    };
                    let expanded = RuntimeRecordRepository::list_records(
                        self,
                        &target_metadata,
                        RuntimeListQuery {
                            scope_id,
                            owner_user_id,
                            filter: domain::ResourceFilterExpr::Field {
                                field: mapped_by.to_string(),
                                operator: domain::ResourceFilterOperator::Eq,
                                value: record_id,
                            },
                            sorts: vec![],
                            expand_relations: vec![],
                            page: 1,
                            page_size: 100,
                        },
                    )
                    .await?;
                    object.insert(field.code.clone(), Value::Array(expanded.items));
                }
                _ => return Err(anyhow!("unsupported relation expansion")),
            }
        }

        Ok(Value::Object(object))
    }
}

fn append_owner_scope_clause(
    builder: &mut QueryBuilder<'_, Postgres>,
    owner_user_id: Option<Uuid>,
) {
    if let Some(owner_user_id) = owner_user_id {
        builder.push(" and created_by = ");
        builder.push_bind(owner_user_id);
    }
}

fn append_scope_clause(
    builder: &mut QueryBuilder<'_, Postgres>,
    scope_column_name: &str,
    scope_id: Option<Uuid>,
) {
    if let Some(scope_id) = scope_id {
        builder.push(" and ");
        builder.push(scope_column_name);
        builder.push(" = ");
        builder.push_bind(scope_id);
    }
}

fn to_runtime_model_metadata(model: domain::ModelDefinitionRecord) -> ModelMetadata {
    let record_capabilities = domain::data_model_capabilities(&model).record;
    ModelMetadata {
        model_id: model.id,
        model_code: model.code.clone(),
        status: model.status,
        scope_kind: model.scope_kind,
        scope_id: model.scope_id,
        data_source_instance_id: model.data_source_instance_id,
        source_kind: model.source_kind,
        external_resource_key: model.external_resource_key,
        physical_table_name: model.physical_table_name,
        scope_column_name: "scope_id".into(),
        fields: model
            .fields
            .into_iter()
            .filter(|field| field.availability_status.is_healthy())
            .collect(),
        record_capabilities,
        resource: runtime_core::resource_descriptor::ResourceDescriptor::runtime_model(
            &model.code,
            model.scope_kind,
        ),
    }
}

fn append_filter_clause(
    builder: &mut QueryBuilder<Postgres>,
    metadata: &ModelMetadata,
    filter: &domain::ResourceFilterExpr,
) -> Result<()> {
    if matches!(filter, domain::ResourceFilterExpr::All(items) if items.is_empty()) {
        return Ok(());
    }

    builder.push(" and ");
    append_filter_expr(builder, metadata, filter)
}

fn append_filter_expr(
    builder: &mut QueryBuilder<Postgres>,
    metadata: &ModelMetadata,
    filter: &domain::ResourceFilterExpr,
) -> Result<()> {
    match filter {
        domain::ResourceFilterExpr::All(items) => {
            if items.is_empty() {
                builder.push("true");
                return Ok(());
            }
            builder.push("(");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    builder.push(" and ");
                }
                append_filter_expr(builder, metadata, item)?;
            }
            builder.push(")");
        }
        domain::ResourceFilterExpr::Any(items) => {
            if items.is_empty() {
                builder.push("false");
                return Ok(());
            }
            builder.push("(");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    builder.push(" or ");
                }
                append_filter_expr(builder, metadata, item)?;
            }
            builder.push(")");
        }
        domain::ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => {
            let field_record = metadata
                .field_by_code(field)
                .ok_or_else(|| anyhow!("undeclared field code: {}", field))?;
            append_field_filter_expr(builder, field_record, *operator, value)?;
        }
    }

    Ok(())
}

fn append_field_filter_expr(
    builder: &mut QueryBuilder<Postgres>,
    field: &domain::ModelFieldRecord,
    operator: domain::ResourceFilterOperator,
    value: &Value,
) -> Result<()> {
    if operator == domain::ResourceFilterOperator::In {
        let Some(values) = value.as_array() else {
            return Err(anyhow!("filter $in value must be an array"));
        };
        if values.is_empty() {
            builder.push("false");
            return Ok(());
        }
        builder.push(quote_identifier(&field.physical_column_name)?);
        builder.push(" in (");
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                builder.push(", ");
            }
            push_field_value(builder, field, value)?;
        }
        builder.push(")");
        return Ok(());
    }

    builder.push(quote_identifier(&field.physical_column_name)?);
    builder.push(" ");
    builder.push(filter_operator_sql(operator)?);
    builder.push(" ");
    if matches!(
        operator,
        domain::ResourceFilterOperator::Includes | domain::ResourceFilterOperator::NotIncludes
    ) {
        push_string_pattern_value(builder, field, value)?;
    } else {
        push_field_value(builder, field, value)?;
    }
    Ok(())
}

fn append_sort_clause(
    builder: &mut QueryBuilder<Postgres>,
    metadata: &ModelMetadata,
    sorts: &[RuntimeSortInput],
) -> Result<()> {
    if sorts.is_empty() {
        return append_default_sort_clause(builder, metadata);
    }

    builder.push(" order by ");
    for (index, sort) in sorts.iter().enumerate() {
        if index > 0 {
            builder.push(", ");
        }
        let field = metadata
            .field_by_code(&sort.field_code)
            .ok_or_else(|| anyhow!("undeclared sort field: {}", sort.field_code))?;
        builder.push(quote_identifier(&field.physical_column_name)?);
        builder.push(" ");
        builder.push(sort_direction_sql(&sort.direction)?);
    }

    Ok(())
}

fn append_default_sort_clause(
    builder: &mut QueryBuilder<Postgres>,
    metadata: &ModelMetadata,
) -> Result<()> {
    let Some(created_at) = metadata.field_by_code("created_at") else {
        builder.push(" order by ");
        if let Some(id) = metadata.field_by_code("id") {
            builder.push(quote_identifier(&id.physical_column_name)?);
            builder.push(" desc");
        } else {
            builder.push("1 asc");
        }
        return Ok(());
    };

    builder.push(" order by ");
    builder.push(quote_identifier(&created_at.physical_column_name)?);
    builder.push(" desc");
    if let Some(id) = metadata.field_by_code("id") {
        builder.push(", ");
        builder.push(quote_identifier(&id.physical_column_name)?);
        builder.push(" desc");
    }
    Ok(())
}

fn projected_select_list(metadata: &ModelMetadata) -> Result<String> {
    if metadata.fields.is_empty() {
        return Ok("*".to_string());
    }

    let mut seen = HashSet::new();
    let mut columns = Vec::new();
    for field in &metadata.fields {
        if !field_requires_physical_column(field) {
            continue;
        }
        if seen.insert(field.physical_column_name.clone()) {
            columns.push(quote_identifier(&field.physical_column_name)?);
        }
    }

    if columns.is_empty() {
        return Ok("*".to_string());
    }

    Ok(columns.join(", "))
}

enum RuntimeInsertColumn<'a> {
    GeneratedRecordId(Uuid),
    ScopeId {
        physical_column_name: String,
        scope_id: Uuid,
    },
    CreatedBy(Option<Uuid>),
    UpdatedBy(Option<Uuid>),
    PayloadField {
        field: &'a domain::ModelFieldRecord,
        value: &'a Value,
    },
}

fn runtime_insert_column_name<'a>(column: &'a RuntimeInsertColumn<'a>) -> &'a str {
    match column {
        RuntimeInsertColumn::GeneratedRecordId(_) => "id",
        RuntimeInsertColumn::ScopeId {
            physical_column_name,
            ..
        } => physical_column_name.as_str(),
        RuntimeInsertColumn::CreatedBy(_) => "created_by",
        RuntimeInsertColumn::UpdatedBy(_) => "updated_by",
        RuntimeInsertColumn::PayloadField { field, .. } => &field.physical_column_name,
    }
}

fn push_runtime_insert_column_value(
    builder: &mut QueryBuilder<Postgres>,
    column: &RuntimeInsertColumn<'_>,
) -> Result<()> {
    match column {
        RuntimeInsertColumn::GeneratedRecordId(record_id) => {
            builder.push_bind(*record_id);
        }
        RuntimeInsertColumn::ScopeId { scope_id, .. } => {
            builder.push_bind(*scope_id);
        }
        RuntimeInsertColumn::CreatedBy(actor_user_id)
        | RuntimeInsertColumn::UpdatedBy(actor_user_id) => {
            builder.push_bind(*actor_user_id);
        }
        RuntimeInsertColumn::PayloadField { field, value } => {
            push_field_value(builder, field, value)?;
        }
    };
    Ok(())
}

fn metadata_uses_scope_column(metadata: &ModelMetadata) -> bool {
    metadata
        .fields
        .iter()
        .any(|field| field.physical_column_name == metadata.scope_column_name)
}

fn metadata_scope_filter(metadata: &ModelMetadata, scope_id: Option<Uuid>) -> Option<Uuid> {
    metadata_uses_scope_column(metadata)
        .then_some(scope_id)
        .flatten()
}

fn push_field_value(
    builder: &mut QueryBuilder<Postgres>,
    field: &domain::ModelFieldRecord,
    value: &Value,
) -> Result<()> {
    if field.physical_column_name == "id" {
        builder.push_bind(json_uuid(value)?);
        return Ok(());
    }
    if matches!(
        field.physical_column_name.as_str(),
        "created_by" | "updated_by"
    ) {
        builder.push_bind(json_nullable_uuid(value)?);
        return Ok(());
    }

    match field.field_kind {
        domain::ModelFieldKind::String
        | domain::ModelFieldKind::Enum
        | domain::ModelFieldKind::Text => builder.push_bind(json_string(value)?),
        domain::ModelFieldKind::Datetime => builder.push_bind(json_datetime(value)?),
        domain::ModelFieldKind::Number => builder.push_bind(json_number(value)?),
        domain::ModelFieldKind::Boolean => builder.push_bind(json_bool(value)?),
        domain::ModelFieldKind::Json => builder.push_bind(value.clone()),
        domain::ModelFieldKind::ManyToOne => builder.push_bind(json_nullable_uuid(value)?),
        domain::ModelFieldKind::OneToMany => {
            return Err(anyhow!("one_to_many cannot be persisted directly"))
        }
        domain::ModelFieldKind::ManyToMany => {
            return Err(anyhow!("many_to_many cannot be persisted directly"))
        }
    };

    Ok(())
}

fn push_string_pattern_value(
    builder: &mut QueryBuilder<Postgres>,
    field: &domain::ModelFieldRecord,
    value: &Value,
) -> Result<()> {
    match field.field_kind {
        domain::ModelFieldKind::String
        | domain::ModelFieldKind::Enum
        | domain::ModelFieldKind::Text => {
            builder.push_bind(format!("%{}%", json_string(value)?));
            Ok(())
        }
        _ => Err(anyhow!(
            "$includes filter only supports string, text, or enum fields"
        )),
    }
}

fn normalize_record(metadata: &ModelMetadata, value: Value) -> Value {
    let Value::Object(mut object) = value else {
        return value;
    };
    let exposes_scope_column = metadata
        .fields
        .iter()
        .any(|field| field.code == metadata.scope_column_name);
    if !exposes_scope_column {
        object.remove(&metadata.scope_column_name);
    }
    for field in &metadata.fields {
        if field.code != field.physical_column_name {
            if let Some(field_value) = object.remove(&field.physical_column_name) {
                object.insert(field.code.clone(), field_value);
            }
        }
    }

    Value::Object(object)
}

fn payload_object(payload: Value) -> Result<serde_json::Map<String, Value>> {
    match payload {
        Value::Object(map) => Ok(map),
        _ => Err(anyhow!("runtime payload must be object")),
    }
}

fn filter_operator_sql(operator: domain::ResourceFilterOperator) -> Result<&'static str> {
    match operator {
        domain::ResourceFilterOperator::Eq => Ok("="),
        domain::ResourceFilterOperator::Ne => Ok("<>"),
        domain::ResourceFilterOperator::Gt => Ok(">"),
        domain::ResourceFilterOperator::Gte => Ok(">="),
        domain::ResourceFilterOperator::Lt => Ok("<"),
        domain::ResourceFilterOperator::Lte => Ok("<="),
        domain::ResourceFilterOperator::Includes => Ok("ilike"),
        domain::ResourceFilterOperator::NotIncludes => Ok("not ilike"),
        domain::ResourceFilterOperator::In => Err(anyhow!("$in is handled separately")),
    }
}

fn sort_direction_sql(direction: &str) -> Result<&'static str> {
    match direction.to_ascii_lowercase().as_str() {
        "asc" => Ok("asc"),
        "desc" => Ok("desc"),
        _ => Err(anyhow!("unsupported sort direction")),
    }
}

fn quote_identifier(value: &str) -> Result<String> {
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(anyhow!("invalid sql identifier"));
    }

    Ok(format!("\"{value}\""))
}

fn parse_record_id(record_id: &str) -> Result<Uuid> {
    Uuid::parse_str(record_id).map_err(Into::into)
}

fn json_string(value: &Value) -> Result<String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("expected string value"))
}

fn json_number(value: &Value) -> Result<f64> {
    value
        .as_f64()
        .ok_or_else(|| anyhow!("expected numeric value"))
}

fn json_bool(value: &Value) -> Result<bool> {
    value
        .as_bool()
        .ok_or_else(|| anyhow!("expected boolean value"))
}

fn json_datetime(value: &Value) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(&json_string(value)?, &Rfc3339).map_err(Into::into)
}

fn json_uuid(value: &Value) -> Result<Uuid> {
    parse_record_id(
        value
            .as_str()
            .ok_or_else(|| anyhow!("expected uuid string value"))?,
    )
}

fn json_nullable_uuid(value: &Value) -> Result<Option<Uuid>> {
    if value.is_null() {
        return Ok(None);
    }
    json_uuid(value).map(Some)
}

fn nullable_actor_user_id(actor_user_id: Uuid) -> Option<Uuid> {
    (!actor_user_id.is_nil()).then_some(actor_user_id)
}

fn field_requires_physical_column(field: &domain::ModelFieldRecord) -> bool {
    !matches!(
        field.field_kind,
        domain::ModelFieldKind::OneToMany | domain::ModelFieldKind::ManyToMany
    )
}

fn field_requires_external_mapping(field: &domain::ModelFieldRecord) -> bool {
    field_requires_physical_column(field)
}

fn required_text(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

fn is_runtime_object_missing_error(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database_error) => {
            matches!(database_error.code().as_deref(), Some("42P01" | "42703"))
        }
        _ => false,
    }
}

#[allow(dead_code)]
fn to_model_field_record(row: PgRow) -> domain::ModelFieldRecord {
    domain::ModelFieldRecord {
        id: row.get("id"),
        data_model_id: row.get("data_model_id"),
        code: row.get("code"),
        title: row.get("title"),
        description: row.get("description"),
        physical_column_name: row.get("physical_column_name"),
        external_field_key: row.get("external_field_key"),
        field_kind: domain::ModelFieldKind::from_db(row.get("field_kind")),
        is_system: row.get("is_system"),
        is_writable: row.get("is_writable"),
        is_required: row.get("is_required"),
        is_unique: row.get("is_unique"),
        default_value: row.get("default_value"),
        display_interface: row.get("display_interface"),
        display_options: row.get("display_options"),
        relation_target_model_id: row.get("relation_target_model_id"),
        relation_options: row.get("relation_options"),
        sort_order: row.get("sort_order"),
        availability_status: domain::MetadataAvailabilityStatus::from_db(
            row.get("availability_status"),
        ),
    }
}
