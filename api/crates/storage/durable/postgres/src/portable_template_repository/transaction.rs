//! A narrow native-owner unit of work. SQLx inner transactions become savepoints;
//! all owner authorization, audit, publication compilation and DDL paths are reused.
use super::*;
use sqlx::postgres::{PgPoolOptions, PgTransactionManager};
// SQLx 0.9 removed the facade re-export. Keep the existing owner transaction
// depth invariant through its defining crate, rather than raw SQL COMMIT.
use sqlx_core::transaction::TransactionManager;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct NativeTemplateTransaction {
    pool: Option<crate::PgTemporaryPool>,
}
#[async_trait]
impl PortableTemplateTransactionGuard for NativeTemplateTransaction {
    async fn commit(&mut self) -> Result<()> {
        let owner = self
            .pool
            .as_ref()
            .context("portable_template_transaction_finished")?;
        let mut connection = owner.pool().acquire().await?;
        anyhow::ensure!(
            PgTransactionManager::get_transaction_depth(&connection) == 1,
            "portable_template_transaction_depth"
        );
        PgTransactionManager::commit(&mut connection).await?;
        drop(connection);
        self.pool
            .take()
            .expect("unfinished transaction has a pool owner")
            .close()
            .await;
        Ok(())
    }
    async fn rollback(&mut self) -> Result<()> {
        if let Some(owner) = self.pool.take() {
            // Closing the sole physical session rolls back the outer transaction,
            // even if an owner future failed with an unbalanced savepoint.
            owner.close().await;
        }
        Ok(())
    }
}

#[async_trait]
impl PortableTemplateTransactionRepository for PgControlPlaneStore {
    async fn begin_portable_template_transaction(
        &self,
    ) -> Result<PortableTemplateTransaction<Self>> {
        let initialized = Arc::new(AtomicBool::new(false));
        let options = PgPoolOptions::new()
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
                    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
                    loop {
                        PgTransactionManager::begin(connection, None).await?;
                        sqlx::query("SET LOCAL lock_timeout = '10s'").execute(&mut *connection).await?;
                        // Preserve the complete editor write exclusion and its 10s wait
                        // window. NOWAIT keeps a cancelled bootstrap from leaving a
                        // server-side lock waiter after its temporary pool has drained.
                        let locked = sqlx::query("LOCK TABLE application_api_mappings, application_publication_versions, application_tag_bindings, application_tags, applications, flows, flow_drafts, flow_versions, frontstage_block_codes, frontstage_block_nodes, frontstage_page_schemas, frontstage_page_tabs, frontstage_page_visibility_rules, frontstage_pages, mcp_groups, mcp_instance_discovery_policies, mcp_instances, mcp_tool_bindings, mcp_tools, mcp_upstream_connection_secrets, mcp_upstream_connections, model_definitions, model_fields, roles, workflow_extension_triggers, workflow_schedule_triggers IN SHARE ROW EXCLUSIVE MODE NOWAIT")
                            .execute(&mut *connection).await;
                        match locked {
                            Ok(_) => return Ok(()),
                            Err(error) if error.as_database_error().is_some_and(|error| error.code().as_deref()==Some("55P03"))
                                && tokio::time::Instant::now()<deadline => {
                                // A failed LOCK aborts this attempt. No resource writer
                                // receives the pool until a fresh outer transaction owns
                                // every table, at the original transaction depth of one.
                                PgTransactionManager::rollback(connection).await?;
                                tokio::time::sleep_until(std::cmp::min(
                                    deadline,
                                    tokio::time::Instant::now()+std::time::Duration::from_millis(20),
                                )).await;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                })
            })
            .before_acquire(|connection, _| Box::pin(async move {
                Ok(PgTransactionManager::get_transaction_depth(connection) == 1)
            }));
        // The factory owns quota before creating the private session and retains
        // it through rollback/commit drain, even when this caller is cancelled.
        let owner = self
            .connections()
            .temporary_pool(
                options,
                tokio::time::Instant::now() + std::time::Duration::from_secs(30),
            )
            .await?;
        let repository = self.with_portable_template_pool(owner.pool().clone());
        Ok(PortableTemplateTransaction {
            repository,
            guard: Box::new(NativeTemplateTransaction { pool: Some(owner) }),
        })
    }
    async fn retarget_template_mcp_binding(
        &self,
        scope: &TemplateBaselineScope,
        command: &TemplateMcpBindingRetarget,
    ) -> Result<bool> {
        let mut connection = self.pool().acquire().await?;
        anyhow::ensure!(
            self.portable_template_transaction
                && PgTransactionManager::get_transaction_depth(&connection) == 1,
            "portable_template_retarget_requires_owner_transaction"
        );
        super::mcp_retarget::retarget(&mut connection, scope, command).await
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
