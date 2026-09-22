//! Portable settings rows. Structure means persisted configuration, never DDL.
use super::BackupComponentSource;
use crate::ports::BackupComponentReader;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

pub const SELECTIVE_BACKUP_CONTENT_TYPE: &str = "application/vnd.1flowbase.settings-rows+gzip";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SelectiveBackupSelection {
    pub feature_id: String,
    pub structure: bool,
    pub data: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectiveBackupCategory {
    pub feature_id: String,
    pub label_key: String,
    pub structure_bytes: u64,
    pub data_bytes: u64,
    pub structure_tables: Vec<String>,
    pub data_tables: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectiveBackupPreview {
    pub failures: Vec<String>,
    pub missing_plugins: Vec<String>,
    pub table_count: u64,
    pub row_count: u64,
    pub selected_tables: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectiveBackupObjectScope {
    pub file_table_ids: Vec<Uuid>,
    pub include_runtime_debug_artifacts: bool,
}
#[async_trait]
pub trait SelectiveBackupRepository: Send + Sync {
    async fn catalog(&self) -> anyhow::Result<Vec<SelectiveBackupCategory>>;
    async fn source(
        &self,
        selection: Vec<SelectiveBackupSelection>,
    ) -> anyhow::Result<Arc<dyn BackupComponentSource>>;
    async fn object_scope(
        &self,
        selection: &[SelectiveBackupSelection],
    ) -> anyhow::Result<SelectiveBackupObjectScope>;
    async fn preflight(
        &self,
        reader: BackupComponentReader,
        source_master_key: &str,
        target_master_key: &str,
    ) -> anyhow::Result<SelectiveBackupPreview>;
    async fn prepare_restore(
        &self,
        reader: BackupComponentReader,
        source_master_key: &str,
        target_master_key: &str,
        confirm_missing_plugins: bool,
    ) -> anyhow::Result<Box<dyn PreparedSelectiveRestore>>;
    async fn restore(
        &self,
        reader: BackupComponentReader,
        source_master_key: &str,
        target_master_key: &str,
        confirm_missing_plugins: bool,
    ) -> anyhow::Result<SelectiveBackupPreview>;
}

/// Internal object resolver material; deliberately not serializable as an API response.
#[derive(Clone)]
pub struct SelectiveObjectStorage {
    pub storage_id: Uuid,
    pub driver_type: String,
    pub config_json: serde_json::Value,
}
#[async_trait]
pub trait PreparedSelectiveRestore: Send {
    fn preview(&self) -> &SelectiveBackupPreview;
    fn object_storages(&self) -> &[SelectiveObjectStorage];
    async fn commit(self: Box<Self>) -> anyhow::Result<SelectiveBackupPreview>;
    async fn rollback(self: Box<Self>) -> anyhow::Result<()>;
}
