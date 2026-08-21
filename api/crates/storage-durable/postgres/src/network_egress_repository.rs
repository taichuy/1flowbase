use anyhow::{bail, Result};
use async_trait::async_trait;
use control_plane::ports::{
    CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
    CreateNetworkEgressProviderInput, CreateNetworkEgressRouteInput, NetworkEgressPoolRepository,
    NetworkEgressRepository, NetworkEgressRouteRepository, RecordNetworkEgressSyncFailureInput,
    ReplaceNetworkEgressProjectionInput, UpdateNetworkEgressPoolInput,
    UpdateNetworkEgressPoolMemberInput, UpdateNetworkEgressProviderLifecycleInput,
    UpdateNetworkEgressRouteInput, UpsertNetworkEgressProviderSecretInput,
};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;
use crate::secret_crypto::{decrypt_secret_json, encrypt_secret_json};

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

fn provider_secret(row: sqlx::postgres::PgRow) -> domain::NetworkEgressProviderSecretRecord {
    domain::NetworkEgressProviderSecretRecord {
        provider_id: row.get("provider_id"),
        secret_ref: row.get("secret_ref"),
        encrypted_secret_json: row.get("encrypted_secret_json"),
        secret_version: row.get("secret_version"),
        updated_at: row.get("updated_at"),
    }
}

fn selection_strategy(value: &str) -> Result<domain::NetworkEgressPoolSelectionStrategy> {
    match value {
        "healthy_first" => Ok(domain::NetworkEgressPoolSelectionStrategy::HealthyFirst),
        _ => bail!("invalid network egress pool selection strategy"),
    }
}

fn pool(row: sqlx::postgres::PgRow) -> Result<domain::NetworkEgressPool> {
    Ok(domain::NetworkEgressPool {
        id: row.get("id"),
        display_name: row.get("display_name"),
        selection_strategy: selection_strategy(
            row.get::<String, _>("selection_strategy").as_str(),
        )?,
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn pool_member(row: sqlx::postgres::PgRow) -> domain::NetworkEgressPoolMember {
    domain::NetworkEgressPoolMember {
        id: row.get("id"),
        pool_id: row.get("pool_id"),
        provider_id: row.get("provider_id"),
        provider_egress_key: row.get("provider_egress_key"),
        enabled: row.get("enabled"),
        sequence: row.get("sequence"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn route(row: sqlx::postgres::PgRow) -> Result<domain::NetworkEgressRoute> {
    let consumer_kind: String = row.get("consumer_kind");
    let consumer_reference = row.get("consumer_reference");
    let selector =
        domain::NetworkEgressConsumerSelector::from_storage(&consumer_kind, consumer_reference)
            .map_err(|error| anyhow::anyhow!(error))?;
    Ok(domain::NetworkEgressRoute {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        selector,
        pool_id: row.get("pool_id"),
        enabled: row.get("enabled"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
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

    async fn upsert_network_egress_provider_secret(
        &self,
        input: &UpsertNetworkEgressProviderSecretInput,
    ) -> Result<domain::NetworkEgressProviderSecretRecord> {
        let encrypted_secret_json =
            encrypt_secret_json(&input.plaintext_secret_json, &input.master_key)?;
        let row = sqlx::query(
            r#"
            insert into network_egress_provider_secrets (
                provider_id, secret_ref, encrypted_secret_json, secret_version
            )
            select id, $2, $3, $4
            from network_egress_providers
            where id = $1
              and secret_ref = $2
            on conflict (provider_id) do update
            set secret_ref = excluded.secret_ref,
                encrypted_secret_json = excluded.encrypted_secret_json,
                secret_version = excluded.secret_version,
                updated_at = now()
            returning provider_id, secret_ref, encrypted_secret_json, secret_version, updated_at
            "#,
        )
        .bind(input.provider_id)
        .bind(&input.secret_ref)
        .bind(&encrypted_secret_json)
        .bind(input.secret_version)
        .fetch_optional(self.pool())
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "network_egress_provider_secret_ref",
        ))?;
        Ok(provider_secret(row))
    }

    async fn resolve_network_egress_provider_secret_json(
        &self,
        provider_id: Uuid,
        secret_ref: &str,
        master_key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query(
            r#"
            select secret.encrypted_secret_json
            from network_egress_provider_secrets secret
            join network_egress_providers provider on provider.id = secret.provider_id
            where secret.provider_id = $1
              and secret.secret_ref = $2
              and provider.secret_ref = $2
            "#,
        )
        .bind(provider_id)
        .bind(secret_ref)
        .fetch_optional(self.pool())
        .await?;
        row.map(|row| decrypt_secret_json(&row.get("encrypted_secret_json"), master_key))
            .transpose()
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        PgControlPlaneStore::append_audit_log(self, event).await
    }
}

#[async_trait]
impl NetworkEgressPoolRepository for PgControlPlaneStore {
    async fn get_network_egress_pool(
        &self,
        pool_id: Uuid,
    ) -> Result<Option<domain::NetworkEgressPool>> {
        sqlx::query("select * from network_egress_pools where id = $1")
            .bind(pool_id)
            .fetch_optional(self.pool())
            .await?
            .map(pool)
            .transpose()
    }

    async fn list_network_egress_pools(&self) -> Result<Vec<domain::NetworkEgressPool>> {
        sqlx::query("select * from network_egress_pools order by display_name asc, id asc")
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(pool)
            .collect()
    }

    async fn create_network_egress_pool(
        &self,
        input: &CreateNetworkEgressPoolInput,
    ) -> Result<domain::NetworkEgressPool> {
        let row = sqlx::query(
            r#"
                insert into network_egress_pools (
                    id, scope_id, display_name, selection_strategy, created_by, updated_by
                ) values ($1, $2, $3, 'healthy_first', $4, $4)
                returning *
            "#,
        )
        .bind(input.pool_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(&input.display_name)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;
        pool(row)
    }

    async fn update_network_egress_pool(
        &self,
        input: &UpdateNetworkEgressPoolInput,
    ) -> Result<domain::NetworkEgressPool> {
        let row = sqlx::query(
            r#"
                update network_egress_pools
                set display_name = $2, updated_by = $3, updated_at = now()
                where id = $1
                returning *
            "#,
        )
        .bind(input.pool_id)
        .bind(&input.display_name)
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(pool).transpose()?.ok_or_else(|| {
            anyhow::anyhow!(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_pool"
            ))
        })
    }

    async fn delete_network_egress_pool(&self, pool_id: Uuid) -> Result<()> {
        let result = sqlx::query("delete from network_egress_pools where id = $1")
            .bind(pool_id)
            .execute(self.pool())
            .await?;
        if result.rows_affected() == 0 {
            return Err(
                control_plane::errors::ControlPlaneError::NotFound("network_egress_pool").into(),
            );
        }
        Ok(())
    }

    async fn list_network_egress_pool_members(
        &self,
        pool_id: Uuid,
    ) -> Result<Vec<domain::NetworkEgressPoolMember>> {
        Ok(sqlx::query(
            r#"
                select * from network_egress_pool_members
                where pool_id = $1
                order by sequence asc, id asc
            "#,
        )
        .bind(pool_id)
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(pool_member)
        .collect())
    }

    async fn create_network_egress_pool_member(
        &self,
        input: &CreateNetworkEgressPoolMemberInput,
    ) -> Result<domain::NetworkEgressPoolMember> {
        let row = sqlx::query(
            r#"
                insert into network_egress_pool_members (
                    id, pool_id, provider_id, provider_egress_key, enabled, sequence,
                    created_by, updated_by
                ) values ($1, $2, $3, $4, $5, $6, $7, $7)
                returning *
            "#,
        )
        .bind(input.member_id)
        .bind(input.pool_id)
        .bind(input.provider_id)
        .bind(&input.provider_egress_key)
        .bind(input.enabled)
        .bind(input.sequence)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;
        Ok(pool_member(row))
    }

    async fn update_network_egress_pool_member(
        &self,
        input: &UpdateNetworkEgressPoolMemberInput,
    ) -> Result<domain::NetworkEgressPoolMember> {
        let row = sqlx::query(
            r#"
                update network_egress_pool_members
                set enabled = $3, sequence = $4, updated_by = $5, updated_at = now()
                where pool_id = $1 and id = $2
                returning *
            "#,
        )
        .bind(input.pool_id)
        .bind(input.member_id)
        .bind(input.enabled)
        .bind(input.sequence)
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(pool_member).ok_or_else(|| {
            anyhow::anyhow!(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_pool_member"
            ))
        })
    }

    async fn delete_network_egress_pool_member(
        &self,
        pool_id: Uuid,
        member_id: Uuid,
    ) -> Result<()> {
        let result =
            sqlx::query("delete from network_egress_pool_members where pool_id = $1 and id = $2")
                .bind(pool_id)
                .bind(member_id)
                .execute(self.pool())
                .await?;
        if result.rows_affected() == 0 {
            return Err(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_pool_member",
            )
            .into());
        }
        Ok(())
    }
}

#[async_trait]
impl NetworkEgressRouteRepository for PgControlPlaneStore {
    async fn list_network_egress_routes(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::NetworkEgressRoute>> {
        sqlx::query(
            r#"
                select * from network_egress_routes
                where workspace_id = $1
                order by consumer_kind asc, consumer_reference asc nulls first, id asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(route)
        .collect()
    }

    async fn create_network_egress_route(
        &self,
        input: &CreateNetworkEgressRouteInput,
    ) -> Result<domain::NetworkEgressRoute> {
        let row = sqlx::query(
            r#"
                insert into network_egress_routes (
                    id, workspace_id, consumer_kind, consumer_reference, pool_id, enabled,
                    failure_policy, created_by, updated_by
                ) values ($1, $2, $3, $4, $5, $6, 'block', $7, $7)
                returning *
            "#,
        )
        .bind(input.route_id)
        .bind(input.workspace_id)
        .bind(input.selector.consumer_kind())
        .bind(input.selector.consumer_reference())
        .bind(input.pool_id)
        .bind(input.enabled)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;
        route(row)
    }

    async fn update_network_egress_route(
        &self,
        input: &UpdateNetworkEgressRouteInput,
    ) -> Result<domain::NetworkEgressRoute> {
        let row = sqlx::query(
            r#"
                update network_egress_routes
                set pool_id = $3, enabled = $4, updated_by = $5, updated_at = now()
                where workspace_id = $1 and id = $2
                returning *
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.route_id)
        .bind(input.pool_id)
        .bind(input.enabled)
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(route).transpose()?.ok_or_else(|| {
            anyhow::anyhow!(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_route"
            ))
        })
    }

    async fn delete_network_egress_route(&self, workspace_id: Uuid, route_id: Uuid) -> Result<()> {
        let result =
            sqlx::query("delete from network_egress_routes where workspace_id = $1 and id = $2")
                .bind(workspace_id)
                .bind(route_id)
                .execute(self.pool())
                .await?;
        if result.rows_affected() == 0 {
            return Err(
                control_plane::errors::ControlPlaneError::NotFound("network_egress_route").into(),
            );
        }
        Ok(())
    }

    async fn find_enabled_network_egress_route(
        &self,
        workspace_id: Uuid,
        selector: &domain::NetworkEgressConsumerSelector,
    ) -> Result<Option<domain::NetworkEgressRoute>> {
        sqlx::query(
            r#"
                select * from network_egress_routes
                where workspace_id = $1
                  and consumer_kind = $2
                  and consumer_reference is not distinct from $3
                  and enabled = true
            "#,
        )
        .bind(workspace_id)
        .bind(selector.consumer_kind())
        .bind(selector.consumer_reference())
        .fetch_optional(self.pool())
        .await?
        .map(route)
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane::{
        network_egress_pool::{CreateNetworkEgressPoolMemberCommand, NetworkEgressPoolService},
        ports::{
            CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
            CreateNetworkEgressRouteInput, NetworkEgressPoolRepository,
            NetworkEgressRouteRepository, PluginRepository, UpsertPluginInstallationInput,
        },
    };
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
        let plaintext = json!({ "token": "registry-secret-must-not-persist-in-cleartext" });
        let secret = NetworkEgressRepository::upsert_network_egress_provider_secret(
            &store,
            &UpsertNetworkEgressProviderSecretInput {
                provider_id,
                secret_ref: "secret://system/network-egress/fixture".to_string(),
                plaintext_secret_json: plaintext.clone(),
                master_key: "network-egress-test-master-key".to_string(),
                secret_version: 1,
            },
        )
        .await
        .expect("registry-owned secret should persist encrypted");
        assert!(!secret
            .encrypted_secret_json
            .to_string()
            .contains("registry-secret-must-not-persist-in-cleartext"));
        let resolved = NetworkEgressRepository::resolve_network_egress_provider_secret_json(
            &store,
            provider_id,
            "secret://system/network-egress/fixture",
            "network-egress-test-master-key",
        )
        .await
        .expect("secret should resolve with the provisioning key");
        assert_eq!(resolved, Some(plaintext));
        let mismatched_ref = NetworkEgressRepository::resolve_network_egress_provider_secret_json(
            &store,
            provider_id,
            "secret://system/network-egress/wrong-ref",
            "network-egress-test-master-key",
        )
        .await
        .expect("mismatched reference should not decrypt any material");
        assert!(mismatched_ref.is_none());
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

    #[tokio::test]
    async fn ac_007_pool_member_preserves_missing_provider_reference_without_runtime_lease_fields()
    {
        let (store, actor) = store().await;
        let pool_id = Uuid::now_v7();
        NetworkEgressPoolRepository::create_network_egress_pool(
            &store,
            &CreateNetworkEgressPoolInput {
                pool_id,
                display_name: "Stable references".to_string(),
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("pool should persist");
        let provider_id = Uuid::now_v7();
        let member_id = Uuid::now_v7();
        NetworkEgressPoolRepository::create_network_egress_pool_member(
            &store,
            &CreateNetworkEgressPoolMemberInput {
                member_id,
                pool_id,
                provider_id,
                provider_egress_key: "gone-egress".to_string(),
                enabled: true,
                sequence: 10,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("durable reference should persist even when a provider later disappears");

        let duplicate = NetworkEgressPoolRepository::create_network_egress_pool_member(
            &store,
            &CreateNetworkEgressPoolMemberInput {
                member_id: Uuid::now_v7(),
                pool_id,
                provider_id,
                provider_egress_key: "gone-egress".to_string(),
                enabled: true,
                sequence: 20,
                actor_user_id: actor.id,
            },
        )
        .await;
        assert!(
            duplicate.is_err(),
            "stable pool references must be unique per pool"
        );

        let view = NetworkEgressPoolService::new(store.clone())
            .list()
            .await
            .expect("missing provider should project instead of discarding the member");
        assert_eq!(view[0].members[0].member.id, member_id);
        assert_eq!(
            view[0].members[0].health,
            domain::NetworkEgressPoolMemberHealth::Invalid
        );

        let columns = sqlx::query_scalar::<_, String>(
            r#"
                select column_name from information_schema.columns
                where table_schema = current_schema()
                    and table_name = 'network_egress_pool_members'
                order by column_name
            "#,
        )
        .fetch_all(store.pool())
        .await
        .expect("pool member columns should be readable");
        for forbidden in ["http_proxy_url", "port", "lease_id", "cleanup_token"] {
            assert!(
                !columns.iter().any(|column| column == forbidden),
                "pool members must not persist runtime lease field {forbidden}"
            );
        }
    }

    /// AC-006/007: a pool stores only durable provider/key references. Runtime proxy material is
    /// acquired after this selection, so an unavailable member is skipped and a stale descriptor
    /// cannot be selected after a later provider synchronization replaces the projection.
    #[tokio::test]
    async fn ac_006_ac_007_pool_selection_uses_current_healthy_projection_and_never_persists_lease()
    {
        let (store, actor) = store().await;
        let installation_id = Uuid::now_v7();
        PluginRepository::upsert_installation(
            &store,
            &UpsertPluginInstallationInput {
                installation_id,
                category: ExtensionCategory::RuntimeExtensions,
                organization: "test".to_string(),
                provider_code: "selection_fixture".to_string(),
                plugin_id: "selection_fixture@0.1.0".to_string(),
                plugin_version: "0.1.0".to_string(),
                contract_version: plugin_framework::NETWORK_EGRESS_PROVIDER_CONTRACT.to_string(),
                protocol: "stdio_json_worker".to_string(),
                display_name: "Selection fixture".to_string(),
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
        .expect("fixture installation should persist");
        let provider_id = Uuid::now_v7();
        let created = NetworkEgressRepository::create_network_egress_provider(
            &store,
            &CreateNetworkEgressProviderInput {
                provider_id,
                installation_id,
                provider_code: "selection_fixture".to_string(),
                display_name: "Selection fixture".to_string(),
                secret_ref: "secret://system/network-egress/selection".to_string(),
                lifecycle: domain::NetworkEgressProviderLifecycle::Draft,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("provider configuration should persist as draft");
        assert_eq!(
            created.lifecycle,
            domain::NetworkEgressProviderLifecycle::Draft
        );
        let started = NetworkEgressRepository::update_network_egress_provider_lifecycle(
            &store,
            &UpdateNetworkEgressProviderLifecycleInput {
                provider_id,
                lifecycle: domain::NetworkEgressProviderLifecycle::Active,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("provider start should persist its active lifecycle");
        assert_eq!(
            started.lifecycle,
            domain::NetworkEgressProviderLifecycle::Active
        );

        let synced_at = OffsetDateTime::now_utc();
        NetworkEgressRepository::replace_network_egress_projection(
            &store,
            &ReplaceNetworkEgressProjectionInput {
                provider_id,
                health_status: domain::NetworkEgressHealthStatus::Healthy,
                last_sync_error: None,
                synchronized_at: synced_at,
                actor_user_id: actor.id,
                egresses: vec![
                    domain::NetworkEgressProjectionRecord {
                        provider_id,
                        provider_egress_key: "unavailable-first".to_string(),
                        display_name: "Unavailable first".to_string(),
                        region: Some("test-a".to_string()),
                        tags: vec!["fixture".to_string()],
                        availability: "unavailable".to_string(),
                        synced_at,
                    },
                    domain::NetworkEgressProjectionRecord {
                        provider_id,
                        provider_egress_key: "available-second".to_string(),
                        display_name: "Available second".to_string(),
                        region: Some("test-b".to_string()),
                        tags: vec!["fixture".to_string()],
                        availability: "available".to_string(),
                        synced_at,
                    },
                ],
            },
        )
        .await
        .expect("provider synchronization should persist the stable projection");

        let pool = NetworkEgressPoolService::new(store.clone())
            .create(
                control_plane::network_egress_pool::CreateNetworkEgressPoolCommand {
                    actor_user_id: actor.id,
                    display_name: "Healthy first".to_string(),
                },
            )
            .await
            .expect("pool should persist");
        assert_eq!(pool.pool.selection_strategy.as_str(), "healthy_first");
        let pool_id = pool.pool.id;
        let unavailable = NetworkEgressPoolService::new(store.clone())
            .add_member(CreateNetworkEgressPoolMemberCommand {
                actor_user_id: actor.id,
                pool_id,
                provider_id,
                provider_egress_key: "unavailable-first".to_string(),
                enabled: true,
                sequence: 0,
            })
            .await
            .expect("current but unavailable descriptor may be retained as a durable reference");
        let available = NetworkEgressPoolService::new(store.clone())
            .add_member(CreateNetworkEgressPoolMemberCommand {
                actor_user_id: actor.id,
                pool_id,
                provider_id,
                provider_egress_key: "available-second".to_string(),
                enabled: true,
                sequence: 10,
            })
            .await
            .expect("current available descriptor should join the pool");
        assert_eq!(unavailable.health.as_str(), "unhealthy");
        assert_eq!(available.health.as_str(), "healthy");

        let selection = NetworkEgressPoolService::new(store.clone())
            .select_healthy_first(pool_id)
            .await
            .expect("selection should skip unhealthy members and choose the durable key");
        assert_eq!(selection.member_id, available.member.id);
        assert_eq!(selection.provider_id, provider_id);
        assert_eq!(selection.provider_egress_key, "available-second");

        NetworkEgressRepository::replace_network_egress_projection(
            &store,
            &ReplaceNetworkEgressProjectionInput {
                provider_id,
                health_status: domain::NetworkEgressHealthStatus::Healthy,
                last_sync_error: None,
                synchronized_at: OffsetDateTime::now_utc(),
                actor_user_id: actor.id,
                egresses: vec![],
            },
        )
        .await
        .expect("a later sync may invalidate a formerly current descriptor");
        let views = NetworkEgressPoolService::new(store.clone())
            .list()
            .await
            .expect("stale references should remain visible for correction");
        assert!(views[0]
            .members
            .iter()
            .all(|member| member.health == domain::NetworkEgressPoolMemberHealth::Invalid));
        let unavailable_error = NetworkEgressPoolService::new(store.clone())
            .select_healthy_first(pool_id)
            .await
            .expect_err("a stale projection must fail closed before any runtime lease is acquired");
        assert!(unavailable_error
            .to_string()
            .contains("network_egress_pool_unavailable"));

        let stopped = NetworkEgressRepository::update_network_egress_provider_lifecycle(
            &store,
            &UpdateNetworkEgressProviderLifecycleInput {
                provider_id,
                lifecycle: domain::NetworkEgressProviderLifecycle::Disabled,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("provider stop should persist its disabled lifecycle");
        assert_eq!(
            stopped.lifecycle,
            domain::NetworkEgressProviderLifecycle::Disabled
        );

        let columns = sqlx::query_scalar::<_, String>(
            r#"
                select column_name from information_schema.columns
                where table_schema = current_schema()
                    and table_name in ('network_egress_pool_members', 'network_egress_projections')
                order by column_name
            "#,
        )
        .fetch_all(store.pool())
        .await
        .expect("network center storage columns should be readable");
        for forbidden in ["http_proxy_url", "port", "lease_id", "cleanup_token"] {
            assert!(
                !columns.iter().any(|column| column == forbidden),
                "durable control-plane records must not persist runtime lease field {forbidden}"
            );
        }
    }

    #[tokio::test]
    async fn nc_09_route_storage_keeps_closed_selector_identity_and_workspace_instance_boundary() {
        let (store, actor) = store().await;
        let workspace_id = sqlx::query_scalar::<_, Uuid>(
            "select id from workspaces where name = 'network-egress'",
        )
        .fetch_one(store.pool())
        .await
        .expect("fixture workspace should exist");
        let pool_id = Uuid::now_v7();
        NetworkEgressPoolRepository::create_network_egress_pool(
            &store,
            &CreateNetworkEgressPoolInput {
                pool_id,
                display_name: "Route target".to_string(),
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("route target pool should persist");

        let route = NetworkEgressRouteRepository::create_network_egress_route(
            &store,
            &CreateNetworkEgressRouteInput {
                route_id: Uuid::now_v7(),
                workspace_id,
                selector: domain::NetworkEgressConsumerSelector::GithubOfficialSources,
                pool_id,
                enabled: true,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("github selector should persist without a free reference");
        assert_eq!(
            route.selector,
            domain::NetworkEgressConsumerSelector::GithubOfficialSources
        );

        let duplicate = NetworkEgressRouteRepository::create_network_egress_route(
            &store,
            &CreateNetworkEgressRouteInput {
                route_id: Uuid::now_v7(),
                workspace_id,
                selector: domain::NetworkEgressConsumerSelector::GithubOfficialSources,
                pool_id,
                enabled: false,
                actor_user_id: actor.id,
            },
        )
        .await;
        assert!(
            duplicate.is_err(),
            "one workspace can have only one github selector"
        );

        let unknown_instance = sqlx::query(
            r#"
                insert into network_egress_routes (
                    id, workspace_id, consumer_kind, consumer_reference, pool_id, enabled,
                    failure_policy, created_by, updated_by
                ) values ($1, $2, 'model_provider', $3, $4, true, 'block', $5, $5)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(workspace_id)
        .bind(Uuid::now_v7())
        .bind(pool_id)
        .bind(actor.id)
        .execute(store.pool())
        .await;
        assert!(
            unknown_instance.is_err(),
            "an exact model provider selector must belong to the route workspace"
        );
    }
}
