//! A narrow native-owner unit of work. SQLx inner transactions become savepoints;
//! all owner authorization, audit, publication compilation and DDL paths are reused.
use super::*;
use sqlx::{
    postgres::{PgPoolOptions, PgTransactionManager},
    TransactionManager,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct NativeTemplateTransaction {
    pool: sqlx::PgPool,
    finished: bool,
}
#[async_trait]
impl PortableTemplateTransactionGuard for NativeTemplateTransaction {
    async fn commit(&mut self) -> Result<()> {
        anyhow::ensure!(!self.finished, "portable_template_transaction_finished");
        let mut connection = self.pool.acquire().await?;
        anyhow::ensure!(
            PgTransactionManager::get_transaction_depth(&connection) == 1,
            "portable_template_transaction_depth"
        );
        PgTransactionManager::commit(&mut connection).await?;
        self.finished = true;
        drop(connection);
        self.pool.close().await;
        Ok(())
    }
    async fn rollback(&mut self) -> Result<()> {
        if !self.finished {
            // Closing the sole physical session rolls back the outer transaction,
            // even if an owner future failed with an unbalanced savepoint.
            self.finished = true;
            self.pool.close().await;
        }
        Ok(())
    }
}
impl Drop for NativeTemplateTransaction {
    fn drop(&mut self) {
        if !self.finished {
            let pool = self.pool.clone();
            // Never leave an uncommitted session reusable after cancellation.
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move { pool.close().await });
            }
        }
    }
}

#[async_trait]
impl PortableTemplateTransactionRepository for PgControlPlaneStore {
    async fn begin_portable_template_transaction(
        &self,
    ) -> Result<PortableTemplateTransaction<Self>> {
        let initialized = Arc::new(AtomicBool::new(false));
        let pool = PgPoolOptions::new()
            .max_connections(1).min_connections(0)
            .max_lifetime(None).idle_timeout(None)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .after_connect(move |connection, _| {
                let initialized = initialized.clone();
                Box::pin(async move {
                    // A replacement connection would lose transaction state. Fail closed.
                    if initialized.swap(true, Ordering::SeqCst) {
                        return Err(sqlx::Error::Protocol("portable_template_transaction_connection_lost".into()));
                    }
                    PgTransactionManager::begin(connection, None).await?;
                    sqlx::query("SET LOCAL lock_timeout = '10s'").execute(&mut *connection).await?;
                    // These are actual PostgreSQL write locks, including ordinary editor
                    // UPDATE/INSERT/DELETE transactions. READ queries remain available.
                    sqlx::query("LOCK TABLE application_api_mappings, application_publication_versions, application_tag_bindings, application_tags, applications, flows, flow_drafts, flow_versions, frontstage_block_codes, frontstage_block_nodes, frontstage_page_schemas, frontstage_page_tabs, frontstage_page_visibility_rules, frontstage_pages, model_definitions, model_fields, roles, workflow_extension_triggers, workflow_schedule_triggers IN SHARE ROW EXCLUSIVE MODE")
                        .execute(&mut *connection).await?;
                    Ok(())
                })
            })
            .before_acquire(|connection, _| Box::pin(async move {
                Ok(PgTransactionManager::get_transaction_depth(connection) == 1)
            }))
            .connect_with((*self.pool().connect_options()).clone()).await?;
        let repository = self.with_portable_template_pool(pool.clone());
        Ok(PortableTemplateTransaction {
            repository,
            guard: Box::new(NativeTemplateTransaction {
                pool,
                finished: false,
            }),
        })
    }
    async fn acknowledge_portable_template_write(
        &self,
        scope: &TemplateBaselineScope,
        key: &TemplateResourceKey,
        intent: &TemplateWriteIntent,
        actual_fingerprint: &str,
    ) -> Result<bool> {
        let mut connection = self.pool().acquire().await?;
        anyhow::ensure!(
            PgTransactionManager::get_transaction_depth(&connection) == 1,
            "portable_template_receipt_requires_owner_transaction"
        );
        acknowledge_template_write(&mut connection, scope, key, intent, actual_fingerprint).await
    }
}
