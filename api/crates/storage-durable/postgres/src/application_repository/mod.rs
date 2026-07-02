use anyhow::Result;
use async_trait::async_trait;
use control_plane::errors::ControlPlaneError;
use control_plane::ports::{
    ApplicationRepository, ApplicationVisibility, AuthRepository, CreateApplicationInput,
    CreateApplicationTagInput, DeleteApplicationInput, ReplaceApplicationEnvironmentVariablesInput,
    UpdateApplicationInput,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::application_mapper::{PgApplicationMapper, StoredApplicationRow},
    repositories::{tenant_id_for_workspace, workspace_id_for_user, PgControlPlaneStore},
};

fn map_application_record(row: sqlx::postgres::PgRow) -> Result<domain::ApplicationRecord> {
    PgApplicationMapper::to_application_record(StoredApplicationRow {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        application_type: row.get("application_type"),
        workflow_trigger_type: row.get("workflow_trigger_type"),
        name: row.get("name"),
        description: row.get("description"),
        icon: row.get("icon"),
        icon_type: row.get("icon_type"),
        icon_background: row.get("icon_background"),
        created_by: row.get("created_by"),
        updated_at: row.get("updated_at"),
        current_flow_id: row.get("current_flow_id"),
        current_draft_id: row.get("current_draft_id"),
        api_enabled: row.get("api_enabled"),
        has_application_api_keys: row.get("has_application_api_keys"),
        has_application_api_mapping: row.get("has_application_api_mapping"),
        active_publication_id: row.get("active_publication_id"),
        tags: row.get("tags"),
    })
}

#[async_trait]
impl ApplicationRepository for PgControlPlaneStore {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let workspace_id = workspace_id_for_user(self.pool(), actor_user_id).await?;
        let tenant_id = tenant_id_for_workspace(self.pool(), workspace_id).await?;

        AuthRepository::load_actor_context(self, actor_user_id, tenant_id, workspace_id, None).await
    }

    async fn list_applications(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationRecord>> {
        let visibility_value = match visibility {
            ApplicationVisibility::Own => "own",
            ApplicationVisibility::All => "all",
        };
        let rows = sqlx::query(
            r#"
            select
                a.id,
                a.workspace_id,
                a.application_type,
                a.workflow_trigger_type,
                a.name,
                a.description,
                a.icon_type,
                a.icon,
                a.icon_background,
                a.created_by,
                a.updated_at,
                null::uuid as current_flow_id,
                null::uuid as current_draft_id,
                a.api_enabled,
                exists(
                    select 1
                    from api_keys key
                    where key.application_id = a.id
                      and key.key_kind = 'application_api_key'
                      and key.enabled = true
                ) as has_application_api_keys,
                exists(
                    select 1
                    from application_api_mappings mapping
                    where mapping.application_id = a.id
                ) as has_application_api_mapping,
                active_publication.id as active_publication_id,
                coalesce(tags.tags, '[]'::jsonb) as tags
            from applications a
            left join lateral (
                select publication.id
                from application_publication_versions publication
                where publication.application_id = a.id
                  and publication.active = true
                limit 1
            ) active_publication on true
            left join lateral (
                select jsonb_agg(
                    jsonb_build_object('id', tag.id, 'name', tag.name)
                    order by tag.name asc, tag.id asc
                ) as tags
                from application_tag_bindings binding
                join application_tags tag on tag.id = binding.tag_id
                where binding.application_id = a.id
            ) tags on true
            where a.workspace_id = $1
              and ($3 = 'all' or a.created_by = $2)
            order by a.updated_at desc, a.id desc
            "#,
        )
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(visibility_value)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_application_record).collect()
    }

    async fn create_application(
        &self,
        input: &CreateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        let row = sqlx::query(
            r#"
            insert into applications (
                id,
                workspace_id,
                application_type,
                workflow_trigger_type,
                name,
                description,
                icon_type,
                icon,
                icon_background,
                created_by,
                updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning
                id,
                workspace_id,
                application_type,
                workflow_trigger_type,
                name,
                description,
                icon_type,
                icon,
                icon_background,
                created_by,
                updated_at,
                null::uuid as current_flow_id,
                null::uuid as current_draft_id,
                api_enabled,
                false as has_application_api_keys,
                false as has_application_api_mapping,
                null::uuid as active_publication_id,
                '[]'::jsonb as tags
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.application_type.as_str())
        .bind(input.workflow_trigger_type.map(|value| value.as_str()))
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.icon_type.as_deref())
        .bind(input.icon.as_deref())
        .bind(input.icon_background.as_deref())
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_application_record(row)
    }

    async fn update_application(
        &self,
        input: &UpdateApplicationInput,
    ) -> Result<domain::ApplicationRecord> {
        let mut tx = self.pool().begin().await?;
        let tag_count = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)::bigint
            from application_tags
            where workspace_id = $1
              and id = any($2)
            "#,
        )
        .bind(input.workspace_id)
        .bind(&input.tag_ids)
        .fetch_one(&mut *tx)
        .await?;

        if tag_count != input.tag_ids.len() as i64 {
            anyhow::bail!(ControlPlaneError::InvalidInput("tag_ids"));
        }

        let updated_rows = sqlx::query(
            r#"
            update applications
            set
                name = $3,
                description = $4,
                updated_by = $5,
                updated_at = now()
            where workspace_id = $1
              and id = $2
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if updated_rows == 0 {
            anyhow::bail!(ControlPlaneError::NotFound("application"));
        }

        sqlx::query("delete from application_tag_bindings where application_id = $1")
            .bind(input.application_id)
            .execute(&mut *tx)
            .await?;

        for tag_id in &input.tag_ids {
            sqlx::query(
                r#"
                insert into application_tag_bindings (
                    id,
                    scope_id,
                    application_id,
                    tag_id,
                    created_by,
                    updated_by
                ) values ($1, (select scope_id from applications where id = $2), $2, $3, $4, $4)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(input.application_id)
            .bind(tag_id)
            .bind(input.actor_user_id)
            .execute(&mut *tx)
            .await?;
        }

        let row = sqlx::query(
            r#"
            select
                a.id,
                a.workspace_id,
                a.application_type,
                a.workflow_trigger_type,
                a.name,
                a.description,
                a.icon_type,
                a.icon,
                a.icon_background,
                a.created_by,
                a.updated_at,
                null::uuid as current_flow_id,
                null::uuid as current_draft_id,
                a.api_enabled,
                exists(
                    select 1
                    from api_keys key
                    where key.application_id = a.id
                      and key.key_kind = 'application_api_key'
                      and key.enabled = true
                ) as has_application_api_keys,
                exists(
                    select 1
                    from application_api_mappings mapping
                    where mapping.application_id = a.id
                ) as has_application_api_mapping,
                active_publication.id as active_publication_id,
                coalesce(tags.tags, '[]'::jsonb) as tags
            from applications a
            left join lateral (
                select publication.id
                from application_publication_versions publication
                where publication.application_id = a.id
                  and publication.active = true
                limit 1
            ) active_publication on true
            left join lateral (
                select jsonb_agg(
                    jsonb_build_object('id', tag.id, 'name', tag.name)
                    order by tag.name asc, tag.id asc
                ) as tags
                from application_tag_bindings binding
                join application_tags tag on tag.id = binding.tag_id
                where binding.application_id = a.id
            ) tags on true
            where a.workspace_id = $1
              and a.id = $2
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.application_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        map_application_record(row)
    }

    async fn delete_application(&self, input: &DeleteApplicationInput) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        sqlx::query(
            r#"
            delete from flow_runs
            where application_id = $1
            "#,
        )
        .bind(input.application_id)
        .execute(&mut *tx)
        .await?;

        let deleted_rows = sqlx::query(
            r#"
            delete from applications
            where workspace_id = $1
              and id = $2
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.application_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if deleted_rows == 0 {
            anyhow::bail!(ControlPlaneError::NotFound("application"));
        }

        tx.commit().await?;

        Ok(())
    }

    async fn get_application(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<domain::ApplicationRecord>> {
        let row = sqlx::query(
            r#"
            select
                a.id,
                a.workspace_id,
                a.application_type,
                a.workflow_trigger_type,
                a.name,
                a.description,
                a.icon_type,
                a.icon,
                a.icon_background,
                a.created_by,
                a.updated_at,
                f.id as current_flow_id,
                fd.id as current_draft_id,
                a.api_enabled,
                exists(
                    select 1
                    from api_keys key
                    where key.application_id = a.id
                      and key.key_kind = 'application_api_key'
                      and key.enabled = true
                ) as has_application_api_keys,
                exists(
                    select 1
                    from application_api_mappings mapping
                    where mapping.application_id = a.id
                ) as has_application_api_mapping,
                active_publication.id as active_publication_id,
                coalesce(tags.tags, '[]'::jsonb) as tags
            from applications a
            left join flows f on f.application_id = a.id
            left join flow_drafts fd on fd.flow_id = f.id
            left join lateral (
                select publication.id
                from application_publication_versions publication
                where publication.application_id = a.id
                  and publication.active = true
                limit 1
            ) active_publication on true
            left join lateral (
                select jsonb_agg(
                    jsonb_build_object('id', tag.id, 'name', tag.name)
                    order by tag.name asc, tag.id asc
                ) as tags
                from application_tag_bindings binding
                join application_tags tag on tag.id = binding.tag_id
                where binding.application_id = a.id
            ) tags on true
            where a.workspace_id = $1
              and a.id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_application_record).transpose()
    }

    async fn list_application_tags(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Vec<domain::ApplicationTagCatalogEntry>> {
        let visibility_value = match visibility {
            ApplicationVisibility::Own => "own",
            ApplicationVisibility::All => "all",
        };

        let rows = sqlx::query(
            r#"
            select
                tag.id,
                tag.name,
                count(app.id)::bigint as application_count
            from application_tags tag
            left join application_tag_bindings binding on binding.tag_id = tag.id
            left join applications app
                on app.id = binding.application_id
               and app.workspace_id = $1
               and ($3 = 'all' or app.created_by = $2)
            where tag.workspace_id = $1
            group by tag.id, tag.name
            order by tag.name asc, tag.id asc
            "#,
        )
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(visibility_value)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| domain::ApplicationTagCatalogEntry {
                id: row.get("id"),
                name: row.get("name"),
                application_count: row.get("application_count"),
            })
            .collect())
    }

    async fn create_application_tag(
        &self,
        input: &CreateApplicationTagInput,
    ) -> Result<domain::ApplicationTagCatalogEntry> {
        let normalized_name = input.name.to_lowercase();
        let row = sqlx::query(
            r#"
            insert into application_tags (
                id,
                workspace_id,
                name,
                normalized_name,
                created_by,
                updated_by
            ) values ($1, $2, $3, $4, $5, $5)
            on conflict (workspace_id, normalized_name) do update
                set name = excluded.name,
                    updated_by = excluded.updated_by,
                    updated_at = now()
            returning
                id,
                name,
                0::bigint as application_count
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(&input.name)
        .bind(&normalized_name)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        Ok(domain::ApplicationTagCatalogEntry {
            id: row.get("id"),
            name: row.get("name"),
            application_count: row.get("application_count"),
        })
    }

    async fn list_application_environment_variables(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let rows = sqlx::query(
            r#"
            select
                env.application_id,
                env.name,
                env.value_type,
                env.value_json,
                env.description,
                env.updated_at
            from application_environment_variables env
            join applications app on app.id = env.application_id
            where app.workspace_id = $1
              and env.application_id = $2
            order by env.name asc
            "#,
        )
        .bind(workspace_id)
        .bind(application_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| domain::ApplicationEnvironmentVariable {
                application_id: row.get("application_id"),
                name: row.get("name"),
                value_type: row.get("value_type"),
                value: row.get("value_json"),
                description: row.get("description"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    async fn replace_application_environment_variables(
        &self,
        input: &ReplaceApplicationEnvironmentVariablesInput,
    ) -> Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let mut tx = self.pool().begin().await?;
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
                select 1
                from applications
                where workspace_id = $1
                  and id = $2
            )
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.application_id)
        .fetch_one(&mut *tx)
        .await?;

        if !exists {
            anyhow::bail!(ControlPlaneError::NotFound("application"));
        }

        sqlx::query("delete from application_environment_variables where application_id = $1")
            .bind(input.application_id)
            .execute(&mut *tx)
            .await?;

        for variable in &input.variables {
            sqlx::query(
                r#"
                insert into application_environment_variables (
                    id,
                    scope_id,
                    application_id,
                    name,
                    value_type,
                    value_json,
                    description,
                    created_by,
                    updated_by
                ) values ($1, (select scope_id from applications where id = $2), $2, $3, $4, $5, $6, $7, $7)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(input.application_id)
            .bind(&variable.name)
            .bind(&variable.value_type)
            .bind(&variable.value)
            .bind(&variable.description)
            .bind(input.actor_user_id)
            .execute(&mut *tx)
            .await?;
        }

        let rows = sqlx::query(
            r#"
            select
                application_id,
                name,
                value_type,
                value_json,
                description,
                updated_at
            from application_environment_variables
            where application_id = $1
            order by name asc
            "#,
        )
        .bind(input.application_id)
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(rows
            .into_iter()
            .map(|row| domain::ApplicationEnvironmentVariable {
                application_id: row.get("application_id"),
                name: row.get("name"),
                value_type: row.get("value_type"),
                value: row.get("value_json"),
                description: row.get("description"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }
}
