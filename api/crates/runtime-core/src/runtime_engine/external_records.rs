use super::*;

impl RuntimeEngine {
    pub(super) async fn list_external_records(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        query: RuntimeListQuery,
    ) -> Result<RuntimeListResult> {
        let output = self
            .external_backend()?
            .list_records(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceListRecordsInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    context: data_source_context(query.scope_id, query.owner_user_id),
                    filters: external_filters(metadata, &query.filter)?,
                    sort: external_sorts(metadata, query.sorts)?,
                    page: Some(data_source_page(query.page, query.page_size)),
                    options_json: serde_json::json!({
                        "expand_relations": query.expand_relations
                    }),
                },
            )
            .await?;

        Ok(RuntimeListResult {
            total: output.total_count.unwrap_or(output.rows.len() as u64) as i64,
            items: output
                .rows
                .into_iter()
                .map(|record| external_record_to_runtime(metadata, record))
                .collect(),
        })
    }

    pub(super) async fn get_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        access_scope: crate::runtime_acl::RuntimeAccessScope,
        record_id: String,
    ) -> Result<Option<Value>> {
        self.external_backend()?
            .get_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceGetRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record_id,
                    context: data_source_context(access_scope.scope_id, access_scope.owner_user_id),
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| {
                output
                    .record
                    .map(|record| external_record_to_runtime(metadata, record))
            })
    }

    pub(super) async fn create_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        scope_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        payload: Value,
    ) -> Result<Value> {
        let record = runtime_payload_to_external(metadata, payload)?;
        self.external_backend()?
            .create_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceCreateRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record,
                    context: data_source_context(scope_id, owner_user_id),
                    transaction_id: None,
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| external_record_to_runtime(metadata, output.record))
    }

    pub(super) async fn update_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        access_scope: crate::runtime_acl::RuntimeAccessScope,
        record_id: String,
        payload: Value,
    ) -> Result<Value> {
        let patch = runtime_payload_to_external(metadata, payload)?;
        self.external_backend()?
            .update_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceUpdateRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record_id,
                    patch,
                    context: data_source_context(access_scope.scope_id, access_scope.owner_user_id),
                    transaction_id: None,
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| external_record_to_runtime(metadata, output.record))
    }

    pub(super) async fn delete_external_record(
        &self,
        workspace_id: Uuid,
        metadata: &ModelMetadata,
        access_scope: crate::runtime_acl::RuntimeAccessScope,
        record_id: String,
    ) -> Result<bool> {
        self.external_backend()?
            .delete_record(
                workspace_id,
                external_data_source_instance_id(metadata)?,
                DataSourceDeleteRecordInput {
                    connection: DataSourceConfigInput::default(),
                    resource_key: external_resource_key(metadata)?,
                    record_id,
                    context: data_source_context(access_scope.scope_id, access_scope.owner_user_id),
                    transaction_id: None,
                    options_json: Value::Object(Default::default()),
                },
            )
            .await
            .map(|output| output.deleted)
    }

    pub(super) fn external_backend(&self) -> Result<&Arc<dyn DataSourceRuntimeRecordBackend>> {
        self.data_source_records
            .as_ref()
            .ok_or_else(|| anyhow!("external data source runtime backend is not configured"))
    }
}
