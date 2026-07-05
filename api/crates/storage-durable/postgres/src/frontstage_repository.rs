use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateFrontstagePageInput, FrontstagePageRepository,
        MoveFrontstagePageInput, SaveFrontstageBlockCodeInput, SaveFrontstagePageContentInput,
        UpdateFrontstagePageMetadataInput, WorkspaceRepository,
    },
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn map_frontstage_page_row(row: &sqlx::postgres::PgRow) -> Result<domain::FrontstagePageRecord> {
    let raw_kind: String = row.get("kind");
    let kind = domain::FrontstagePageKind::from_db(&raw_kind)
        .ok_or(ControlPlaneError::InvalidInput("frontstage_page_kind"))?;

    Ok(domain::FrontstagePageRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        parent_id: row.get("parent_id"),
        kind,
        title: row.get("title"),
        icon: row.get("icon"),
        tooltip: row.get("tooltip"),
        is_hidden: row.get("is_hidden"),
        slug: row.get("slug"),
        schema_root_uid: row.get("schema_root_uid"),
        rank: row.get("rank"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_frontstage_schema_row(
    row: &sqlx::postgres::PgRow,
) -> domain::frontstage::FrontstagePageSchemaRecord {
    domain::frontstage::FrontstagePageSchemaRecord {
        workspace_id: row.get("schema_workspace_id"),
        page_id: row.get("schema_page_id"),
        root_uid: row.get("root_uid"),
        schema_payload: row.get("schema_payload"),
        root_payload: row.get("root_payload"),
        created_at: row.get("schema_created_at"),
        updated_at: row.get("schema_updated_at"),
    }
}

fn map_frontstage_block_code_row(
    row: sqlx::postgres::PgRow,
) -> domain::frontstage::FrontstageBlockCodeRecord {
    domain::frontstage::FrontstageBlockCodeRecord {
        workspace_id: row.get("workspace_id"),
        page_id: row.get("page_id"),
        code_ref: row.get("code_ref"),
        code: row.get("code"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_frontstage_visibility_rule_row(
    row: &sqlx::postgres::PgRow,
) -> Result<domain::frontstage::FrontstagePageVisibilityRuleRecord> {
    let raw_visibility: String = row.get("visibility");
    let visibility = domain::frontstage::FrontstagePageVisibility::from_db(&raw_visibility).ok_or(
        ControlPlaneError::InvalidInput("frontstage_page_visibility"),
    )?;

    Ok(domain::frontstage::FrontstagePageVisibilityRuleRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        page_id: row.get("page_id"),
        role_id: row.get("role_id"),
        visibility,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[async_trait]
impl FrontstagePageRepository for PgControlPlaneStore {
    async fn load_actor_context_for_workspace(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let workspace =
            WorkspaceRepository::get_accessible_workspace(self, actor_user_id, workspace_id)
                .await?
                .ok_or(ControlPlaneError::PermissionDenied(
                    "workspace_access_denied",
                ))?;

        AuthRepository::load_actor_context(
            self,
            actor_user_id,
            workspace.tenant_id,
            workspace.id,
            None,
        )
        .await
    }

    async fn list_frontstage_pages(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontstagePageRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                parent_id,
                kind,
                title,
                icon,
                tooltip,
                is_hidden,
                slug,
                schema_root_uid,
                rank,
                created_at,
                updated_at
            from frontstage_pages
            where workspace_id = $1
            order by parent_id nulls first, rank asc, id asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| map_frontstage_page_row(&row))
            .collect()
    }

    async fn list_frontstage_page_visibility_rules_for_actor_roles(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>> {
        let rows = sqlx::query(
            r#"
            select
                rules.id,
                rules.workspace_id,
                rules.page_id,
                rules.role_id,
                rules.visibility,
                rules.created_at,
                rules.updated_at
            from frontstage_page_visibility_rules rules
            where rules.workspace_id = $2
              and rules.role_id in (
                  select roles.id
                  from user_role_bindings bindings
                  join roles on roles.id = bindings.role_id
                  where bindings.user_id = $1
                    and roles.scope_kind = 'workspace'
                    and roles.workspace_id = $2
              )
            order by rules.page_id nulls first, rules.role_id asc, rules.id asc
            "#,
        )
        .bind(actor_user_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| map_frontstage_visibility_rule_row(&row))
            .collect()
    }

    async fn get_frontstage_page(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> Result<Option<domain::FrontstagePageRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                parent_id,
                kind,
                title,
                icon,
                tooltip,
                is_hidden,
                slug,
                schema_root_uid,
                rank,
                created_at,
                updated_at
            from frontstage_pages
            where workspace_id = $1 and id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(page_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(|row| map_frontstage_page_row(&row)).transpose()
    }

    async fn get_frontstage_page_detail(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> Result<Option<domain::frontstage::FrontstagePageDetail>> {
        let row = sqlx::query(
            r#"
            select
                p.id,
                p.workspace_id,
                p.parent_id,
                p.kind,
                p.title,
                p.icon,
                p.tooltip,
                p.is_hidden,
                p.slug,
                p.schema_root_uid,
                p.rank,
                p.created_at,
                p.updated_at,
                s.workspace_id as schema_workspace_id,
                s.page_id as schema_page_id,
                s.root_uid,
                s.schema_payload,
                s.root_payload,
                s.created_at as schema_created_at,
                s.updated_at as schema_updated_at
            from frontstage_pages p
            join frontstage_page_schemas s
              on s.workspace_id = p.workspace_id
             and s.page_id = p.id
            where p.workspace_id = $1 and p.id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(page_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(|row| {
            let page = map_frontstage_page_row(&row)?;
            Ok(domain::frontstage::FrontstagePageDetail {
                page,
                schema: map_frontstage_schema_row(&row),
            })
        })
        .transpose()
    }

    async fn create_frontstage_page(
        &self,
        input: &CreateFrontstagePageInput,
    ) -> Result<domain::FrontstagePageRecord> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            insert into frontstage_pages (
                id,
                workspace_id,
                parent_id,
                kind,
                title,
                icon,
                tooltip,
                schema_root_uid,
                rank,
                created_by,
                updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning
                id,
                workspace_id,
                parent_id,
                kind,
                title,
                icon,
                tooltip,
                is_hidden,
                slug,
                schema_root_uid,
                rank,
                created_at,
                updated_at
            "#,
        )
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(input.parent_id)
        .bind(input.kind.as_str())
        .bind(&input.title)
        .bind(&input.icon)
        .bind(&input.tooltip)
        .bind(&input.schema_root_uid)
        .bind(&input.rank)
        .bind(input.actor_user_id)
        .fetch_one(&mut *tx)
        .await?;
        let page = map_frontstage_page_row(&row)?;
        if let Some(root_uid) = &input.schema_root_uid {
            sqlx::query(
                r#"
                insert into frontstage_page_schemas (
                    id,
                    scope_id,
                    page_id,
                    workspace_id,
                    root_uid,
                    schema_payload,
                    root_payload,
                    created_by,
                    updated_by
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(input.workspace_id)
            .bind(input.id)
            .bind(input.workspace_id)
            .bind(root_uid)
            .bind(json!({
                "version": 1,
                "root_uid": root_uid,
                "nodes": []
            }))
            .bind(json!({
                "uid": root_uid,
                "kind": "frontstage.page.root",
                "children": []
            }))
            .bind(input.actor_user_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Ok(page)
    }

    async fn update_frontstage_page_metadata(
        &self,
        input: &UpdateFrontstagePageMetadataInput,
    ) -> Result<domain::FrontstagePageRecord> {
        let mut tx = self.pool().begin().await?;
        let title_present = input.title.is_some();
        let title_value = input.title.clone().flatten();
        let icon_present = input.icon.is_some();
        let icon_value = input.icon.clone().flatten();
        let tooltip_present = input.tooltip.is_some();
        let tooltip_value = input.tooltip.clone().flatten();
        let hidden_present = input.is_hidden.is_some();
        let row = sqlx::query(
            r#"
            update frontstage_pages
            set title = case when $3 then $4 else title end,
                icon = case when $5 then $6 else icon end,
                tooltip = case when $7 then $8 else tooltip end,
                is_hidden = case when $9 then $10 else is_hidden end,
                updated_at = now()
            where workspace_id = $1 and id = $2
            returning
                id,
                workspace_id,
                parent_id,
                kind,
                title,
                icon,
                tooltip,
                is_hidden,
                slug,
                schema_root_uid,
                rank,
                created_at,
                updated_at
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(title_present)
        .bind(&title_value)
        .bind(icon_present)
        .bind(&icon_value)
        .bind(tooltip_present)
        .bind(&tooltip_value)
        .bind(hidden_present)
        .bind(input.is_hidden)
        .fetch_optional(&mut *tx)
        .await?;
        let page = row
            .map(|row| map_frontstage_page_row(&row))
            .transpose()?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        tx.commit().await?;

        Ok(page)
    }

    async fn move_frontstage_page(
        &self,
        input: &MoveFrontstagePageInput,
    ) -> Result<domain::FrontstagePageRecord> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            update frontstage_pages
            set parent_id = $3,
                rank = $4,
                updated_at = now()
            where workspace_id = $1 and id = $2
            returning
                id,
                workspace_id,
                parent_id,
                kind,
                title,
                icon,
                tooltip,
                is_hidden,
                slug,
                schema_root_uid,
                rank,
                created_at,
                updated_at
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(input.parent_id)
        .bind(&input.rank)
        .fetch_optional(&mut *tx)
        .await?;
        let page = row
            .map(|row| map_frontstage_page_row(&row))
            .transpose()?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        tx.commit().await?;

        Ok(page)
    }

    async fn delete_frontstage_page(&self, workspace_id: Uuid, page_id: Uuid) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let deleted = sqlx::query(
            r#"
            delete from frontstage_pages
            where workspace_id = $1 and id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(page_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if deleted == 0 {
            return Err(ControlPlaneError::NotFound("frontstage_page").into());
        }

        tx.commit().await?;
        Ok(())
    }

    async fn save_frontstage_page_content(
        &self,
        input: &SaveFrontstagePageContentInput,
    ) -> Result<domain::frontstage::FrontstagePageDetail> {
        let row = sqlx::query(
            r#"
            with updated_schema as (
                update frontstage_page_schemas
                set schema_payload = $3,
                    root_payload = $4,
                    updated_by = $5,
                    updated_at = now()
                where workspace_id = $1 and page_id = $2
                returning
                    workspace_id,
                    page_id,
                    root_uid,
                    schema_payload,
                    root_payload,
                    created_at,
                    updated_at
            ),
            updated_page as (
                update frontstage_pages
                set updated_by = $5,
                    updated_at = now()
                where workspace_id = $1
                  and id = $2
                  and exists (select 1 from updated_schema)
                returning
                    id,
                    workspace_id,
                    parent_id,
                    kind,
                    title,
                    icon,
                    tooltip,
                    is_hidden,
                    slug,
                    schema_root_uid,
                    rank,
                    created_at,
                    updated_at
            )
            select
                p.id,
                p.workspace_id,
                p.parent_id,
                p.kind,
                p.title,
                p.icon,
                p.tooltip,
                p.is_hidden,
                p.slug,
                p.schema_root_uid,
                p.rank,
                p.created_at,
                p.updated_at,
                s.workspace_id as schema_workspace_id,
                s.page_id as schema_page_id,
                s.root_uid,
                s.schema_payload,
                s.root_payload,
                s.created_at as schema_created_at,
                s.updated_at as schema_updated_at
            from updated_page p
            join updated_schema s
              on s.workspace_id = p.workspace_id
             and s.page_id = p.id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.schema_payload)
        .bind(&input.root_payload)
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;

        let row = row.ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        let page = map_frontstage_page_row(&row)?;
        Ok(domain::frontstage::FrontstagePageDetail {
            page,
            schema: map_frontstage_schema_row(&row),
        })
    }

    async fn get_frontstage_block_code(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        code_ref: &str,
    ) -> Result<Option<domain::frontstage::FrontstageBlockCodeRecord>> {
        let row = sqlx::query(
            r#"
            select workspace_id, page_id, code_ref, code, created_at, updated_at
            from frontstage_block_codes
            where workspace_id = $1 and page_id = $2 and code_ref = $3
            "#,
        )
        .bind(workspace_id)
        .bind(page_id)
        .bind(code_ref)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_frontstage_block_code_row))
    }

    async fn save_frontstage_block_code(
        &self,
        input: &SaveFrontstageBlockCodeInput,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        let row = sqlx::query(
            r#"
            insert into frontstage_block_codes (
                id,
                workspace_id,
                page_id,
                code_ref,
                code
            ) values ($1, $2, $3, $4, $5)
            on conflict (workspace_id, page_id, code_ref)
            do update set
                code = excluded.code,
                updated_at = now()
            returning workspace_id, page_id, code_ref, code, created_at, updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(&input.code_ref)
        .bind(&input.code)
        .fetch_one(self.pool())
        .await?;

        Ok(map_frontstage_block_code_row(row))
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }
}
