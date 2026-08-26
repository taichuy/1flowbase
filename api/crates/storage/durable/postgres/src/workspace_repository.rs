use anyhow::Result;
use async_trait::async_trait;
use control_plane::ports::WorkspaceRepository;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::workspace_mapper::{PgWorkspaceMapper, StoredWorkspaceRow},
    repositories::{is_root_user, PgControlPlaneStore},
};

fn map_workspace_record(row: sqlx::postgres::PgRow) -> domain::WorkspaceRecord {
    PgWorkspaceMapper::to_workspace_record(StoredWorkspaceRow {
        id: row.get("id"),
        tenant_id: row.get("tenant_id"),
        name: row.get("name"),
        logo_url: row.get("logo_url"),
        introduction: row.get("introduction"),
    })
}

#[async_trait]
impl WorkspaceRepository for PgControlPlaneStore {
    async fn get_workspace(&self, workspace_id: Uuid) -> Result<Option<domain::WorkspaceRecord>> {
        let row = sqlx::query(
            "select id, tenant_id, name, logo_url, introduction from workspaces where id = $1",
        )
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_workspace_record))
    }

    async fn list_accessible_workspaces(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<domain::WorkspaceRecord>> {
        let is_root = match self.request_actor_for_user(user_id)? {
            Some(actor) => actor.is_root,
            None => is_root_user(self.pool(), user_id).await?,
        };
        let rows = if is_root {
            sqlx::query(
                r#"
                select id, tenant_id, name, logo_url, introduction
                from workspaces
                order by lower(name), created_at asc, id asc
                "#,
            )
            .fetch_all(self.pool())
            .await?
        } else {
            sqlx::query(
                r#"
                select w.id, w.tenant_id, w.name, w.logo_url, w.introduction
                from workspaces w
                where exists (
                  select 1
                  from workspace_memberships wm
                  where wm.workspace_id = w.id
                    and wm.user_id = $1
                )
                order by lower(w.name), w.created_at asc, w.id asc
                "#,
            )
            .bind(user_id)
            .fetch_all(self.pool())
            .await?
        };

        Ok(rows.into_iter().map(map_workspace_record).collect())
    }

    async fn get_accessible_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Option<domain::WorkspaceRecord>> {
        let row = if is_root_user(self.pool(), user_id).await? {
            sqlx::query(
                r#"
                select id, tenant_id, name, logo_url, introduction
                from workspaces
                where id = $1
                "#,
            )
            .bind(workspace_id)
            .fetch_optional(self.pool())
            .await?
        } else {
            sqlx::query(
                r#"
                select w.id, w.tenant_id, w.name, w.logo_url, w.introduction
                from workspaces w
                where w.id = $2
                  and exists (
                    select 1
                    from workspace_memberships wm
                    where wm.workspace_id = w.id
                      and wm.user_id = $1
                  )
                "#,
            )
            .bind(user_id)
            .bind(workspace_id)
            .fetch_optional(self.pool())
            .await?
        };

        Ok(row.map(map_workspace_record))
    }

    async fn update_workspace(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        name: &str,
        logo_url: Option<&str>,
        introduction: &str,
    ) -> Result<domain::WorkspaceRecord> {
        let row = sqlx::query(
            r#"
            update workspaces
            set name = $2,
                logo_url = $3,
                introduction = $4,
                updated_by = $5,
                updated_at = now()
            where id = $1
            returning id, tenant_id, name, logo_url, introduction
            "#,
        )
        .bind(workspace_id)
        .bind(name)
        .bind(logo_url)
        .bind(introduction)
        .bind(actor_user_id)
        .fetch_one(self.pool())
        .await?;

        Ok(map_workspace_record(row))
    }
}
