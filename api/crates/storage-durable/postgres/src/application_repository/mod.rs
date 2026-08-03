use anyhow::Result;
use async_trait::async_trait;
use control_plane::errors::ControlPlaneError;
use control_plane::ports::{
    ApplicationArchiveRelease, ApplicationArchiveReleaseDigest, ApplicationManagementPage,
    ApplicationManagementQuery, ApplicationManagementRecord, ApplicationManagementRepository,
    ApplicationManagementSortDirection, ApplicationManagementSortField, ApplicationRepository,
    ApplicationVisibility, AuthRepository, CreateApplicationInput, CreateApplicationTagInput,
    CreateWorkflowTriggerConfig, DeleteApplicationInput,
    ReplaceApplicationEnvironmentVariablesInput, UpdateApplicationInput,
};
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::{
    mappers::application_mapper::{
        parse_application_type, parse_workflow_trigger_type, PgApplicationMapper,
        StoredApplicationRow,
    },
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
        release_version: row.get("release_version"),
        release_digest: row.get("release_digest"),
        current_flow_id: row.get("current_flow_id"),
        current_draft_id: row.get("current_draft_id"),
        api_enabled: row.get("api_enabled"),
        has_application_api_keys: row.get("has_application_api_keys"),
        has_application_api_mapping: row.get("has_application_api_mapping"),
        active_publication_id: row.get("active_publication_id"),
        tags: row.get("tags"),
    })
}

async fn find_application(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    application_id: Uuid,
    actor_user_id: Option<Uuid>,
    visibility: ApplicationVisibility,
) -> Result<Option<domain::ApplicationRecord>> {
    let visibility_value = match visibility {
        ApplicationVisibility::Own => "own",
        ApplicationVisibility::All => "all",
    };
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
            a.release_version,
            a.release_digest,
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
          and ($3::uuid is null or $4 = 'all' or a.created_by = $3)
        "#,
    )
    .bind(workspace_id)
    .bind(application_id)
    .bind(actor_user_id)
    .bind(visibility_value)
    .fetch_optional(pool)
    .await?;

    row.map(map_application_record).transpose()
}

#[async_trait]
impl ApplicationRepository for PgControlPlaneStore {
    async fn settle_application_archive_releases(
        &self,
        workspace_id: Uuid,
        digests: &[ApplicationArchiveReleaseDigest],
    ) -> Result<Vec<ApplicationArchiveRelease>> {
        let mut tx = self.pool().begin().await?;
        let mut ordered = digests.to_vec();
        ordered.sort_by_key(|digest| digest.application_id);
        let mut releases = Vec::with_capacity(ordered.len());

        for digest in ordered {
            let row = sqlx::query(
                r#"
                select release_version, release_digest
                from applications
                where workspace_id = $1 and id = $2
                for update
                "#,
            )
            .bind(workspace_id)
            .bind(digest.application_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
            let current_version: i64 = row.get("release_version");
            let current_digest: Option<String> = row.get("release_digest");
            let release_version = if current_digest.as_deref() == Some(&digest.release_digest) {
                current_version
            } else {
                current_version + 1
            };

            if current_digest.as_deref() != Some(&digest.release_digest) {
                sqlx::query(
                    r#"
                    update applications
                    set release_version = $3, release_digest = $4
                    where workspace_id = $1 and id = $2
                    "#,
                )
                .bind(workspace_id)
                .bind(digest.application_id)
                .bind(release_version)
                .bind(&digest.release_digest)
                .execute(&mut *tx)
                .await?;
            }
            releases.push(ApplicationArchiveRelease {
                application_id: digest.application_id,
                release_version,
                release_digest: digest.release_digest,
            });
        }
        tx.commit().await?;
        releases.sort_by_key(|release| {
            digests
                .iter()
                .position(|digest| digest.application_id == release.application_id)
                .unwrap_or(usize::MAX)
        });
        Ok(releases)
    }

    async fn record_application_extension_source(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        extension_installation_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into application_extension_sources (
                application_id, workspace_id, extension_installation_id, imported_by
            ) values ($1, $2, $3, $4)
            on conflict (application_id) do nothing
            "#,
        )
        .bind(application_id)
        .bind(workspace_id)
        .bind(extension_installation_id)
        .bind(actor_user_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn has_application_extension_source(
        &self,
        workspace_id: Uuid,
        extension_installation_id: Uuid,
    ) -> Result<bool> {
        Ok(sqlx::query_scalar(
            r#"
            select exists(
                select 1
                from application_extension_sources
                where workspace_id = $1 and extension_installation_id = $2
            )
            "#,
        )
        .bind(workspace_id)
        .bind(extension_installation_id)
        .fetch_one(self.pool())
        .await?)
    }

    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        let workspace_id = workspace_id_for_user(self.pool(), actor_user_id).await?;
        let tenant_id = tenant_id_for_workspace(self.pool(), workspace_id).await?;

        AuthRepository::load_actor_context(self, actor_user_id, tenant_id, workspace_id, None).await
    }

    async fn load_role_console_policies_for_user(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        let role_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            select role.id
            from user_role_bindings binding
            join roles role on role.id = binding.role_id
            where binding.user_id = $1
              and (role.scope_kind = 'system' or role.workspace_id = $2)
            order by role.scope_kind asc, role.code asc, role.id asc
            "#,
        )
        .bind(actor_user_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;
        let mut policies = Vec::with_capacity(role_ids.len());
        for role_id in role_ids {
            policies.push(
                crate::role_repository::role_console_policy_by_id(self.pool(), role_id).await?,
            );
        }
        Ok(policies)
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
                a.release_version,
                a.release_digest,
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
        let mut tx = self.pool().begin().await?;
        let application_id = Uuid::now_v7();
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
                release_version,
                release_digest,
                null::uuid as current_flow_id,
                null::uuid as current_draft_id,
                api_enabled,
                false as has_application_api_keys,
                false as has_application_api_mapping,
                null::uuid as active_publication_id,
                '[]'::jsonb as tags
            "#,
        )
        .bind(application_id)
        .bind(input.workspace_id)
        .bind(input.application_type.as_str())
        .bind(input.workflow_trigger_type.map(|value| value.as_str()))
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.icon_type.as_deref())
        .bind(input.icon.as_deref())
        .bind(input.icon_background.as_deref())
        .bind(input.actor_user_id)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(trigger_config) = &input.workflow_trigger_config {
            match trigger_config {
                CreateWorkflowTriggerConfig::Schedule {
                    cron,
                    timezone,
                    input_payload,
                } => {
                    sqlx::query(
                        r#"
                        insert into workflow_schedule_triggers (
                            id,
                            application_id,
                            scope_id,
                            enabled,
                            cron,
                            timezone,
                            input_payload,
                            created_by,
                            updated_by
                        ) values ($1, $2, $3, false, $4, $5, $6, $7, $7)
                        "#,
                    )
                    .bind(Uuid::now_v7())
                    .bind(application_id)
                    .bind(input.workspace_id)
                    .bind(cron)
                    .bind(timezone)
                    .bind(input_payload)
                    .bind(input.actor_user_id)
                    .execute(&mut *tx)
                    .await?;
                }
                CreateWorkflowTriggerConfig::Extension {
                    subpath,
                    http_method,
                    response_mode,
                } => {
                    sqlx::query(
                        r#"
                        insert into workflow_extension_triggers (
                            id,
                            application_id,
                            scope_id,
                            subpath,
                            http_method,
                            response_mode,
                            created_by,
                            updated_by
                        ) values ($1, $2, $3, $4, $5, $6, $7, $7)
                        "#,
                    )
                    .bind(Uuid::now_v7())
                    .bind(application_id)
                    .bind(input.workspace_id)
                    .bind(subpath)
                    .bind(http_method)
                    .bind(response_mode)
                    .bind(input.actor_user_id)
                    .execute(&mut *tx)
                    .await?;

                    let mapping_config = serde_json::json!({
                        "input": {
                            "query_target": "node-start.query",
                            "model_target": "node-start.model",
                            "inputs_target": "node-start",
                            "history_target": "node-start.history",
                            "attachments_target": "node-start.files"
                        },
                        "output": {
                            "answer_selector": null,
                            "usage_selector": null,
                            "files_selector": null,
                            "error_selector": null
                        },
                        "extension": {
                            "slug": subpath,
                            "method": http_method,
                            "response_mode": response_mode,
                        }
                    });
                    sqlx::query(
                        r#"
                        insert into application_api_mappings (
                            id,
                            application_id,
                            scope_id,
                            mapping_config,
                            extension_slug,
                            created_by,
                            updated_by
                        ) values ($1, $2, $3, $4, $5, $6, $6)
                        "#,
                    )
                    .bind(Uuid::now_v7())
                    .bind(application_id)
                    .bind(input.workspace_id)
                    .bind(mapping_config)
                    .bind(subpath)
                    .bind(input.actor_user_id)
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;
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
                icon = case when $6 then $7 else icon end,
                icon_type = case when $8 then $9 else icon_type end,
                icon_background = case when $10 then $11 else icon_background end,
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
        .bind(input.icon.is_some())
        .bind(input.icon.as_ref().and_then(|value| value.as_deref()))
        .bind(input.icon_type.is_some())
        .bind(input.icon_type.as_ref().and_then(|value| value.as_deref()))
        .bind(input.icon_background.is_some())
        .bind(
            input
                .icon_background
                .as_ref()
                .and_then(|value| value.as_deref()),
        )
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
                a.release_version,
                a.release_digest,
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
        find_application(
            self.pool(),
            workspace_id,
            application_id,
            None,
            ApplicationVisibility::All,
        )
        .await
    }

    async fn get_application_for_visibility(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> Result<Option<domain::ApplicationRecord>> {
        find_application(
            self.pool(),
            workspace_id,
            application_id,
            Some(actor_user_id),
            visibility,
        )
        .await
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

#[async_trait]
impl ApplicationManagementRepository for PgControlPlaneStore {
    async fn list_application_management(
        &self,
        workspace_id: Uuid,
        query: &ApplicationManagementQuery,
    ) -> Result<ApplicationManagementPage> {
        let mut count_builder = QueryBuilder::<Postgres>::new(
            "select count(*) from applications a where a.workspace_id = ",
        );
        count_builder.push_bind(workspace_id);
        append_application_management_filter(&mut count_builder, &query.filter)?;
        let total = count_builder
            .build_query_scalar::<i64>()
            .fetch_one(self.pool())
            .await?;

        let mut list_builder = QueryBuilder::<Postgres>::new(
            r#"
            select
                a.id,
                a.application_type,
                a.workflow_trigger_type,
                a.name,
                a.description,
                a.icon,
                a.icon_type,
                a.icon_background,
                a.created_by,
                coalesce(nullif(creator.nickname, ''), nullif(creator.name, ''), creator.account)
                    as created_by_display_name,
                a.created_at,
                a.updated_at,
                exists(
                    select 1
                    from application_publication_versions publication
                    where publication.application_id = a.id
                      and publication.active = true
                ) as published,
                coalesce(tags.tags, '[]'::jsonb) as tags
            from applications a
            join users creator on creator.id = a.created_by
            left join lateral (
                select jsonb_agg(
                    jsonb_build_object('id', tag.id, 'name', tag.name)
                    order by tag.name asc, tag.id asc
                ) as tags
                from application_tag_bindings binding
                join application_tags tag on tag.id = binding.tag_id
                where binding.application_id = a.id
            ) tags on true
            where a.workspace_id =
            "#,
        );
        list_builder.push_bind(workspace_id);
        append_application_management_filter(&mut list_builder, &query.filter)?;
        append_application_management_sort(
            &mut list_builder,
            query.sort_field,
            query.sort_direction,
        );
        list_builder.push(" limit ");
        list_builder.push_bind(query.page_size);
        list_builder.push(" offset ");
        list_builder.push_bind(query.page.saturating_sub(1).saturating_mul(query.page_size));

        let rows = list_builder.build().fetch_all(self.pool()).await?;
        let items = rows
            .into_iter()
            .map(|row| {
                let application_type =
                    parse_application_type(row.get::<String, _>("application_type").as_str())?;
                let workflow_trigger_type = row
                    .get::<Option<String>, _>("workflow_trigger_type")
                    .as_deref()
                    .map(parse_workflow_trigger_type)
                    .transpose()?;
                let published: bool = row.get("published");

                Ok(ApplicationManagementRecord {
                    id: row.get("id"),
                    application_type,
                    workflow_trigger_type,
                    name: row.get("name"),
                    description: row.get("description"),
                    icon: row.get("icon"),
                    icon_type: row.get("icon_type"),
                    icon_background: row.get("icon_background"),
                    created_by: row.get("created_by"),
                    created_by_display_name: row.get("created_by_display_name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                    tags: serde_json::from_value(row.get::<Value, _>("tags"))?,
                    publication_status: if published {
                        domain::ApplicationPublicationStatus::Published
                    } else {
                        domain::ApplicationPublicationStatus::Unpublished
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(ApplicationManagementPage {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })
    }
}

fn append_application_management_filter(
    builder: &mut QueryBuilder<Postgres>,
    filter: &domain::ResourceFilterExpr,
) -> Result<()> {
    if matches!(filter, domain::ResourceFilterExpr::All(items) if items.is_empty()) {
        return Ok(());
    }

    builder.push(" and ");
    append_application_management_filter_expr(builder, filter)
}

fn append_application_management_filter_expr(
    builder: &mut QueryBuilder<Postgres>,
    filter: &domain::ResourceFilterExpr,
) -> Result<()> {
    match filter {
        domain::ResourceFilterExpr::All(items) => {
            if items.is_empty() {
                builder.push("true");
                return Ok(());
            }
            builder.push("(");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    builder.push(" and ");
                }
                append_application_management_filter_expr(builder, item)?;
            }
            builder.push(")");
        }
        domain::ResourceFilterExpr::Any(items) => {
            if items.is_empty() {
                builder.push("false");
                return Ok(());
            }
            builder.push("(");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    builder.push(" or ");
                }
                append_application_management_filter_expr(builder, item)?;
            }
            builder.push(")");
        }
        domain::ResourceFilterExpr::Field {
            field,
            operator,
            value,
        } => {
            if field == "tags.id" {
                append_application_tag_filter(builder, *operator, value)?;
            } else {
                let expression = match field.as_str() {
                    "id" => "a.id::text",
                    "name" => "a.name",
                    "application_type" => "a.application_type",
                    "workflow_trigger_type" => "coalesce(a.workflow_trigger_type, '')",
                    "publication_status" => {
                        "case when exists (select 1 from application_publication_versions publication where publication.application_id = a.id and publication.active = true) then 'published' else 'unpublished' end"
                    }
                    "created_by" => "a.created_by::text",
                    _ => anyhow::bail!(ControlPlaneError::InvalidInput("filter")),
                };
                append_application_text_filter(builder, expression, *operator, value)?;
            }
        }
    }

    Ok(())
}

fn append_application_tag_filter(
    builder: &mut QueryBuilder<Postgres>,
    operator: domain::ResourceFilterOperator,
    value: &Value,
) -> Result<()> {
    builder.push(
        "exists (select 1 from application_tag_bindings filter_binding where filter_binding.application_id = a.id and ",
    );
    append_application_text_filter(builder, "filter_binding.tag_id::text", operator, value)?;
    builder.push(")");
    Ok(())
}

fn append_application_text_filter(
    builder: &mut QueryBuilder<Postgres>,
    expression: &'static str,
    operator: domain::ResourceFilterOperator,
    value: &Value,
) -> Result<()> {
    if operator == domain::ResourceFilterOperator::In {
        let values = value
            .as_array()
            .ok_or(ControlPlaneError::InvalidInput("filter"))?;
        if values.is_empty() {
            builder.push("false");
            return Ok(());
        }
        builder.push(expression);
        builder.push(" in (");
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                builder.push(", ");
            }
            builder.push_bind(application_management_filter_text(value)?);
        }
        builder.push(")");
        return Ok(());
    }

    builder.push(expression);
    builder.push(match operator {
        domain::ResourceFilterOperator::Eq => " = ",
        domain::ResourceFilterOperator::Ne => " <> ",
        domain::ResourceFilterOperator::Includes => " ilike ",
        domain::ResourceFilterOperator::NotIncludes => " not ilike ",
        _ => anyhow::bail!(ControlPlaneError::InvalidInput("filter")),
    });
    let value = application_management_filter_text(value)?;
    if matches!(
        operator,
        domain::ResourceFilterOperator::Includes | domain::ResourceFilterOperator::NotIncludes
    ) {
        builder.push_bind(format!("%{value}%"));
    } else {
        builder.push_bind(value);
    }
    Ok(())
}

fn application_management_filter_text(value: &Value) -> Result<String> {
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| ControlPlaneError::InvalidInput("filter").into())
}

fn append_application_management_sort(
    builder: &mut QueryBuilder<Postgres>,
    field: ApplicationManagementSortField,
    direction: ApplicationManagementSortDirection,
) {
    builder.push(" order by ");
    builder.push(match field {
        ApplicationManagementSortField::UpdatedAt => "a.updated_at",
        ApplicationManagementSortField::CreatedAt => "a.created_at",
        ApplicationManagementSortField::Name => "a.name",
        ApplicationManagementSortField::ApplicationType => "a.application_type",
    });
    builder.push(match direction {
        ApplicationManagementSortDirection::Asc => " asc",
        ApplicationManagementSortDirection::Desc => " desc",
    });
    builder.push(", a.id");
    builder.push(match direction {
        ApplicationManagementSortDirection::Asc => " asc",
        ApplicationManagementSortDirection::Desc => " desc",
    });
}
