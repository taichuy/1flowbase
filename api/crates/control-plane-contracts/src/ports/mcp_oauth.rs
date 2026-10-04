//! Durable OAuth protocol state. Secrets are represented only by one-way hashes.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpOAuthGrant {
    pub id: Uuid,
    pub user_id: Uuid,
    pub api_key_id: Uuid,
    pub key_hash: String,
    pub role_code: String,
    pub workspace_id: Uuid,
    pub instance_id: String,
    pub client_id: String,
    pub resource: String,
    pub expires_at: i64,
    pub authorization_stamp: Value,
}

#[async_trait]
pub trait McpOAuthRepository: Send + Sync {
    async fn oauth_authorization_stamp(&self, user_id: Uuid) -> anyhow::Result<Value>;
    async fn oauth_put(
        &self,
        kind: &str,
        hash: &str,
        value: Value,
        expires_at: i64,
    ) -> anyhow::Result<()>;
    async fn oauth_get(&self, kind: &str, hash: &str) -> anyhow::Result<Option<Value>>;
    /// Atomic compare-and-consume; callers validate the complete payload before consumption.
    async fn oauth_consume(&self, kind: &str, hash: &str, expected: &Value)
        -> anyhow::Result<bool>;
    async fn oauth_create_grant(&self, grant: &McpOAuthGrant) -> anyhow::Result<()>;
    async fn oauth_grant(&self, id: Uuid) -> anyhow::Result<Option<McpOAuthGrant>>;
    async fn oauth_revoke_grant(&self, id: Uuid) -> anyhow::Result<()>;
    /// Serializes on the grant row. A reused refresh hash revokes the entire grant family.
    async fn oauth_rotate_refresh(
        &self,
        grant: Uuid,
        old_hash: Option<&str>,
        new_hash: &str,
        expires_at: i64,
    ) -> anyhow::Result<bool>;
    async fn oauth_refresh_grant(&self, hash: &str) -> anyhow::Result<Option<Uuid>>;
}
