use anyhow::{bail, Result};
use async_trait::async_trait;
use control_plane::ports::{
    CreateNetworkEgressProviderInput, NetworkEgressRepository, RecordNetworkEgressSyncFailureInput,
    ReplaceNetworkEgressProjectionInput, UpdateNetworkEgressProviderLifecycleInput,
};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn lifecycle(value: &str) -> Result<domain::NetworkEgressProviderLifecycle> {
    match value {
        "draft" => Ok(domain::NetworkEgressProviderLifecycle::Draft),
        "active" => Ok(domain::NetworkEgressProviderLifecycle::Active),
        "disabled" => Ok(domain::NetworkEgressProviderLifecycle::Disabled),
        _ => bail!("invalid network egress provider lifecycle"),
    }
}

fn health(value: &str) -> Result<domain::NetworkEgressHealthStatus> {
    match value {
        "unknown" => Ok(domain::NetworkEgressHealthStatus::Unknown),
        "healthy" => Ok(domain::NetworkEgressHealthStatus::Healthy),
        "unhealthy" => Ok(domain::NetworkEgressHealthStatus::Unhealthy),
        _ => bail!("invalid network egress health status"),
    }
}

fn provider(row: sqlx::postgres::PgRow) -> Result<domain::NetworkEgressProviderRecord> {
    Ok(domain::NetworkEgressProviderRecord {
        id: row.get("id"),
        installation_id: row.get("installation_id"),
        provider_code: row.get("provider_code"),
        display_name: row.get("display_name"),
        secret_ref: row.get("secret_ref"),
        lifecycle: lifecycle(row.get::<String, _>("lifecycle").as_str())?,
        health_status: health(row.get::<String, _>("health_status").as_str())?,
        last_sync_error: row.get("last_sync_error"),
        last_synced_at: row.get("last_synced_at"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn projection(row: sqlx::postgres::PgRow) -> domain::NetworkEgressProjectionRecord {
    domain::NetworkEgressProjectionRecord {
        provider_id: row.get("provider_id"),
        provider_egress_key: row.get("provider_egress_key"),
        display_name: row.get("display_name"),
        region: row.get("region"),
        tags: row.get("tags"),
        availability: row.get("availability"),
        synced_at: row.get("synced_at"),
    }
}

#[async_trait]
impl NetworkEgressRepository for PgControlPlaneStore {
    async fn get_network_egress_provider(
        &self,
        provider_id: Uuid,
    ) -> Result<Option<domain::NetworkEgressProviderRecord>> {
        sqlx::query("select * from network_egress_providers where id = $1")
            .bind(provider_id)
            .fetch_optional(self.pool())
            .await?
            .map(provider)
            .transpose()
    }

    async fn list_network_egress_providers(
        &self,
    ) -> Result<Vec<domain::NetworkEgressProviderRecord>> {
        sqlx::query("select * from network_egress_providers order by display_name asc, id asc")
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(provider)
            .collect()
    }

    async fn create_network_egress_provider(
        &self,
        input: &CreateNetworkEgressProviderInput,
    ) -> Result<domain::NetworkEgressProviderRecord> {
        let row = sqlx::query(
            r#"
            insert into network_egress_providers (
                id, scope_id, installation_id, provider_code, display_name, secret_ref,
                lifecycle, health_status, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, 'unknown', $8, $8) returning *
        "#,
        )
        .bind(input.provider_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(input.installation_id)
        .bind(&input.provider_code)
        .bind(&input.display_name)
        .bind(&input.secret_ref)
        .bind(input.lifecycle.as_str())
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;
        provider(row)
    }

    async fn update_network_egress_provider_lifecycle(
        &self,
        input: &UpdateNetworkEgressProviderLifecycleInput,
    ) -> Result<domain::NetworkEgressProviderRecord> {
        let row = sqlx::query(
            r#"
            update network_egress_providers set lifecycle = $2, updated_by = $3, updated_at = now()
            where id = $1 returning *
        "#,
        )
        .bind(input.provider_id)
        .bind(input.lifecycle.as_str())
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(provider).transpose()?.ok_or_else(|| {
            anyhow::anyhow!(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_provider"
            ))
        })
    }

    async fn list_network_egress_projections(
        &self,
        provider_id: Uuid,
    ) -> Result<Vec<domain::NetworkEgressProjectionRecord>> {
        Ok(sqlx::query(r#"
            select provider_id, provider_egress_key, display_name, region, tags, availability, synced_at
            from network_egress_projections where provider_id = $1 order by provider_egress_key asc
        "#).bind(provider_id).fetch_all(self.pool()).await?.into_iter().map(projection).collect())
    }

    async fn replace_network_egress_projection(
        &self,
        input: &ReplaceNetworkEgressProjectionInput,
    ) -> Result<domain::NetworkEgressProviderRecord> {
        let mut transaction = self.pool().begin().await?;
        let row = sqlx::query(r#"
            update network_egress_providers
            set health_status = $2, last_sync_error = $3, last_synced_at = $4, updated_by = $5, updated_at = now()
            where id = $1 returning *
        "#).bind(input.provider_id).bind(input.health_status.as_str()).bind(&input.last_sync_error)
          .bind(input.synchronized_at).bind(input.actor_user_id).fetch_optional(&mut *transaction).await?;
        let Some(row) = row else {
            return Err(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_provider",
            )
            .into());
        };
        sqlx::query("delete from network_egress_projections where provider_id = $1")
            .bind(input.provider_id)
            .execute(&mut *transaction)
            .await?;
        for egress in &input.egresses {
            sqlx::query(r#"
                insert into network_egress_projections (
                    provider_id, provider_egress_key, display_name, region, tags, availability, synced_at
                ) values ($1, $2, $3, $4, $5, $6, $7)
            "#).bind(egress.provider_id).bind(&egress.provider_egress_key).bind(&egress.display_name)
              .bind(&egress.region).bind(&egress.tags).bind(&egress.availability).bind(egress.synced_at)
              .execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        provider(row)
    }

    async fn record_network_egress_sync_failure(
        &self,
        input: &RecordNetworkEgressSyncFailureInput,
    ) -> Result<domain::NetworkEgressProviderRecord> {
        let row = sqlx::query(r#"
            update network_egress_providers
            set health_status = 'unhealthy', last_sync_error = $2, last_synced_at = $3, updated_by = $4, updated_at = now()
            where id = $1 returning *
        "#).bind(input.provider_id).bind(&input.last_sync_error).bind(input.synchronized_at)
          .bind(input.actor_user_id).fetch_optional(self.pool()).await?;
        row.map(provider).transpose()?.ok_or_else(|| {
            anyhow::anyhow!(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_provider"
            ))
        })
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        self.append_audit_log(event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane::ports::{PluginRepository, UpsertPluginInstallationInput};
    use domain::{
        ExtensionCategory, ExtensionSignatureStatus, PluginDesiredState, PluginVerificationStatus,
    };
    use serde_json::json;
    use time::OffsetDateTime;

    fn database_url() -> String {
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
    }

    async fn store() -> (PgControlPlaneStore, domain::UserRecord) {
        let schema = postgres_test_support::PostgresTestSchema::create(&database_url())
            .await
            .expect("isolated schema should be available");
        let pool = schema.connect().await.expect("schema should connect");
        crate::run_migrations(&pool)
            .await
            .expect("migrations should apply");
        let store = PgControlPlaneStore::new(pool);
        let tenant = store
            .upsert_root_tenant()
            .await
            .expect("tenant should seed");
        let workspace = store
            .upsert_workspace(tenant.id, "network-egress")
            .await
            .expect("workspace should seed");
        let actor = store
            .upsert_root_user(
                workspace.id,
                "network-egress-root",
                "network-egress-root@example.com",
                "$argon2id$v=19$m=19456,t=2,p=1$test$test",
                "Network",
                "Root",
            )
            .await
            .expect("actor should seed");
        (store, actor)
    }

    #[tokio::test]
    async fn ac_015_failed_sync_preserves_stably_ordered_egress_projection() {
        let (store, actor) = store().await;
        let installation_id = Uuid::now_v7();
        PluginRepository::upsert_installation(
            &store,
            &UpsertPluginInstallationInput {
                installation_id,
                category: ExtensionCategory::RuntimeExtensions,
                organization: "test".to_string(),
                provider_code: "fixture_egress".to_string(),
                plugin_id: "fixture_egress@0.1.0".to_string(),
                plugin_version: "0.1.0".to_string(),
                contract_version: plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT.to_string(),
                protocol: "stdio_json_worker".to_string(),
                display_name: "Fixture egress".to_string(),
                source_kind: "uploaded".to_string(),
                trust_level: "unverified".to_string(),
                verification_status: PluginVerificationStatus::Valid,
                desired_state: PluginDesiredState::ActiveRequested,
                expected_checksum: None,
                signature_status: ExtensionSignatureStatus::Missing,
                signature_algorithm: None,
                signing_key_id: None,
                metadata_json: json!({}),
                is_system_reserved: false,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("installation should persist");
        let provider_id = Uuid::now_v7();
        NetworkEgressRepository::create_network_egress_provider(
            &store,
            &CreateNetworkEgressProviderInput {
                provider_id,
                installation_id,
                provider_code: "fixture_egress".to_string(),
                display_name: "Fixture egress".to_string(),
                secret_ref: "secret://system/network-egress/fixture".to_string(),
                lifecycle: domain::NetworkEgressProviderLifecycle::Active,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("provider should persist");
        let synchronized_at = OffsetDateTime::now_utc();
        NetworkEgressRepository::replace_network_egress_projection(
            &store,
            &ReplaceNetworkEgressProjectionInput {
                provider_id,
                health_status: domain::NetworkEgressHealthStatus::Healthy,
                last_sync_error: None,
                synchronized_at,
                actor_user_id: actor.id,
                egresses: vec![
                    domain::NetworkEgressProjectionRecord {
                        provider_id,
                        provider_egress_key: "z-last".to_string(),
                        display_name: "Last".to_string(),
                        region: None,
                        tags: Vec::new(),
                        availability: "available".to_string(),
                        synced_at: synchronized_at,
                    },
                    domain::NetworkEgressProjectionRecord {
                        provider_id,
                        provider_egress_key: "a-first".to_string(),
                        display_name: "First".to_string(),
                        region: None,
                        tags: Vec::new(),
                        availability: "available".to_string(),
                        synced_at: synchronized_at,
                    },
                ],
            },
        )
        .await
        .expect("projection should persist");

        let initial = NetworkEgressRepository::list_network_egress_projections(&store, provider_id)
            .await
            .expect("projection should list");
        assert_eq!(
            initial
                .iter()
                .map(|egress| egress.provider_egress_key.as_str())
                .collect::<Vec<_>>(),
            ["a-first", "z-last"]
        );

        let failed = NetworkEgressRepository::record_network_egress_sync_failure(
            &store,
            &RecordNetworkEgressSyncFailureInput {
                provider_id,
                last_sync_error: "network_egress_sync_failed".to_string(),
                synchronized_at: OffsetDateTime::now_utc(),
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("failure should be projected");
        assert_eq!(
            failed.health_status,
            domain::NetworkEgressHealthStatus::Unhealthy
        );
        let retained =
            NetworkEgressRepository::list_network_egress_projections(&store, provider_id)
                .await
                .expect("prior projection should remain readable");
        assert_eq!(retained, initial);
    }
}
