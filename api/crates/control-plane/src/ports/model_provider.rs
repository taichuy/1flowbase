use super::*;

#[derive(Debug, Clone)]
pub struct CreateModelProviderInstanceInput {
    pub instance_id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Uuid,
    pub provider_code: String,
    pub protocol: String,
    pub display_name: String,
    pub status: domain::ModelProviderInstanceStatus,
    pub config_json: serde_json::Value,
    pub configured_models: Vec<domain::ModelProviderConfiguredModel>,
    pub enabled_model_ids: Vec<String>,
    pub included_in_main: Option<bool>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateModelProviderInstanceInput {
    pub instance_id: Uuid,
    pub workspace_id: Uuid,
    pub display_name: String,
    pub status: domain::ModelProviderInstanceStatus,
    pub config_json: serde_json::Value,
    pub configured_models: Vec<domain::ModelProviderConfiguredModel>,
    pub enabled_model_ids: Vec<String>,
    pub included_in_main: bool,
    pub updated_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateModelProviderPreviewSessionInput {
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub instance_id: Option<Uuid>,
    pub config_fingerprint: String,
    pub models_json: serde_json::Value,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct UpsertModelProviderCatalogCacheInput {
    pub provider_instance_id: Uuid,
    pub model_discovery_mode: domain::ModelProviderDiscoveryMode,
    pub refresh_status: domain::ModelProviderCatalogRefreshStatus,
    pub source: domain::ModelProviderCatalogSource,
    pub models_json: serde_json::Value,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct CreateModelProviderCatalogSourceInput {
    pub source_id: Uuid,
    pub workspace_id: Uuid,
    pub source_kind: String,
    pub plugin_id: String,
    pub provider_code: String,
    pub display_name: String,
    pub base_url_ref: Option<String>,
    pub auth_secret_ref: Option<String>,
    pub protocol: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct CreateModelCatalogSyncRunInput {
    pub sync_run_id: Uuid,
    pub catalog_source_id: Uuid,
    pub status: String,
    pub error_message_ref: Option<String>,
    pub discovered_count: i64,
    pub imported_count: i64,
    pub disabled_count: i64,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct UpsertModelProviderCatalogEntryInput {
    pub provider_instance_id: Option<Uuid>,
    pub catalog_source_id: Uuid,
    pub upstream_model_id: String,
    pub display_label: String,
    pub protocol: String,
    pub capability_snapshot: serde_json::Value,
    pub parameter_schema_ref: Option<String>,
    pub context_window: Option<i64>,
    pub max_output_tokens: Option<i64>,
    pub pricing_ref: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct CreateModelFailoverQueueTemplateInput {
    pub queue_template_id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub created_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateModelFailoverQueueItemInput {
    pub queue_item_id: Uuid,
    pub queue_template_id: Uuid,
    pub sort_index: i32,
    pub provider_instance_id: Uuid,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub protocol: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct CreateModelFailoverQueueSnapshotInput {
    pub snapshot_id: Uuid,
    pub queue_template_id: Uuid,
    pub version: i64,
    pub items: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ReassignModelProviderInstancesInput {
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub target_installation_id: Uuid,
    pub target_protocol: String,
    pub updated_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpsertModelProviderSecretInput {
    pub provider_instance_id: Uuid,
    pub plaintext_secret_json: serde_json::Value,
    pub secret_version: i32,
    pub master_key: String,
}

#[derive(Debug, Clone)]
pub struct UpsertModelProviderMainInstanceInput {
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub model_distribution_rules: Option<Vec<domain::ModelProviderMainModelDistributionRule>>,
    pub updated_by: Uuid,
}

#[async_trait]
pub trait ModelProviderRepository: Send + Sync {
    async fn create_instance(
        &self,
        input: &CreateModelProviderInstanceInput,
    ) -> anyhow::Result<domain::ModelProviderInstanceRecord>;
    async fn update_instance(
        &self,
        input: &UpdateModelProviderInstanceInput,
    ) -> anyhow::Result<domain::ModelProviderInstanceRecord>;
    async fn get_instance(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<domain::ModelProviderInstanceRecord>>;
    async fn list_instances(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ModelProviderInstanceRecord>>;
    async fn list_instances_by_provider_code(
        &self,
        provider_code: &str,
    ) -> anyhow::Result<Vec<domain::ModelProviderInstanceRecord>>;
    async fn reassign_instances_to_installation(
        &self,
        input: &ReassignModelProviderInstancesInput,
    ) -> anyhow::Result<Vec<domain::ModelProviderInstanceRecord>>;
    async fn upsert_catalog_cache(
        &self,
        input: &UpsertModelProviderCatalogCacheInput,
    ) -> anyhow::Result<domain::ModelProviderCatalogCacheRecord>;
    async fn get_catalog_cache(
        &self,
        provider_instance_id: Uuid,
    ) -> anyhow::Result<Option<domain::ModelProviderCatalogCacheRecord>>;
    async fn upsert_secret(
        &self,
        input: &UpsertModelProviderSecretInput,
    ) -> anyhow::Result<domain::ModelProviderSecretRecord>;
    async fn upsert_main_instance(
        &self,
        input: &UpsertModelProviderMainInstanceInput,
    ) -> anyhow::Result<domain::ModelProviderMainInstanceRecord>;
    async fn get_main_instance(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
    ) -> anyhow::Result<Option<domain::ModelProviderMainInstanceRecord>>;
    async fn create_preview_session(
        &self,
        input: &CreateModelProviderPreviewSessionInput,
    ) -> anyhow::Result<domain::ModelProviderPreviewSessionRecord>;
    async fn get_preview_session(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<Option<domain::ModelProviderPreviewSessionRecord>>;
    async fn delete_preview_session(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn get_secret_json(
        &self,
        provider_instance_id: Uuid,
        master_key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>>;
    async fn get_secret_record(
        &self,
        provider_instance_id: Uuid,
    ) -> anyhow::Result<Option<domain::ModelProviderSecretRecord>>;
    async fn delete_instance(&self, workspace_id: Uuid, instance_id: Uuid) -> anyhow::Result<()>;
    async fn count_instance_references(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> anyhow::Result<u64>;
    async fn create_catalog_source(
        &self,
        _input: &CreateModelProviderCatalogSourceInput,
    ) -> anyhow::Result<domain::ModelProviderCatalogSourceRecord> {
        Err(anyhow::anyhow!(
            "model provider catalog sources are not supported by this repository"
        ))
    }
    async fn create_catalog_sync_run(
        &self,
        _input: &CreateModelCatalogSyncRunInput,
    ) -> anyhow::Result<domain::ModelCatalogSyncRunRecord> {
        Err(anyhow::anyhow!(
            "model catalog sync runs are not supported by this repository"
        ))
    }
    async fn upsert_catalog_entry(
        &self,
        _input: &UpsertModelProviderCatalogEntryInput,
    ) -> anyhow::Result<domain::ModelProviderCatalogEntryRecord> {
        Err(anyhow::anyhow!(
            "model provider catalog entries are not supported by this repository"
        ))
    }
    async fn list_catalog_entries(
        &self,
        _catalog_source_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ModelProviderCatalogEntryRecord>> {
        Err(anyhow::anyhow!(
            "model provider catalog entries are not supported by this repository"
        ))
    }
    async fn list_catalog_entries_for_provider_instance(
        &self,
        _provider_instance_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ModelProviderCatalogEntryRecord>> {
        Err(anyhow::anyhow!(
            "model provider catalog entries are not supported by this repository"
        ))
    }
    async fn create_failover_queue_template(
        &self,
        _input: &CreateModelFailoverQueueTemplateInput,
    ) -> anyhow::Result<domain::ModelFailoverQueueTemplateRecord> {
        Err(anyhow::anyhow!(
            "model failover queue templates are not supported by this repository"
        ))
    }
    async fn get_failover_queue_template(
        &self,
        _queue_template_id: Uuid,
    ) -> anyhow::Result<Option<domain::ModelFailoverQueueTemplateRecord>> {
        Err(anyhow::anyhow!(
            "model failover queue templates are not supported by this repository"
        ))
    }
    async fn create_failover_queue_item(
        &self,
        _input: &CreateModelFailoverQueueItemInput,
    ) -> anyhow::Result<domain::ModelFailoverQueueItemRecord> {
        Err(anyhow::anyhow!(
            "model failover queue items are not supported by this repository"
        ))
    }
    async fn list_failover_queue_items(
        &self,
        _queue_template_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ModelFailoverQueueItemRecord>> {
        Err(anyhow::anyhow!(
            "model failover queue items are not supported by this repository"
        ))
    }
    async fn create_failover_queue_snapshot(
        &self,
        _input: &CreateModelFailoverQueueSnapshotInput,
    ) -> anyhow::Result<domain::ModelFailoverQueueSnapshotRecord> {
        Err(anyhow::anyhow!(
            "model failover queue snapshots are not supported by this repository"
        ))
    }
    async fn list_failover_queue_snapshots(
        &self,
        _queue_template_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ModelFailoverQueueSnapshotRecord>> {
        Err(anyhow::anyhow!(
            "model failover queue snapshots are not supported by this repository"
        ))
    }
}
