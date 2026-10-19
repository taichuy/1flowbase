use async_trait::async_trait;
/// Exclusive mutation ownership for a system managed service across API nodes.
#[async_trait]
pub trait ManagedServiceMutationRepository: Send + Sync {
    async fn lock_managed_service_mutation(
        &self,
        installation_id: uuid::Uuid,
        scope_id: uuid::Uuid,
        deadline_unix_ms: i64,
    ) -> anyhow::Result<Box<dyn Send>>;
}
