//! Persistence for host-owned record policy; worker frames never supply identity.
use async_trait::async_trait;
use domain::{
    ActorContext, RecordPermissionKey, RecordPermissionPolicy, RecordPermissionRole,
    ReplaceRecordPermissions,
};
use uuid::Uuid;

#[async_trait]
pub trait RecordPermissionRepository: Send + Sync {
    async fn get_record_permissions(
        &self,
        key: &RecordPermissionKey,
    ) -> anyhow::Result<Option<RecordPermissionPolicy>>;
    /// Atomically validate workspace roles, check revision, replace header/grants and append audit.
    /// Inherit with nonempty grants is invalid. A restricted empty header denies every non-root.
    async fn replace_record_permissions(
        &self,
        key: &RecordPermissionKey,
        input: &ReplaceRecordPermissions,
        actor_user_id: Uuid,
    ) -> anyhow::Result<RecordPermissionPolicy>;
    /// Resolve current durable role relations, including departments and active-role validation.
    /// Return only role UUIDs owned by actor.current_workspace_id; never accept worker role IDs.
    async fn record_permission_effective_role_ids(
        &self,
        actor: &ActorContext,
    ) -> anyhow::Result<Vec<Uuid>>;
    /// UUID projection of the existing workspace roles source (no parallel role registry).
    async fn list_record_permission_roles(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<RecordPermissionRole>>;
}
