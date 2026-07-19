use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateFrontstagePageInput, CreateFrontstagePageTabInput,
        FrontstagePageRepository, MoveFrontstagePageInput, SaveFrontstageBlockCodeInput,
        SaveFrontstageTabDocumentInput, UpdateFrontstagePageMetadataInput,
        UpdateFrontstagePageTabInput, WorkspaceRepository,
    },
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn map_frontstage_placement_error(error: sqlx::Error) -> anyhow::Error {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.constraint() == Some("frontstage_pages_workspace_slug_uidx") {
            return ControlPlaneError::Conflict("frontstage_page_slug_conflict").into();
        }
    }
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.constraint()
            == Some("frontstage_page_tabs_workspace_page_route_segment_uidx")
        {
            return ControlPlaneError::Conflict("frontstage_page_tab_route_segment_conflict").into();
        }
    }
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.constraint() == Some("frontstage_pages_parent_child_placement") {
            if database_error
                .message()
                .contains("frontstage_group_placement_requires_empty_group")
            {
                return ControlPlaneError::InvalidInput(
                    "frontstage_group_placement_requires_empty_group",
                )
                .into();
            }
            return ControlPlaneError::InvalidInput("frontstage_page_placement_mismatch").into();
        }
    }
    error.into()
}

async fn grant_new_frontstage_page_to_auto_grant_roles(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    page_id: Uuid,
    actor_user_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"insert into frontstage_page_visibility_rules
           (id, workspace_id, page_id, tab_id, role_id, visibility, created_by, updated_by)
           select $1, $2, $3, null, roles.id, 'visible', $4, $4
           from roles
           where roles.workspace_id = $1 and roles.auto_grant_new_permissions = true
           on conflict (workspace_id, page_id, role_id) where page_id is not null
           do update set visibility = 'visible', updated_by = excluded.updated_by, updated_at = now()"#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(page_id)
    .bind(actor_user_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn grant_new_frontstage_tab_to_auto_grant_roles(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    tab_id: Uuid,
    actor_user_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"insert into frontstage_page_visibility_rules
           (id, workspace_id, page_id, tab_id, role_id, visibility, created_by, updated_by)
           select $1, $2, null, $3, roles.id, 'visible', $4, $4
           from roles
           where roles.workspace_id = $1 and roles.auto_grant_new_permissions = true
           on conflict (workspace_id, tab_id, role_id) where tab_id is not null
           do update set visibility = 'visible', updated_by = excluded.updated_by, updated_at = now()"#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(tab_id)
    .bind(actor_user_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_frontstage_page_tab(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &CreateFrontstagePageTabInput,
) -> Result<domain::frontstage::FrontstagePageTabRecord> {
    let row = sqlx::query(
        r#"
        insert into frontstage_page_tabs (
            id, workspace_id, page_id, title, rank, is_default, document_root_uid,
            route_segment, created_by, updated_by
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
        returning id as tab_id, workspace_id as tab_workspace_id, page_id as tab_page_id,
                  title as tab_title, rank as tab_rank, is_default as tab_is_default,
                  route_segment as tab_route_segment, document_root_uid,
                  created_at as tab_created_at, updated_at as tab_updated_at
        "#,
    )
    .bind(input.id)
    .bind(input.workspace_id)
    .bind(input.page_id)
    .bind(&input.title)
    .bind(&input.rank)
    .bind(input.is_default)
    .bind(&input.document_root_uid)
    .bind(&input.route_segment)
    .bind(input.actor_user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_frontstage_placement_error)?;
    Ok(map_frontstage_tab_row(&row))
}

fn map_frontstage_page_row(row: &sqlx::postgres::PgRow) -> Result<domain::FrontstagePageRecord> {
    let raw_kind: String = row.get("kind");
    let kind = domain::FrontstagePageKind::from_db(&raw_kind)
        .ok_or(ControlPlaneError::InvalidInput("frontstage_page_kind"))?;
    let raw_placement: String = row.get("placement");
    let placement = domain::frontstage::FrontstageNavigationPlacement::from_db(&raw_placement)
        .ok_or(ControlPlaneError::InvalidInput(
            "frontstage_navigation_placement",
        ))?;
    let raw_content_presentation: String = row.get("content_presentation");
    let content_presentation =
        domain::frontstage::FrontstagePageContentPresentation::from_db(&raw_content_presentation)
            .ok_or(ControlPlaneError::InvalidInput(
                "frontstage_page_content_presentation",
            ))?;

    Ok(domain::FrontstagePageRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        parent_id: row.get("parent_id"),
        kind,
        title: row.get("title"),
        icon: row.get("icon"),
        tooltip: row.get("tooltip"),
        is_hidden: row.get("is_hidden"),
        placement,
        content_presentation,
        slug: row.get("slug"),
        rank: row.get("rank"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_frontstage_document_row(
    row: &sqlx::postgres::PgRow,
) -> domain::frontstage::FrontstageTabDocumentRecord {
    domain::frontstage::FrontstageTabDocumentRecord {
        workspace_id: row.get("schema_workspace_id"),
        tab_id: row.get("schema_tab_id"),
        root_uid: row.get("root_uid"),
        payload: row.get("document_payload"),
        created_at: row.get("schema_created_at"),
        updated_at: row.get("schema_updated_at"),
    }
}

fn map_frontstage_tab_row(
    row: &sqlx::postgres::PgRow,
) -> domain::frontstage::FrontstagePageTabRecord {
    domain::frontstage::FrontstagePageTabRecord {
        id: row.get("tab_id"),
        workspace_id: row.get("tab_workspace_id"),
        page_id: row.get("tab_page_id"),
        title: row.get("tab_title"),
        rank: row.get("tab_rank"),
        is_default: row.get("tab_is_default"),
        route_segment: row.get("tab_route_segment"),
        document_root_uid: row.get("document_root_uid"),
        created_at: row.get("tab_created_at"),
        updated_at: row.get("tab_updated_at"),
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
        tab_id: row.get("tab_id"),
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
                placement,
                content_presentation,
                slug,
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
                rules.tab_id,
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

    async fn list_frontstage_page_visibility_rules_for_role(
        &self,
        workspace_id: Uuid,
        role_code: &str,
    ) -> Result<Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>> {
        let rows = sqlx::query(
            r#"
            select rules.id, rules.workspace_id, rules.page_id, rules.tab_id,
                   rules.role_id, rules.visibility, rules.created_at, rules.updated_at
            from frontstage_page_visibility_rules rules
            join roles on roles.id = rules.role_id and roles.workspace_id = rules.workspace_id
            where rules.workspace_id = $1 and roles.code = $2
              and (rules.page_id is not null or rules.tab_id is not null)
            order by rules.page_id nulls last, rules.tab_id nulls last
            "#,
        )
        .bind(workspace_id)
        .bind(role_code)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|row| map_frontstage_visibility_rule_row(&row))
            .collect()
    }

    async fn replace_frontstage_page_visibility_rules_for_role(
        &self,
        workspace_id: Uuid,
        role_code: &str,
        page_ids: &[Uuid],
        tab_ids: &[Uuid],
        actor_user_id: Uuid,
    ) -> Result<()> {
        let mut transaction = self.pool().begin().await?;
        let role_id: Uuid =
            sqlx::query_scalar("select id from roles where workspace_id = $1 and code = $2")
                .bind(workspace_id)
                .bind(role_code)
                .fetch_optional(&mut *transaction)
                .await?
                .ok_or(ControlPlaneError::NotFound("role"))?;

        sqlx::query(
            "delete from frontstage_page_visibility_rules where workspace_id = $1 and role_id = $2 and (page_id is not null or tab_id is not null)",
        )
        .bind(workspace_id)
        .bind(role_id)
        .execute(&mut *transaction)
        .await?;

        for page_id in page_ids {
            sqlx::query(
                r#"insert into frontstage_page_visibility_rules
                   (id, workspace_id, page_id, tab_id, role_id, visibility, created_by, updated_by)
                   select $1, $2, pages.id, null, $3, 'visible', $4, $4
                   from frontstage_pages pages where pages.workspace_id = $2 and pages.id = $5"#,
            )
            .bind(Uuid::now_v7())
            .bind(workspace_id)
            .bind(role_id)
            .bind(actor_user_id)
            .bind(page_id)
            .execute(&mut *transaction)
            .await?;
        }
        for tab_id in tab_ids {
            sqlx::query(
                r#"insert into frontstage_page_visibility_rules
                   (id, workspace_id, page_id, tab_id, role_id, visibility, created_by, updated_by)
                   select $1, $2, null, tabs.id, $3, 'visible', $4, $4
                   from frontstage_page_tabs tabs where tabs.workspace_id = $2 and tabs.id = $5"#,
            )
            .bind(Uuid::now_v7())
            .bind(workspace_id)
            .bind(role_id)
            .bind(actor_user_id)
            .bind(tab_id)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
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
                placement,
                content_presentation,
                slug,
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

    async fn list_frontstage_page_tabs(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageTabRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id as tab_id, workspace_id as tab_workspace_id, page_id as tab_page_id,
                title as tab_title, rank as tab_rank, is_default as tab_is_default,
                route_segment as tab_route_segment, document_root_uid,
                created_at as tab_created_at, updated_at as tab_updated_at
            from frontstage_page_tabs
            where workspace_id = $1 and page_id = $2
            order by rank, id
            "#,
        )
        .bind(workspace_id)
        .bind(page_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.iter().map(map_frontstage_tab_row).collect())
    }

    async fn get_frontstage_page_tab_detail(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_reference: &str,
    ) -> Result<Option<domain::frontstage::FrontstagePageDetail>> {
        let row = sqlx::query(
            r#"
            select p.id, p.workspace_id, p.parent_id, p.kind, p.title, p.icon, p.tooltip,
                   p.is_hidden, p.placement, p.content_presentation, p.slug, p.rank,
                   p.created_at, p.updated_at,
                   t.id as tab_id, t.workspace_id as tab_workspace_id, t.page_id as tab_page_id,
                   t.title as tab_title, t.rank as tab_rank, t.is_default as tab_is_default,
                   t.route_segment as tab_route_segment, t.document_root_uid,
                   t.created_at as tab_created_at, t.updated_at as tab_updated_at,
                   s.workspace_id as schema_workspace_id, s.tab_id as schema_tab_id,
                   s.root_uid, s.document_payload,
                   s.created_at as schema_created_at, s.updated_at as schema_updated_at
            from frontstage_pages p
            join frontstage_page_tabs t on t.workspace_id = p.workspace_id and t.page_id = p.id
            join frontstage_page_schemas s on s.workspace_id = t.workspace_id and s.tab_id = t.id
            where p.workspace_id = $1
              and p.id = $2
              and (t.route_segment = $3 or t.id::text = $3)
            "#,
        )
        .bind(workspace_id)
        .bind(page_id)
        .bind(tab_reference)
        .fetch_optional(self.pool()).await?;
        row.map(|row| {
            Ok(domain::frontstage::FrontstagePageDetail {
                page: map_frontstage_page_row(&row)?,
                tab: map_frontstage_tab_row(&row),
                document: map_frontstage_document_row(&row),
            })
        })
        .transpose()
    }

    async fn create_frontstage_page(
        &self,
        input: &CreateFrontstagePageInput,
    ) -> Result<domain::frontstage::FrontstagePageCreation> {
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
                placement,
                content_presentation,
                slug,
                rank,
                created_by,
                updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
            returning
                id,
                workspace_id,
                parent_id,
                kind,
                title,
                icon,
                tooltip,
                is_hidden,
                placement,
                content_presentation,
                slug,
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
        .bind(input.placement.as_str())
        .bind(input.content_presentation.as_str())
        .bind(&input.slug)
        .bind(&input.rank)
        .bind(input.actor_user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_frontstage_placement_error)?;
        let page = map_frontstage_page_row(&row)?;
        grant_new_frontstage_page_to_auto_grant_roles(
            &mut tx,
            input.workspace_id,
            page.id,
            input.actor_user_id,
        )
        .await?;
        let default_tab = if let Some(tab) = &input.default_tab {
            let tab = insert_frontstage_page_tab(&mut tx, tab).await?;
            grant_new_frontstage_tab_to_auto_grant_roles(
                &mut tx,
                input.workspace_id,
                tab.id,
                input.actor_user_id,
            )
            .await?;
            sqlx::query(
                r#"
                insert into frontstage_page_schemas (
                    id,
                    scope_id,
                    tab_id,
                    workspace_id,
                    root_uid,
                    schema_payload,
                    root_payload,
                    document_payload,
                    created_by,
                    updated_by
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(input.workspace_id)
            .bind(tab.id)
            .bind(input.workspace_id)
            .bind(&tab.document_root_uid)
            .bind(json!({
                "version": 1,
                "root_uid": tab.document_root_uid,
                "nodes": []
            }))
            .bind(json!({
                "uid": tab.document_root_uid,
                "kind": "frontstage.tab.root",
                "children": []
            }))
            .bind(json!({
                "version": 1,
                "root_uid": tab.document_root_uid,
                "blocks": []
            }))
            .bind(input.actor_user_id)
            .execute(&mut *tx)
            .await?;
            Some(tab)
        } else {
            None
        };
        tx.commit().await?;
        Ok(domain::frontstage::FrontstagePageCreation { page, default_tab })
    }

    async fn create_frontstage_page_tab(
        &self,
        input: &CreateFrontstagePageTabInput,
    ) -> Result<domain::frontstage::FrontstagePageTabRecord> {
        let mut tx = self.pool().begin().await?;
        let tab = insert_frontstage_page_tab(&mut tx, input).await?;
        grant_new_frontstage_tab_to_auto_grant_roles(
            &mut tx,
            input.workspace_id,
            tab.id,
            input.actor_user_id,
        )
        .await?;
        sqlx::query(
            r#"
            insert into frontstage_page_schemas (
                id, scope_id, tab_id, workspace_id, root_uid, schema_payload, root_payload,
                document_payload, created_by, updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(&input.document_root_uid)
        .bind(json!({"version": 1, "root_uid": input.document_root_uid, "nodes": []}))
        .bind(
            json!({"uid": input.document_root_uid, "kind": "frontstage.tab.root", "children": []}),
        )
        .bind(json!({"version": 1, "root_uid": input.document_root_uid, "blocks": []}))
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(tab)
    }

    async fn update_frontstage_page_tab(
        &self,
        input: &UpdateFrontstagePageTabInput,
    ) -> Result<domain::frontstage::FrontstagePageTabRecord> {
        let title_present = input.title.is_some();
        let title_value = input.title.clone().flatten();
        let rank_present = input.rank.is_some();
        let row = sqlx::query(
            r#"
            update frontstage_page_tabs
            set title = case when $4 then $5 else title end,
                rank = case when $6 then $7 else rank end,
                updated_by = $8,
                updated_at = now()
            where workspace_id = $1 and page_id = $2 and id = $3
            returning id as tab_id, workspace_id as tab_workspace_id, page_id as tab_page_id,
                      title as tab_title, rank as tab_rank, is_default as tab_is_default,
                      route_segment as tab_route_segment, document_root_uid,
                      created_at as tab_created_at, updated_at as tab_updated_at
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(input.tab_id)
        .bind(title_present)
        .bind(title_value)
        .bind(rank_present)
        .bind(&input.rank)
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;
        row.as_ref()
            .map(map_frontstage_tab_row)
            .ok_or_else(|| ControlPlaneError::NotFound("frontstage_page_tab").into())
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
        let placement_present = input.placement.is_some();
        let placement_value = input.placement.map(|placement| placement.as_str());
        let content_presentation_present = input.content_presentation.is_some();
        let content_presentation_value = input
            .content_presentation
            .map(|content_presentation| content_presentation.as_str());
        let slug_present = input.slug.is_some();
        let slug_value = input.slug.clone().flatten();
        let row = sqlx::query(
            r#"
            update frontstage_pages
            set title = case when $3 then $4 else title end,
                icon = case when $5 then $6 else icon end,
                tooltip = case when $7 then $8 else tooltip end,
                is_hidden = case when $9 then $10 else is_hidden end,
                placement = case when $11 then $12 else placement end,
                content_presentation = case when $13 then $14 else content_presentation end,
                slug = case when $15 then $16 else slug end,
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
                placement,
                content_presentation,
                slug,
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
        .bind(placement_present)
        .bind(placement_value)
        .bind(content_presentation_present)
        .bind(content_presentation_value)
        .bind(slug_present)
        .bind(&slug_value)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_frontstage_placement_error)?;
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
                placement,
                content_presentation,
                slug,
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
        .await
        .map_err(map_frontstage_placement_error)?;
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

    async fn delete_frontstage_page_tab(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let tab_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            select id from frontstage_page_tabs
            where workspace_id = $1 and page_id = $2
            order by id
            for update
            "#,
        )
        .bind(workspace_id)
        .bind(page_id)
        .fetch_all(&mut *tx)
        .await?;
        if !tab_ids.contains(&tab_id) {
            return Err(ControlPlaneError::NotFound("frontstage_page_tab").into());
        }
        if tab_ids.len() == 1 {
            return Err(ControlPlaneError::Conflict("frontstage_page_requires_tab").into());
        }

        let deleting_default: bool = sqlx::query_scalar(
            "select is_default from frontstage_page_tabs where workspace_id = $1 and page_id = $2 and id = $3",
        ).bind(workspace_id).bind(page_id).bind(tab_id).fetch_one(&mut *tx).await?;
        sqlx::query(
            "delete from frontstage_page_tabs where workspace_id = $1 and page_id = $2 and id = $3",
        )
        .bind(workspace_id)
        .bind(page_id)
        .bind(tab_id)
        .execute(&mut *tx)
        .await?;
        if deleting_default {
            sqlx::query(
                r#"
                update frontstage_page_tabs
                set is_default = true,
                    route_segment = null,
                    updated_by = $3,
                    updated_at = now()
                where id = (
                    select id from frontstage_page_tabs
                    where workspace_id = $1 and page_id = $2
                    order by rank, id limit 1
                )
                "#,
            ).bind(workspace_id).bind(page_id).bind(actor_user_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn save_frontstage_tab_document(
        &self,
        input: &SaveFrontstageTabDocumentInput,
    ) -> Result<domain::frontstage::FrontstagePageDetail> {
        let row = sqlx::query(
            r#"
            with updated_schema as (
                update frontstage_page_schemas
                set schema_payload = $4,
                    root_payload = case
                        when jsonb_typeof($4->'blocks') = 'array'
                            then jsonb_set(root_payload, '{blocks}', $4->'blocks', true)
                        else root_payload
                    end,
                    document_payload = $4,
                    updated_by = $5,
                    updated_at = now()
                where workspace_id = $1 and tab_id = $3
                returning
                    workspace_id,
                    tab_id,
                    root_uid,
                    document_payload,
                    created_at,
                    updated_at
            ),
            updated_page as (
                update frontstage_pages
                set updated_by = $5,
                    updated_at = now()
                where workspace_id = $1
                  and id = $2
                  and exists (
                      select 1 from frontstage_page_tabs t
                      join updated_schema s on s.tab_id = t.id
                      where t.workspace_id = $1 and t.page_id = $2 and t.id = $3
                  )
                returning
                    id,
                    workspace_id,
                    parent_id,
                    kind,
                    title,
                    icon,
                    tooltip,
                    is_hidden,
                    placement,
                    content_presentation,
                    slug,
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
                p.placement,
                p.content_presentation,
                p.slug,
                p.rank,
                p.created_at,
                p.updated_at,
                s.workspace_id as schema_workspace_id,
                t.id as tab_id, t.workspace_id as tab_workspace_id, t.page_id as tab_page_id,
                t.title as tab_title, t.rank as tab_rank, t.is_default as tab_is_default,
                t.route_segment as tab_route_segment, t.document_root_uid,
                t.created_at as tab_created_at, t.updated_at as tab_updated_at,
                s.tab_id as schema_tab_id,
                s.root_uid,
                s.document_payload,
                s.created_at as schema_created_at,
                s.updated_at as schema_updated_at
            from updated_page p
            join frontstage_page_tabs t
              on t.workspace_id = p.workspace_id and t.page_id = p.id and t.id = $3
            join updated_schema s
              on s.workspace_id = t.workspace_id and s.tab_id = t.id
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.page_id)
        .bind(input.tab_id)
        .bind(&input.document_payload)
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;

        let row = row.ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        let page = map_frontstage_page_row(&row)?;
        Ok(domain::frontstage::FrontstagePageDetail {
            page,
            tab: map_frontstage_tab_row(&row),
            document: map_frontstage_document_row(&row),
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
