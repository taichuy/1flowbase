use crate::repositories::PgControlPlaneStore;
use async_trait::async_trait;
use control_plane_contracts::ports::ManagedServiceMutationRepository;
#[async_trait]
impl ManagedServiceMutationRepository for PgControlPlaneStore {
    async fn lock_managed_service_mutation(
        &self,
        installation_id: uuid::Uuid,
        scope_id: uuid::Uuid,
        deadline_unix_ms: i64,
    ) -> anyhow::Result<Box<dyn Send>> {
        let remaining = i128::from(deadline_unix_ms)
            - time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        anyhow::ensure!(remaining > 0, "managed service deadline exceeded");
        let mut transaction = self.pool().begin().await?;
        sqlx::query("select set_config('statement_timeout', $1, true)")
            .bind(format!("{remaining}ms"))
            .execute(&mut *transaction)
            .await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended('managed-service-mutation:' || $1 || ':' || $2, 0))")
            .bind(installation_id.to_string()).bind(scope_id.to_string()).execute(&mut *transaction).await?;
        // This transaction owns only the advisory lock. Callback data/authority transactions are independent.
        Ok(Box::new(transaction))
    }
}
