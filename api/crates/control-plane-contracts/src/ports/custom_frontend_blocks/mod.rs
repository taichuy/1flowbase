use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A workspace-owned definition. Runtime authority always comes from the current base contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFrontendBlock {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub provider_code: String,
    pub contribution_code: String,
    pub title: String,
    pub source: String,
    pub language: String,
    pub revision: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFrontendBlockBody {
    pub title: String,
    pub source: String,
    pub language: String,
}

#[async_trait]
pub trait CustomFrontendBlockRepository: Send + Sync {
    async fn list_custom_frontend_blocks(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<CustomFrontendBlock>>;
    async fn create_custom_frontend_block(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
        contribution_code: &str,
        body: &CustomFrontendBlockBody,
        actor_user_id: Uuid,
    ) -> Result<CustomFrontendBlock>;
    async fn update_custom_frontend_block(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        body: &CustomFrontendBlockBody,
        actor_user_id: Uuid,
    ) -> Result<Option<CustomFrontendBlock>>;
    async fn delete_custom_frontend_block(&self, workspace_id: Uuid, id: Uuid) -> Result<bool>;
}
