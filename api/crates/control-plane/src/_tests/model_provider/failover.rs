use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    model_provider::{catalog_source::sync_relay_catalog_models, failover_queue},
    ports::{
        CreateModelCatalogSyncRunInput, CreateModelFailoverQueueItemInput,
        CreateModelFailoverQueueSnapshotInput, CreateModelFailoverQueueTemplateInput,
        CreateModelProviderCatalogSourceInput, CreateModelProviderInstanceInput,
        CreateModelProviderPreviewSessionInput, ModelProviderRepository,
        ReassignModelProviderInstancesInput, UpdateModelProviderInstanceInput,
        UpsertModelProviderCatalogCacheInput, UpsertModelProviderCatalogEntryInput,
        UpsertModelProviderMainInstanceInput, UpsertModelProviderSecretInput,
    },
};

type SharedMap<K, V> = Arc<Mutex<HashMap<K, V>>>;
type CatalogEntryKey = (Uuid, String, String);

#[derive(Default, Clone)]
struct MemoryModelCatalogRepository {
    catalog_sources: SharedMap<Uuid, domain::ModelProviderCatalogSourceRecord>,
    catalog_sync_runs: SharedMap<Uuid, domain::ModelCatalogSyncRunRecord>,
    catalog_entries: SharedMap<CatalogEntryKey, domain::ModelProviderCatalogEntryRecord>,
    queue_templates: SharedMap<Uuid, domain::ModelFailoverQueueTemplateRecord>,
    queue_items: SharedMap<Uuid, Vec<domain::ModelFailoverQueueItemRecord>>,
    queue_snapshots: SharedMap<Uuid, domain::ModelFailoverQueueSnapshotRecord>,
}

#[async_trait]
impl ModelProviderRepository for MemoryModelCatalogRepository {
    async fn create_instance(
        &self,
        _input: &CreateModelProviderInstanceInput,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        unimplemented!("not needed for model catalog failover tests")
    }

    async fn update_instance(
        &self,
        _input: &UpdateModelProviderInstanceInput,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        unimplemented!("not needed for model catalog failover tests")
    }

    async fn get_instance(
        &self,
        _workspace_id: Uuid,
        _instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderInstanceRecord>> {
        Ok(None)
    }

    async fn list_instances(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        Ok(Vec::new())
    }

    async fn list_instances_by_provider_code(
        &self,
        _provider_code: &str,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        Ok(Vec::new())
    }

    async fn reassign_instances_to_installation(
        &self,
        _input: &ReassignModelProviderInstancesInput,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        Ok(Vec::new())
    }

    async fn upsert_catalog_cache(
        &self,
        _input: &UpsertModelProviderCatalogCacheInput,
    ) -> Result<domain::ModelProviderCatalogCacheRecord> {
        unimplemented!("not needed for model catalog failover tests")
    }

    async fn get_catalog_cache(
        &self,
        _provider_instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderCatalogCacheRecord>> {
        Ok(None)
    }

    async fn upsert_secret(
        &self,
        _input: &UpsertModelProviderSecretInput,
    ) -> Result<domain::ModelProviderSecretRecord> {
        unimplemented!("not needed for model catalog failover tests")
    }

    async fn upsert_main_instance(
        &self,
        _input: &UpsertModelProviderMainInstanceInput,
    ) -> Result<domain::ModelProviderMainInstanceRecord> {
        unimplemented!("not needed for model catalog failover tests")
    }

    async fn get_main_instance(
        &self,
        _workspace_id: Uuid,
        _provider_code: &str,
    ) -> Result<Option<domain::ModelProviderMainInstanceRecord>> {
        Ok(None)
    }

    async fn create_preview_session(
        &self,
        _input: &CreateModelProviderPreviewSessionInput,
    ) -> Result<domain::ModelProviderPreviewSessionRecord> {
        unimplemented!("not needed for model catalog failover tests")
    }

    async fn get_preview_session(
        &self,
        _workspace_id: Uuid,
        _session_id: Uuid,
    ) -> Result<Option<domain::ModelProviderPreviewSessionRecord>> {
        Ok(None)
    }

    async fn delete_preview_session(&self, _workspace_id: Uuid, _session_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn get_secret_json(
        &self,
        _provider_instance_id: Uuid,
        _master_key: &str,
    ) -> Result<Option<Value>> {
        Ok(None)
    }

    async fn get_secret_record(
        &self,
        _provider_instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderSecretRecord>> {
        Ok(None)
    }

    async fn delete_instance(&self, _workspace_id: Uuid, _instance_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn count_instance_references(
        &self,
        _workspace_id: Uuid,
        _instance_id: Uuid,
    ) -> Result<u64> {
        Ok(0)
    }

    async fn create_catalog_source(
        &self,
        input: &CreateModelProviderCatalogSourceInput,
    ) -> Result<domain::ModelProviderCatalogSourceRecord> {
        let now = OffsetDateTime::now_utc();
        let record = domain::ModelProviderCatalogSourceRecord {
            id: input.source_id,
            workspace_id: input.workspace_id,
            source_kind: input.source_kind.clone(),
            plugin_id: input.plugin_id.clone(),
            provider_code: input.provider_code.clone(),
            display_name: input.display_name.clone(),
            base_url_ref: input.base_url_ref.clone(),
            auth_secret_ref: input.auth_secret_ref.clone(),
            protocol: input.protocol.clone(),
            status: input.status.clone(),
            last_sync_run_id: None,
            created_at: now,
            updated_at: now,
        };
        self.catalog_sources
            .lock()
            .expect("catalog_sources mutex poisoned")
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn create_catalog_sync_run(
        &self,
        input: &CreateModelCatalogSyncRunInput,
    ) -> Result<domain::ModelCatalogSyncRunRecord> {
        let record = domain::ModelCatalogSyncRunRecord {
            id: input.sync_run_id,
            catalog_source_id: input.catalog_source_id,
            status: input.status.clone(),
            error_message_ref: input.error_message_ref.clone(),
            discovered_count: input.discovered_count,
            imported_count: input.imported_count,
            disabled_count: input.disabled_count,
            started_at: input.started_at,
            finished_at: input.finished_at,
        };
        self.catalog_sync_runs
            .lock()
            .expect("catalog_sync_runs mutex poisoned")
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn upsert_catalog_entry(
        &self,
        input: &UpsertModelProviderCatalogEntryInput,
    ) -> Result<domain::ModelProviderCatalogEntryRecord> {
        let key = (
            input.catalog_source_id,
            input.upstream_model_id.clone(),
            input.protocol.clone(),
        );
        let existing = self
            .catalog_entries
            .lock()
            .expect("catalog_entries mutex poisoned")
            .get(&key)
            .cloned();
        let record = domain::ModelProviderCatalogEntryRecord {
            id: existing
                .map(|record| record.id)
                .unwrap_or_else(Uuid::now_v7),
            provider_instance_id: input.provider_instance_id,
            catalog_source_id: input.catalog_source_id,
            upstream_model_id: input.upstream_model_id.clone(),
            display_label: input.display_label.clone(),
            protocol: input.protocol.clone(),
            capability_snapshot: input.capability_snapshot.clone(),
            parameter_schema_ref: input.parameter_schema_ref.clone(),
            context_window: input.context_window,
            max_output_tokens: input.max_output_tokens,
            pricing_ref: input.pricing_ref.clone(),
            fetched_at: OffsetDateTime::now_utc(),
            status: input.status.clone(),
        };
        self.catalog_entries
            .lock()
            .expect("catalog_entries mutex poisoned")
            .insert(key, record.clone());
        Ok(record)
    }

    async fn list_catalog_entries(
        &self,
        catalog_source_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderCatalogEntryRecord>> {
        let mut entries = self
            .catalog_entries
            .lock()
            .expect("catalog_entries mutex poisoned")
            .values()
            .filter(|entry| entry.catalog_source_id == catalog_source_id)
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.upstream_model_id.cmp(&right.upstream_model_id));
        Ok(entries)
    }

    async fn list_catalog_entries_for_provider_instance(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderCatalogEntryRecord>> {
        let mut entries = self
            .catalog_entries
            .lock()
            .expect("catalog_entries mutex poisoned")
            .values()
            .filter(|entry| entry.provider_instance_id == Some(provider_instance_id))
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.upstream_model_id.cmp(&right.upstream_model_id));
        Ok(entries)
    }

    async fn create_failover_queue_template(
        &self,
        input: &CreateModelFailoverQueueTemplateInput,
    ) -> Result<domain::ModelFailoverQueueTemplateRecord> {
        let now = OffsetDateTime::now_utc();
        let record = domain::ModelFailoverQueueTemplateRecord {
            id: input.queue_template_id,
            workspace_id: input.workspace_id,
            name: input.name.clone(),
            version: input.version,
            status: input.status.clone(),
            created_by: input.created_by,
            created_at: now,
            updated_at: now,
        };
        self.queue_templates
            .lock()
            .expect("queue_templates mutex poisoned")
            .insert(record.id, record.clone());
        Ok(record)
    }

    async fn get_failover_queue_template(
        &self,
        queue_template_id: Uuid,
    ) -> Result<Option<domain::ModelFailoverQueueTemplateRecord>> {
        Ok(self
            .queue_templates
            .lock()
            .expect("queue_templates mutex poisoned")
            .get(&queue_template_id)
            .cloned())
    }

    async fn create_failover_queue_item(
        &self,
        input: &CreateModelFailoverQueueItemInput,
    ) -> Result<domain::ModelFailoverQueueItemRecord> {
        let record = domain::ModelFailoverQueueItemRecord {
            id: input.queue_item_id,
            queue_template_id: input.queue_template_id,
            sort_index: input.sort_index,
            provider_instance_id: input.provider_instance_id,
            provider_instance_display_name: "Fixture Provider".to_string(),
            provider_code: input.provider_code.clone(),
            upstream_model_id: input.upstream_model_id.clone(),
            protocol: input.protocol.clone(),
            enabled: input.enabled,
        };
        self.queue_items
            .lock()
            .expect("queue_items mutex poisoned")
            .entry(record.queue_template_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    async fn list_failover_queue_items(
        &self,
        queue_template_id: Uuid,
    ) -> Result<Vec<domain::ModelFailoverQueueItemRecord>> {
        Ok(self
            .queue_items
            .lock()
            .expect("queue_items mutex poisoned")
            .get(&queue_template_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn create_failover_queue_snapshot(
        &self,
        input: &CreateModelFailoverQueueSnapshotInput,
    ) -> Result<domain::ModelFailoverQueueSnapshotRecord> {
        let record = domain::ModelFailoverQueueSnapshotRecord {
            id: input.snapshot_id,
            queue_template_id: input.queue_template_id,
            version: input.version,
            items: input.items.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        self.queue_snapshots
            .lock()
            .expect("queue_snapshots mutex poisoned")
            .insert(record.id, record.clone());
        Ok(record)
    }
}

#[tokio::test]
async fn relay_catalog_sync_imports_entries_as_model_provider_targets() {
    let repository = MemoryModelCatalogRepository::default();
    let source = repository
        .create_catalog_source(&CreateModelProviderCatalogSourceInput {
            source_id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            source_kind: "relay_plugin".to_string(),
            plugin_id: "new-api".to_string(),
            provider_code: "new-api".to_string(),
            display_name: "New API".to_string(),
            base_url_ref: None,
            auth_secret_ref: None,
            protocol: "openai".to_string(),
            status: "active".to_string(),
        })
        .await
        .unwrap();

    let run = sync_relay_catalog_models(
        &repository,
        source.id,
        json!([
            {
                "upstream_model_id": "gpt-4.1",
                "display_label": "GPT 4.1",
                "protocol": "openai",
                "context_window": 1048576,
                "max_output_tokens": 32768,
                "pricing": { "input": "2.00", "output": "8.00" }
            }
        ]),
    )
    .await
    .unwrap();

    let entries = repository.list_catalog_entries(source.id).await.unwrap();

    assert_eq!(run.imported_count, 1);
    assert_eq!(entries[0].upstream_model_id, "gpt-4.1");
    assert_eq!(entries[0].protocol, "openai");
    assert_eq!(
        entries[0].pricing_ref.as_deref(),
        Some("runtime_artifact:pricing:gpt-4.1")
    );
}

#[tokio::test]
async fn failover_queue_snapshot_freezes_order_for_run() {
    let repository = MemoryModelCatalogRepository::default();
    let queue = repository
        .create_failover_queue_template(&CreateModelFailoverQueueTemplateInput {
            queue_template_id: Uuid::now_v7(),
            workspace_id: Uuid::now_v7(),
            name: "production".to_string(),
            version: 1,
            status: "active".to_string(),
            created_by: Uuid::now_v7(),
        })
        .await
        .unwrap();
    let openai = repository
        .create_failover_queue_item(&CreateModelFailoverQueueItemInput {
            queue_item_id: Uuid::now_v7(),
            queue_template_id: queue.id,
            sort_index: 0,
            provider_instance_id: Uuid::now_v7(),
            provider_code: "openai-prod-a".to_string(),
            upstream_model_id: "gpt-4.1".to_string(),
            protocol: "openai".to_string(),
            enabled: true,
        })
        .await
        .unwrap();
    let anthropic = repository
        .create_failover_queue_item(&CreateModelFailoverQueueItemInput {
            queue_item_id: Uuid::now_v7(),
            queue_template_id: queue.id,
            sort_index: 1,
            provider_instance_id: Uuid::now_v7(),
            provider_code: "anthropic-main".to_string(),
            upstream_model_id: "sonnet".to_string(),
            protocol: "anthropic".to_string(),
            enabled: true,
        })
        .await
        .unwrap();

    let snapshot_items = failover_queue::freeze_queue_items(&[
        failover_queue::FailoverQueueSnapshotItem::from(openai),
        failover_queue::FailoverQueueSnapshotItem::from(anthropic),
    ]);
    let snapshot = repository
        .create_failover_queue_snapshot(&CreateModelFailoverQueueSnapshotInput {
            snapshot_id: Uuid::now_v7(),
            queue_template_id: queue.id,
            version: queue.version,
            items: snapshot_items,
        })
        .await
        .unwrap();

    {
        let mut items = repository
            .queue_items
            .lock()
            .expect("queue_items mutex poisoned");
        let queue_items = items.get_mut(&queue.id).unwrap();
        queue_items[0].sort_index = 1;
        queue_items[1].sort_index = 0;
    }

    let frozen = serde_json::from_value::<Vec<failover_queue::FailoverQueueSnapshotItem>>(
        snapshot.items.clone(),
    )
    .unwrap();
    assert_eq!(frozen[0].upstream_model_id, "gpt-4.1");
    assert_eq!(frozen[1].upstream_model_id, "sonnet");
}
