use crate::PgControlPlaneStore;
use async_trait::async_trait;
use control_plane_contracts::ports::EventCatalogRepository;
use domain::{event_center::*, ActorContext, ScopeContext};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use time::OffsetDateTime;

/// Used by both bootstrap and the plugin authority commit. An unpublished candidate cannot
/// alter the directory by preparing it: this function only runs inside the owner's commit.
pub(crate) async fn synchronize_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    snapshot: &EventCatalogSnapshot,
) -> anyhow::Result<()> {
    snapshot.validate().map_err(anyhow::Error::msg)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('1flowbase.event_catalog', 0))")
        .execute(&mut **transaction)
        .await?;
    let mut entries = Vec::with_capacity(snapshot.entries.len());
    for entry in &snapshot.entries {
        let mut value = serde_json::to_value(entry)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("invalid event catalog definition"))?;
        object.remove("created_at");
        object.remove("updated_at");
        entries.push(value);
    }
    let entries = Value::Array(entries);
    let scopes = serde_json::to_value(&snapshot.scopes)?;
    let now = OffsetDateTime::now_utc();
    sqlx::query(
        "INSERT INTO event_catalog(metadata, created_at, updated_at) \
         SELECT value, $2, $2 FROM jsonb_array_elements($1::jsonb) \
         ON CONFLICT (identity) DO UPDATE SET metadata=EXCLUDED.metadata, available=true, updated_at=EXCLUDED.updated_at \
         WHERE event_catalog.metadata IS DISTINCT FROM EXCLUDED.metadata OR NOT event_catalog.available",
    ).bind(&entries).bind(now).execute(&mut **transaction).await?;
    sqlx::query(
        "UPDATE event_catalog SET available=false, updated_at=$3 \
         WHERE origin IN ('core','plugin') AND available \
         AND metadata->'scope' IN (SELECT value FROM jsonb_array_elements($1::jsonb)) \
         AND identity NOT IN (SELECT event_catalog_identity(value) FROM jsonb_array_elements($2::jsonb))",
    ).bind(scopes).bind(entries).bind(now).execute(&mut **transaction).await?;
    Ok(())
}

#[async_trait]
impl EventCatalogRepository for PgControlPlaneStore {
    async fn event_catalog_workspaces(&self) -> anyhow::Result<Vec<ScopeContext>> {
        sqlx::query("SELECT tenant_id, id FROM workspaces ORDER BY id")
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(|row| {
                Ok(ScopeContext {
                    tenant_id: row.try_get("tenant_id")?,
                    workspace_id: row.try_get("id")?,
                })
            })
            .collect()
    }

    async fn synchronize_event_catalog(
        &self,
        snapshot: &EventCatalogSnapshot,
    ) -> anyhow::Result<()> {
        let mut transaction = self.pool().begin().await?;
        synchronize_in_transaction(&mut transaction, snapshot).await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn list_event_catalog(
        &self,
        actor: &ActorContext,
        query: EventCatalogQuery,
    ) -> anyhow::Result<Vec<EventCatalogEntry>> {
        let field = match query.sort_by {
            EventCatalogSortBy::CreatedAt => "created_at",
            EventCatalogSortBy::UpdatedAt => "updated_at",
        };
        let order = match query.sort_order {
            EventCatalogSortOrder::Asc => "ASC",
            EventCatalogSortOrder::Desc => "DESC",
        };
        let sql = format!(
            "SELECT metadata, available, created_at, updated_at FROM event_catalog \
             WHERE (scope_kind='workspace' AND scope_id=$1) OR ($2 AND scope_kind IN ('system','unbound')) \
             ORDER BY {field} {order}, metadata->>'contract_id', identity"
        );
        // Both interpolated fragments come exclusively from the typed enum matches above.
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(actor.current_workspace_id)
            .bind(actor.is_root)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(|row| {
                let mut entry: EventCatalogEntry =
                    serde_json::from_value(row.try_get("metadata")?)?;
                entry.created_at = Some(row.try_get("created_at")?);
                entry.updated_at = Some(row.try_get("updated_at")?);
                if !row.try_get::<bool, _>("available")? {
                    entry.enabled = Some(false);
                    entry.supported_phases.clear();
                }
                Ok(entry)
            })
            .collect()
    }
}
