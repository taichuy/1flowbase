use std::collections::BTreeMap;

use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct FrontstageBlockPosition {
    pub parent_block_id: Option<String>,
    pub before_block_id: Option<String>,
    pub after_block_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateFrontstageBlockNodeInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub block_id: String,
    pub position: FrontstageBlockPosition,
    pub presentation: domain::FrontstageBlockPresentation,
    pub title: Option<String>,
    pub description: Option<String>,
    pub code_ref: String,
    pub schema_version: u32,
    pub input_mapping: BTreeMap<String, String>,
    pub output_mapping: BTreeMap<String, String>,
    pub runtime_descriptor: serde_json::Value,
    pub code: FrontstageBlockCodeInput,
    pub audit_log: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct FrontstageBlockCodeInput {
    pub source_code: String,
    pub dependency_lock: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpdateFrontstageBlockNodeInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub block_id: String,
    pub presentation: Option<domain::FrontstageBlockPresentation>,
    pub title: Option<Option<String>>,
    pub description: Option<Option<String>>,
    pub input_mapping: Option<BTreeMap<String, String>>,
    pub output_mapping: Option<BTreeMap<String, String>>,
    pub runtime_descriptor: Option<serde_json::Value>,
    pub audit_log: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct SaveFrontstageBlockNodeCodeInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub block_id: String,
    pub expected_source_revision: Option<String>,
    pub code: FrontstageBlockCodeInput,
    pub audit_log: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct MoveFrontstageBlockNodeInput {
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub page_id: Uuid,
    pub block_id: String,
    pub position: FrontstageBlockPosition,
    pub audit_log: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct DeleteFrontstageBlockLeafInput {
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub block_id: String,
    pub audit_log: domain::AuditLogRecord,
}

#[derive(Debug, Clone)]
pub struct DeleteFrontstageBlockSubtreeInput {
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub block_id: String,
    pub expected_affected_count: u64,
    pub audit_log: domain::AuditLogRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontstageBlockSubtreeDeleteResult {
    pub deleted_count: u64,
}

#[async_trait]
pub trait FrontstageBlockTreeRepository: Send + Sync {
    async fn create_frontstage_block_node(
        &self,
        input: &CreateFrontstageBlockNodeInput,
    ) -> anyhow::Result<domain::FrontstageBlockNodeRecord>;

    async fn get_frontstage_block_node(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> anyhow::Result<Option<domain::FrontstageBlockNodeRecord>>;

    async fn get_frontstage_block_runtime_assembly(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> anyhow::Result<Vec<domain::frontstage::FrontstageBlockRuntimeLayer>>;

    async fn list_frontstage_block_roots(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        limit: u32,
    ) -> anyhow::Result<Vec<domain::FrontstageBlockNodeSummary>>;

    async fn list_frontstage_block_children(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        parent_block_id: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<domain::FrontstageBlockNodeSummary>>;

    async fn list_frontstage_block_ancestors(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> anyhow::Result<Vec<domain::FrontstageBlockNodeSummary>>;

    async fn list_frontstage_block_descendants(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
        max_depth: u32,
        limit: u32,
    ) -> anyhow::Result<Vec<domain::FrontstageBlockDescendantProjection>>;

    async fn search_frontstage_blocks(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        query: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<domain::FrontstageBlockSearchResult>>;

    async fn get_frontstage_block_subtree_impact(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> anyhow::Result<domain::FrontstageBlockSubtreeImpact>;

    async fn update_frontstage_block_node(
        &self,
        input: &UpdateFrontstageBlockNodeInput,
    ) -> anyhow::Result<domain::FrontstageBlockNodeRecord>;

    async fn save_frontstage_block_node_code(
        &self,
        input: &SaveFrontstageBlockNodeCodeInput,
    ) -> anyhow::Result<domain::frontstage::FrontstageBlockCodeRecord>;

    async fn move_frontstage_block_node(
        &self,
        input: &MoveFrontstageBlockNodeInput,
    ) -> anyhow::Result<domain::FrontstageBlockNodeRecord>;

    async fn delete_frontstage_block_leaf(
        &self,
        input: &DeleteFrontstageBlockLeafInput,
    ) -> anyhow::Result<bool>;

    async fn delete_frontstage_block_subtree(
        &self,
        input: &DeleteFrontstageBlockSubtreeInput,
    ) -> anyhow::Result<FrontstageBlockSubtreeDeleteResult>;
}
