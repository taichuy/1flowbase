use anyhow::{bail, Result};
use async_trait::async_trait;
use control_plane::ports::{
    CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
    CreateNetworkEgressProviderInput, CreateNetworkEgressRouteInput,
    CreateStaticHttpProxyPoolMemberInput, NetworkEgressPoolRepository, NetworkEgressRepository,
    NetworkEgressRouteRepository, RecordNetworkEgressPoolMemberProbeInput,
    RecordNetworkEgressSyncFailureInput, ReplaceNetworkEgressProjectionInput,
    UpdateNetworkEgressPoolInput, UpdateNetworkEgressPoolMemberInput,
    UpdateNetworkEgressProviderLifecycleInput, UpdateNetworkEgressRouteInput,
    UpsertNetworkEgressProviderSecretInput,
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
    let extension_category: Option<String> = row.get("extension_category");
    let extension_organization: Option<String> = row.get("extension_organization");
    let extension_artifact_id: Option<String> = row.get("extension_artifact_id");
    let extension_family = match (
        extension_category,
        extension_organization,
        extension_artifact_id,
    ) {
        (None, None, None) => None,
        (Some(category), Some(organization), Some(artifact_id)) => {
            let category = domain::ExtensionCategory::parse(&category)
                .ok_or_else(|| anyhow::anyhow!("invalid network egress extension category"))?;
            Some(
                domain::ExtensionCatalogIdentity::new(category, organization, artifact_id)
                    .ok_or_else(|| anyhow::anyhow!("invalid network egress extension family"))?,
            )
        }
        _ => bail!("incomplete network egress extension family"),
    };
    Ok(domain::NetworkEgressProviderRecord {
        id: row.get("id"),
        extension_family,
        provider_code: row.get("provider_code"),
        display_name: row.get("display_name"),
        description: row.get("description"),
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

fn pool_member_probe_status(value: &str) -> Result<domain::NetworkEgressPoolMemberProbeStatus> {
    match value {
        "not_tested" => Ok(domain::NetworkEgressPoolMemberProbeStatus::NotTested),
        "succeeded" => Ok(domain::NetworkEgressPoolMemberProbeStatus::Succeeded),
        "failed" => Ok(domain::NetworkEgressPoolMemberProbeStatus::Failed),
        _ => bail!("invalid network egress pool member probe status"),
    }
}

fn pool(row: sqlx::postgres::PgRow) -> Result<domain::NetworkEgressPool> {
    Ok(domain::NetworkEgressPool {
        id: row.get("id"),
        display_name: row.get("display_name"),
        owner_provider_id: row.get("owner_provider_id"),
        selection_strategy: selection_strategy(
            row.get::<String, _>("selection_strategy").as_str(),
        )?,
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn pool_member(row: sqlx::postgres::PgRow) -> Result<domain::NetworkEgressPoolMember> {
    Ok(domain::NetworkEgressPoolMember {
        id: row.get("id"),
        pool_id: row.get("pool_id"),
        provider_id: row.get("provider_id"),
        provider_egress_key: row.get("provider_egress_key"),
        enabled: row.get("enabled"),
        sequence: row.get("sequence"),
        probe_status: pool_member_probe_status(row.get::<String, _>("probe_status").as_str())?,
        probe_http_status: pool_member_probe_status(
            row.get::<String, _>("probe_http_status").as_str(),
        )?,
        probe_https_status: pool_member_probe_status(
            row.get::<String, _>("probe_https_status").as_str(),
        )?,
        probe_latency_ms: row.get::<i32, _>("probe_latency_ms"),
        probe_exit_ip: row.get("probe_exit_ip"),
        probe_exit_region: row.get("probe_exit_region"),
        probe_error_code: row.get("probe_error_code"),
        last_probed_at: row.get("last_probed_at"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
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
        pool_member_ids: row.get("pool_member_ids"),
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
                id, scope_id, extension_category, extension_organization, extension_artifact_id,
                provider_code, display_name, description, secret_ref,
                lifecycle, health_status, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'unknown', $11, $11)
            returning *
        "#,
        )
        .bind(input.provider_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(
            input
                .extension_family
                .as_ref()
                .map(|family| family.category().as_str()),
        )
        .bind(
            input
                .extension_family
                .as_ref()
                .map(domain::ExtensionCatalogIdentity::organization),
        )
        .bind(
            input
                .extension_family
                .as_ref()
                .map(domain::ExtensionCatalogIdentity::artifact_id),
        )
        .bind(&input.provider_code)
        .bind(&input.display_name)
        .bind(&input.description)
        .bind(&input.secret_ref)
        .bind(input.lifecycle.as_str())
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;
        provider(row)
    }

    async fn delete_network_egress_provider(&self, provider_id: Uuid) -> Result<()> {
        let result = sqlx::query("delete from network_egress_providers where id = $1")
            .bind(provider_id)
            .execute(self.pool())
            .await?;
        if result.rows_affected() == 0 {
            return Err(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_provider",
            )
            .into());
        }
        Ok(())
    }

    async fn create_static_http_proxy_pool_member(
        &self,
        input: &CreateStaticHttpProxyPoolMemberInput,
    ) -> Result<domain::NetworkEgressPoolMember> {
        let encrypted_secret_json =
            encrypt_secret_json(&input.plaintext_secret_json, &input.master_key)?;
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            r#"
            insert into network_egress_providers (
                id, scope_id, extension_category, extension_organization, extension_artifact_id,
                provider_code, display_name, description, secret_ref,
                lifecycle, health_status, created_by, updated_by
            ) values (
                $1, $2, null, null, null, 'builtin_static_http', $3, $4, $5,
                'active', 'healthy', $6, $6
            )
        "#,
        )
        .bind(input.provider_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(&input.display_name)
        .bind(&input.description)
        .bind(&input.secret_ref)
        .bind(input.actor_user_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            insert into network_egress_provider_secrets (
                provider_id, secret_ref, encrypted_secret_json, secret_version
            ) values ($1, $2, $3, 1)
        "#,
        )
        .bind(input.provider_id)
        .bind(&input.secret_ref)
        .bind(&encrypted_secret_json)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            insert into network_egress_projections (
                provider_id, provider_egress_key, display_name, region, tags, availability, synced_at
            ) values ($1, 'static-http', $2, null, array['static', 'http'], 'available', $3)
        "#,
        )
        .bind(input.provider_id)
        .bind(&input.display_name)
        .bind(input.synchronized_at)
        .execute(&mut *transaction)
        .await?;
        let row = sqlx::query(
            r#"
            insert into network_egress_pool_members (
                id, pool_id, provider_id, provider_egress_key, enabled, sequence,
                created_by, updated_by
            ) values ($1, $2, $3, 'static-http', $4, $5, $6, $6)
            returning *
        "#,
        )
        .bind(input.member_id)
        .bind(input.pool_id)
        .bind(input.provider_id)
        .bind(input.enabled)
        .bind(input.sequence)
        .bind(input.actor_user_id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        pool_member(row)
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
                id, scope_id, display_name, owner_provider_id, selection_strategy, created_by, updated_by
                ) values ($1, $2, $3, $4, 'healthy_first', $5, $5)
                returning *
            "#,
        )
        .bind(input.pool_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(&input.display_name)
        .bind(input.owner_provider_id)
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
        .collect::<Result<Vec<_>>>()?)
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
        pool_member(row)
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
        row.map(pool_member).transpose()?.ok_or_else(|| {
            anyhow::anyhow!(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_pool_member"
            ))
        })
    }

    async fn record_network_egress_pool_member_probe(
        &self,
        input: &RecordNetworkEgressPoolMemberProbeInput,
    ) -> Result<domain::NetworkEgressPoolMember> {
        let row = sqlx::query(
            r#"
                update network_egress_pool_members
                set probe_status = $3, probe_http_status = $4, probe_https_status = $5,
                    probe_latency_ms = $6, probe_exit_ip = $7, probe_exit_region = $8,
                    probe_error_code = $9, last_probed_at = $10, updated_by = $11, updated_at = now()
                where pool_id = $1 and id = $2
                returning *
            "#,
        )
        .bind(input.pool_id)
        .bind(input.member_id)
        .bind(input.status.as_str())
        .bind(input.http_status.as_str())
        .bind(input.https_status.as_str())
        .bind(input.latency_ms)
        .bind(&input.exit_ip)
        .bind(&input.exit_region)
        .bind(&input.error_code)
        .bind(input.probed_at)
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(pool_member).transpose()?.ok_or_else(|| {
            anyhow::anyhow!(control_plane::errors::ControlPlaneError::NotFound(
                "network_egress_pool_member",
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
                .await
                .map_err(|error| match &error {
                    sqlx::Error::Database(database_error)
                        if database_error.constraint()
                            == Some("network_egress_route_pool_members_member_fk") =>
                    {
                        anyhow::Error::new(control_plane::errors::ControlPlaneError::Conflict(
                            "network_egress_pool_member_in_use",
                        ))
                    }
                    _ => error.into(),
                })?;
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
                select routes.*,
                    array(
                        select mapping.pool_member_id
                        from network_egress_route_pool_members mapping
                        where mapping.route_id = routes.id
                        order by mapping.sequence asc
                    ) as pool_member_ids
                from network_egress_routes routes
                where routes.workspace_id = $1
                order by routes.consumer_kind asc,
                    routes.consumer_reference asc nulls first,
                    routes.id asc
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
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            r#"
                insert into network_egress_routes (
                    id, workspace_id, consumer_kind, consumer_reference, pool_id, enabled,
                    failure_policy, created_by, updated_by
                ) values ($1, $2, $3, $4, $5, $6, 'block', $7, $7)
            "#,
        )
        .bind(input.route_id)
        .bind(input.workspace_id)
        .bind(input.selector.consumer_kind())
        .bind(input.selector.consumer_reference())
        .bind(input.pool_id)
        .bind(input.enabled)
        .bind(input.actor_user_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
                insert into network_egress_route_pool_members (
                    route_id, pool_member_id, sequence
                )
                select $1, mapping.pool_member_id, mapping.ordinality::integer - 1
                from unnest($2::uuid[]) with ordinality
                    as mapping(pool_member_id, ordinality)
            "#,
        )
        .bind(input.route_id)
        .bind(&input.pool_member_ids)
        .execute(&mut *transaction)
        .await?;
        let row = sqlx::query(
            r#"
                select routes.*,
                    array(
                        select mapping.pool_member_id
                        from network_egress_route_pool_members mapping
                        where mapping.route_id = routes.id
                        order by mapping.sequence asc
                    ) as pool_member_ids
                from network_egress_routes routes
                where routes.id = $1
            "#,
        )
        .bind(input.route_id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        route(row)
    }

    async fn update_network_egress_route(
        &self,
        input: &UpdateNetworkEgressRouteInput,
    ) -> Result<domain::NetworkEgressRoute> {
        let mut transaction = self.pool().begin().await?;
        let updated = sqlx::query(
            r#"
                update network_egress_routes
                set enabled = $3, updated_by = $4, updated_at = now()
                where workspace_id = $1 and id = $2
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.route_id)
        .bind(input.enabled)
        .bind(input.actor_user_id)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(anyhow::anyhow!(
                control_plane::errors::ControlPlaneError::NotFound("network_egress_route")
            ));
        }
        sqlx::query("delete from network_egress_route_pool_members where route_id = $1")
            .bind(input.route_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            r#"
                insert into network_egress_route_pool_members (
                    route_id, pool_member_id, sequence
                )
                select $1, mapping.pool_member_id, mapping.ordinality::integer - 1
                from unnest($2::uuid[]) with ordinality
                    as mapping(pool_member_id, ordinality)
            "#,
        )
        .bind(input.route_id)
        .bind(&input.pool_member_ids)
        .execute(&mut *transaction)
        .await?;
        let row = sqlx::query(
            r#"
                select routes.*,
                    array(
                        select mapping.pool_member_id
                        from network_egress_route_pool_members mapping
                        where mapping.route_id = routes.id
                        order by mapping.sequence asc
                    ) as pool_member_ids
                from network_egress_routes routes
                where routes.id = $1
            "#,
        )
        .bind(input.route_id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        route(row)
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
                select routes.*,
                    array(
                        select mapping.pool_member_id
                        from network_egress_route_pool_members mapping
                        where mapping.route_id = routes.id
                        order by mapping.sequence asc
                    ) as pool_member_ids
                from network_egress_routes routes
                where routes.workspace_id = $1
                  and routes.consumer_kind = $2
                  and routes.consumer_reference is not distinct from $3
                  and routes.enabled = true
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

    async fn is_network_egress_pool_member_referenced(&self, member_id: Uuid) -> Result<bool> {
        sqlx::query_scalar(
            "select exists(select 1 from network_egress_route_pool_members where pool_member_id = $1)",
        )
        .bind(member_id)
        .fetch_one(self.pool())
        .await
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_plane::{
        network_egress_pool::{
            ensure_global_network_egress_pool, CreateNetworkEgressPoolMemberCommand,
            NetworkEgressPoolService, GLOBAL_NETWORK_EGRESS_POOL_ID,
        },
        network_egress_route::{CreateNetworkEgressRouteCommand, NetworkEgressRouteService},
        ports::{
            CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
            CreateNetworkEgressRouteInput, CreateStaticHttpProxyPoolMemberInput,
            NetworkEgressPoolRepository, NetworkEgressRouteRepository, PluginRepository,
            UpsertPluginInstallationInput,
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
        store
            .upsert_authenticator(&domain::AuthenticatorRecord {
                id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
                auth_type: "password-local".into(),
                title: "Password".into(),
                enabled: true,
                is_builtin: true,
                sort_order: 0,
                public_ui_block: String::new(),
                options: json!({}),
            })
            .await
            .expect("password-local authenticator should seed before root identities");
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
                contract_version: extension_contracts::NETWORK_EGRESS_PROVIDER_CONTRACT.to_string(),
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
                extension_family: domain::ExtensionCatalogIdentity::new(
                    ExtensionCategory::RuntimeExtensions,
                    "test",
                    "fixture_egress",
                ),
                provider_code: "fixture_egress".to_string(),
                display_name: "Fixture egress".to_string(),
                description: String::new(),
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

        NetworkEgressRepository::delete_network_egress_provider(&store, provider_id)
            .await
            .expect("an initial failed provider must be removable with its private state");
        assert!(
            NetworkEgressRepository::get_network_egress_provider(&store, provider_id)
                .await
                .expect("deleted provider lookup should succeed")
                .is_none()
        );
        assert!(
            NetworkEgressRepository::resolve_network_egress_provider_secret_json(
                &store,
                provider_id,
                "secret://system/network-egress/fixture",
                "network-egress-test-master-key",
            )
            .await
            .expect("deleted provider secret lookup should succeed")
            .is_none()
        );
        assert!(
            NetworkEgressRepository::list_network_egress_projections(&store, provider_id)
                .await
                .expect("deleted provider projection lookup should succeed")
                .is_empty()
        );
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
                owner_provider_id: None,
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
                contract_version: extension_contracts::NETWORK_EGRESS_PROVIDER_CONTRACT.to_string(),
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
                extension_family: domain::ExtensionCatalogIdentity::new(
                    ExtensionCategory::RuntimeExtensions,
                    "test",
                    "selection_fixture",
                ),
                provider_code: "selection_fixture".to_string(),
                display_name: "Selection fixture".to_string(),
                description: String::new(),
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

        let pool = ensure_global_network_egress_pool(&store, actor.id)
            .await
            .expect("pool should persist");
        assert_eq!(pool.id, GLOBAL_NETWORK_EGRESS_POOL_ID);
        assert_eq!(pool.selection_strategy.as_str(), "healthy_first");
        let pool_id = pool.id;
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
        assert_eq!(available.health.as_str(), "not_tested");

        let selection = NetworkEgressPoolService::new(store.clone())
            .select_healthy_first(pool_id)
            .await
            .expect("an enabled untested member should remain selectable after unhealthy members");
        assert_eq!(selection.member_id, available.member.id);
        assert_eq!(selection.provider_id, provider_id);
        assert_eq!(selection.provider_egress_key, "available-second");

        let route_selection = NetworkEgressPoolService::new(store.clone())
            .select_healthy_first_from(pool_id, &[available.member.id])
            .await
            .expect("route selection must stay inside its explicit proxy mapping");
        assert_eq!(route_selection.member_id, available.member.id);
        let unavailable_route = NetworkEgressPoolService::new(store.clone())
            .select_healthy_first_from(pool_id, &[unavailable.member.id])
            .await
            .expect_err("a route must not fall through to an unbound healthy proxy");
        assert!(unavailable_route
            .to_string()
            .contains("network_egress_pool_unavailable"));

        let workspace_id = sqlx::query_scalar::<_, Uuid>(
            "select id from workspaces where name = 'network-egress'",
        )
        .fetch_one(store.pool())
        .await
        .expect("fixture workspace should exist");
        let invalid_mapping = NetworkEgressRouteService::new(store.clone())
            .create(CreateNetworkEgressRouteCommand {
                actor_user_id: actor.id,
                workspace_id,
                selector: domain::NetworkEgressConsumerSelector::ModelProviderDefault,
                pool_member_ids: vec![available.member.id, available.member.id],
                enabled: true,
            })
            .await
            .expect_err("route mappings must be non-empty and unique");
        assert!(invalid_mapping.to_string().contains("pool_member_ids"));
        let route = NetworkEgressRouteService::new(store.clone())
            .create(CreateNetworkEgressRouteCommand {
                actor_user_id: actor.id,
                workspace_id,
                selector: domain::NetworkEgressConsumerSelector::HttpNodeDefault,
                pool_member_ids: vec![available.member.id, unavailable.member.id],
                enabled: true,
            })
            .await
            .expect("ordered route proxy mapping should persist");
        assert_eq!(
            route.pool_member_ids,
            vec![available.member.id, unavailable.member.id]
        );
        assert!(
            NetworkEgressRouteRepository::is_network_egress_pool_member_referenced(
                &store,
                available.member.id
            )
            .await
            .expect("route reference lookup should succeed")
        );
        let delete_referenced = NetworkEgressPoolService::new(store.clone())
            .delete_member(actor.id, pool_id, available.member.id)
            .await
            .expect_err("a proxy referenced by a route must not be deleted");
        assert!(delete_referenced
            .to_string()
            .contains("network_egress_pool_member_in_use"));

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

    /// AC-NC16: direct pool additions create the static provider, encrypted secret, projection,
    /// and pool member as one durable unit without putting proxy material in the pool table.
    #[tokio::test]
    async fn ac_nc16_static_http_proxy_member_is_atomic_and_keeps_credentials_out_of_pool() {
        let (store, actor) = store().await;
        let pool_id = Uuid::now_v7();
        NetworkEgressPoolRepository::create_network_egress_pool(
            &store,
            &CreateNetworkEgressPoolInput {
                pool_id,
                display_name: "Manual exits".to_string(),
                owner_provider_id: None,
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("target pool should persist");
        let rolled_back_provider_id = Uuid::now_v7();
        let failed = NetworkEgressRepository::create_static_http_proxy_pool_member(
            &store,
            &CreateStaticHttpProxyPoolMemberInput {
                provider_id: rolled_back_provider_id,
                member_id: Uuid::now_v7(),
                pool_id: Uuid::now_v7(),
                display_name: "Should roll back".to_string(),
                description: String::new(),
                secret_ref: format!("secret://system/network-egress/{rolled_back_provider_id}"),
                plaintext_secret_json: json!({"host": "198.65.36.212", "port": 37867}),
                master_key: "test-master-key".to_string(),
                enabled: true,
                sequence: 0,
                synchronized_at: OffsetDateTime::now_utc(),
                actor_user_id: actor.id,
            },
        )
        .await;
        assert!(
            failed.is_err(),
            "a nonexistent pool must reject the complete write"
        );
        assert!(
            NetworkEgressRepository::get_network_egress_provider(&store, rolled_back_provider_id)
                .await
                .expect("provider lookup should succeed")
                .is_none(),
            "a failed member write must roll back its newly-created provider and secret"
        );
        let provider_id = Uuid::now_v7();
        let member = NetworkEgressRepository::create_static_http_proxy_pool_member(
            &store,
            &CreateStaticHttpProxyPoolMemberInput {
                provider_id,
                member_id: Uuid::now_v7(),
                pool_id,
                display_name: "US proxy".to_string(),
                description: "Manual proxy for US traffic".to_string(),
                secret_ref: format!("secret://system/network-egress/{provider_id}"),
                plaintext_secret_json: json!({
                    "host": "198.65.36.212",
                    "port": 37867,
                    "username": "proxy-user",
                    "password": "proxy-password",
                }),
                master_key: "test-master-key".to_string(),
                enabled: true,
                sequence: 0,
                synchronized_at: OffsetDateTime::now_utc(),
                actor_user_id: actor.id,
            },
        )
        .await
        .expect("static proxy should join its target pool atomically");
        assert_eq!(member.provider_id, provider_id);
        assert_eq!(member.provider_egress_key, "static-http");
        let provider = NetworkEgressRepository::get_network_egress_provider(&store, provider_id)
            .await
            .expect("provider lookup should succeed")
            .expect("static provider should persist");
        assert_eq!(provider.extension_family, None);
        assert_eq!(provider.provider_code, "builtin_static_http");
        assert_eq!(provider.description, "Manual proxy for US traffic");
        let secret = NetworkEgressRepository::resolve_network_egress_provider_secret_json(
            &store,
            provider_id,
            &provider.secret_ref,
            "test-master-key",
        )
        .await
        .expect("secret should decrypt")
        .expect("secret should exist");
        assert_eq!(secret["password"], "proxy-password");
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
        assert!(!columns.iter().any(|column| column == "password"));
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
                owner_provider_id: None,
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
                pool_member_ids: vec![],
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
                pool_member_ids: vec![],
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
