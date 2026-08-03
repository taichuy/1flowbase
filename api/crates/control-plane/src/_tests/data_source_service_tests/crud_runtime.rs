use super::*;

#[tokio::test]
async fn map_resource_to_model_rejects_foreign_instance_workspace() {
    let repository = InMemoryDataSourceRepository::default();
    let service = data_source_service(
        repository.clone(),
        StubDataSourceRuntime::ready(),
        "test-master-key",
    );
    let created = service
        .create_instance(CreateDataSourceInstanceCommand {
            actor_user_id: user_id(),
            workspace_id: workspace_id(),
            installation_id: installation_id(),
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            config_json: json!({ "client_id": "abc" }),
            secret_json: json!({}),
        })
        .await
        .unwrap();

    let error = service
        .map_resource_to_model(MapDataSourceResourceToModelCommand {
            actor_user_id: user_id(),
            workspace_id: Uuid::from_u128(0x999),
            instance_id: created.instance.id,
            resource_key: "contacts".into(),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("workspace_id"));
    assert!(repository.mapped_models().await.is_empty());
}

#[async_trait]
impl DataSourceCrudRuntimePort for StubDataSourceRuntime {
    async fn list_records(
        &self,
        _installation: &LocalPluginInstallationRecord,
        input: DataSourceListRecordsInput,
    ) -> Result<DataSourceListRecordsOutput> {
        assert_eq!(input.resource_key, "contacts");
        assert_eq!(input.context.owner_id.as_deref(), Some("user-1"));
        assert_eq!(input.context.scope_id.as_deref(), Some("workspace-1"));
        Ok(DataSourceListRecordsOutput {
            rows: vec![json!({ "id": "contact-1" })],
            next_cursor: Some("next".to_string()),
            total_count: Some(1),
            metadata: json!({}),
        })
    }

    async fn get_record(
        &self,
        _installation: &LocalPluginInstallationRecord,
        input: DataSourceGetRecordInput,
    ) -> Result<DataSourceGetRecordOutput> {
        Ok(DataSourceGetRecordOutput {
            record: Some(json!({ "id": input.record_id })),
            metadata: json!({}),
        })
    }

    async fn create_record(
        &self,
        _installation: &LocalPluginInstallationRecord,
        input: DataSourceCreateRecordInput,
    ) -> Result<DataSourceCreateRecordOutput> {
        Ok(DataSourceCreateRecordOutput {
            record: input.record,
            metadata: json!({}),
        })
    }

    async fn update_record(
        &self,
        _installation: &LocalPluginInstallationRecord,
        input: DataSourceUpdateRecordInput,
    ) -> Result<DataSourceUpdateRecordOutput> {
        Ok(DataSourceUpdateRecordOutput {
            record: input.patch,
            metadata: json!({}),
        })
    }

    async fn delete_record(
        &self,
        _installation: &LocalPluginInstallationRecord,
        input: DataSourceDeleteRecordInput,
    ) -> Result<DataSourceDeleteRecordOutput> {
        assert_eq!(input.record_id, "contact-1");
        assert_eq!(input.transaction_id.as_deref(), Some("tx-1"));
        Ok(DataSourceDeleteRecordOutput {
            deleted: true,
            metadata: json!({}),
        })
    }
}

#[tokio::test]
async fn data_source_crud_runtime_port_exposes_owner_scope_aware_crud_contract() {
    let port = StubDataSourceRuntime::ready();
    let installation = LocalPluginInstallationRecord {
        installation: seeded_installation(),
        artifact: seeded_artifact(),
    };
    let context = DataSourceRecordScopeContext {
        owner_id: Some("user-1".to_string()),
        scope_id: Some("workspace-1".to_string()),
    };

    let list = port
        .list_records(
            &installation,
            DataSourceListRecordsInput {
                connection: Default::default(),
                resource_key: "contacts".to_string(),
                context: context.clone(),
                filters: Vec::new(),
                sort: Vec::new(),
                page: None,
                options_json: json!({}),
            },
        )
        .await
        .unwrap();
    let delete = port
        .delete_record(
            &installation,
            DataSourceDeleteRecordInput {
                connection: Default::default(),
                resource_key: "contacts".to_string(),
                record_id: "contact-1".to_string(),
                context,
                transaction_id: Some("tx-1".to_string()),
                options_json: json!({}),
            },
        )
        .await
        .unwrap();

    assert_eq!(list.total_count, Some(1));
    assert!(delete.deleted);
}
