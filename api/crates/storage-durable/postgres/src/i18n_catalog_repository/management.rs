use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuditedCatalogTranslationInput, AuditedDeleteCatalogTranslationInput,
        AuditedDeleteCustomCatalogMessageInput, AuditedRestoreAllCatalogOverridesInput,
        CatalogManagementEntry, CatalogManagementOrigin, CatalogManagementPage,
        CatalogManagementQuery, I18nCatalogManagementRepository,
    },
};
use domain::{CatalogLocale, WorkspaceCatalogRevision, WorkspaceCatalogState};
use sqlx::{Postgres, Row, Transaction};

use super::{increment_state_revision, insert_catalog_audit, lock_expected_state};
use crate::PgControlPlaneStore;

fn origin_name(origin: CatalogManagementOrigin) -> &'static str {
    match origin {
        CatalogManagementOrigin::Official => "official",
        CatalogManagementOrigin::OfficialOverride => "official_override",
        CatalogManagementOrigin::Custom => "custom",
        CatalogManagementOrigin::English => "english",
    }
}

fn parse_origin(value: &str) -> Result<CatalogManagementOrigin> {
    match value {
        "official" => Ok(CatalogManagementOrigin::Official),
        "official_override" => Ok(CatalogManagementOrigin::OfficialOverride),
        "custom" => Ok(CatalogManagementOrigin::Custom),
        "english" => Ok(CatalogManagementOrigin::English),
        _ => Err(ControlPlaneError::InvalidInput("i18n_catalog_origin").into()),
    }
}

fn validate_audit_workspace(
    workspace_id: uuid::Uuid,
    audit: &domain::AuditLogRecord,
) -> Result<()> {
    if audit.workspace_id != Some(workspace_id) || audit.actor_user_id.is_none() {
        return Err(ControlPlaneError::InvalidInput("i18n_catalog_audit_scope").into());
    }
    Ok(())
}

async fn official_identity_exists(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: uuid::Uuid,
    key: &str,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        r#"
        select exists(
          select 1
          from workspace_i18n_catalog_states state
          join i18n_catalog_release_messages message
            on message.release_id = state.active_release_id
          where state.workspace_id = $1 and message.key = $2
        )
        "#,
    )
    .bind(workspace_id)
    .bind(key)
    .fetch_one(&mut **transaction)
    .await?)
}

async fn finish_mutation(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: uuid::Uuid,
    audit: &domain::AuditLogRecord,
) -> Result<WorkspaceCatalogState> {
    let state = increment_state_revision(transaction, workspace_id).await?;
    insert_catalog_audit(transaction, audit).await?;
    Ok(state)
}

#[async_trait]
impl I18nCatalogManagementRepository for PgControlPlaneStore {
    async fn list_catalog_management_entries(
        &self,
        query: &CatalogManagementQuery,
    ) -> Result<CatalogManagementPage> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query("set transaction isolation level repeatable read")
            .execute(&mut *transaction)
            .await?;
        let revision_value: i64 = sqlx::query_scalar(
            "select revision from workspace_i18n_catalog_states where workspace_id = $1",
        )
        .bind(query.workspace_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(ControlPlaneError::NotFound("workspace_i18n_catalog_state"))?;
        let origin = query.origin.map(origin_name);
        let rows = sqlx::query(
            r#"
            with official_entries as (
              select message.key, locale.locale,
                     translation.translation as official_translation,
                     override_value.translation as override_translation,
                     null::text as custom_translation,
                     coalesce(override_value.translation, translation.translation, message.key) as effective_value,
                     case when override_value.translation is not null then 'official_override'
                          when translation.translation is not null then 'official'
                          else 'english' end as origin,
                     translation.translation is null as missing,
                     obsolete.key is not null as obsolete
              from workspace_i18n_catalog_states state
              join i18n_catalog_releases release on release.id = state.active_release_id
              join i18n_catalog_release_messages message on message.release_id = release.id
              cross join lateral unnest(release.locales) locale(locale)
              left join i18n_catalog_release_translations translation
                on translation.release_id = message.release_id
               and translation.key = message.key
               and translation.locale = locale.locale
              left join workspace_i18n_catalog_overrides override_value
                on override_value.workspace_id = state.workspace_id
               and override_value.key = message.key
               and override_value.locale = locale.locale
              left join workspace_i18n_catalog_obsolete_messages obsolete
                on obsolete.workspace_id = state.workspace_id
               and obsolete.key = message.key
              where state.workspace_id = $1
            ), custom_entries as (
              select custom.key, custom.locale,
                     null::text as official_translation, null::text as override_translation,
                     custom.translation as custom_translation,
                     custom.translation as effective_value, 'custom'::text as origin,
                     false as missing, false as obsolete
              from workspace_i18n_catalog_custom_translations custom
              where custom.workspace_id = $1
            ), obsolete_entries as (
              select obsolete.key, locale.locale,
                     null::text as official_translation,
                     override_value.translation as override_translation,
                     null::text as custom_translation,
                     coalesce(override_value.translation, obsolete.key) as effective_value,
                     case when override_value.translation is not null then 'official_override'
                          else 'english' end as origin,
                     true as missing, true as obsolete
              from workspace_i18n_catalog_obsolete_messages obsolete
              join workspace_i18n_catalog_states state
                on state.workspace_id = obsolete.workspace_id
              join i18n_catalog_releases release on release.id = state.active_release_id
              cross join lateral unnest(release.locales) locale(locale)
              left join workspace_i18n_catalog_overrides override_value
                on override_value.workspace_id = obsolete.workspace_id
               and override_value.key = obsolete.key
               and override_value.locale = locale.locale
              where obsolete.workspace_id = $1
            ), filtered as (
              select * from (
                select * from official_entries
                union all select * from custom_entries
                union all select * from obsolete_entries
              ) entries
              where ($2::text is null or key = $2)
                and ($3::text is null or locale = $3)
                and ($4::text is null or lower(key || ' ' || effective_value)
                     like '%' || lower($4) || '%')
                and ($5::text is null or origin = $5)
            )
            select *, count(*) over() as total
            from filtered order by key, locale
            offset $6 limit $7
            "#,
        )
        .bind(query.workspace_id)
        .bind(query.key.as_deref())
        .bind(query.locale.as_ref().map(CatalogLocale::as_str))
        .bind(query.search.as_deref().filter(|value| !value.trim().is_empty()))
        .bind(origin)
        .bind(i64::from(query.offset))
        .bind(i64::from(query.limit))
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let total = rows
            .first()
            .map(|row| row.get::<i64, _>("total") as u64)
            .unwrap_or(0);
        let revision = WorkspaceCatalogRevision::new(revision_value)?;
        let entries = rows
            .into_iter()
            .map(|row| {
                Ok(CatalogManagementEntry {
                    key: row.get("key"),
                    locale: CatalogLocale::new(row.get::<String, _>("locale"))?,
                    official_translation: row.get("official_translation"),
                    override_translation: row.get("override_translation"),
                    custom_translation: row.get("custom_translation"),
                    effective_value: row.get("effective_value"),
                    origin: parse_origin(row.get("origin"))?,
                    missing: row.get("missing"),
                    obsolete: row.get("obsolete"),
                    revision,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(CatalogManagementPage {
            entries,
            total,
            revision,
        })
    }

    async fn upsert_official_catalog_override(
        &self,
        input: &AuditedCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        validate_audit_workspace(input.workspace_id, &input.audit)?;
        let mut transaction = self.pool().begin().await?;
        lock_expected_state(
            &mut transaction,
            input.workspace_id,
            input.expected_revision,
        )
        .await?;
        if !official_identity_exists(
            &mut transaction,
            input.workspace_id,
            input.value.identity().key(),
        )
        .await?
        {
            return Err(ControlPlaneError::NotFound("official_i18n_catalog_message").into());
        }
        sqlx::query(
            r#"
            insert into workspace_i18n_catalog_overrides
              (workspace_id, key, locale, translation)
            values ($1, $2, $3, $4)
            on conflict (workspace_id, key, locale) do update
            set translation = excluded.translation, updated_at = now()
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.value.identity().key())
        .bind(input.value.locale().as_str())
        .bind(input.value.translation())
        .execute(&mut *transaction)
        .await?;
        let state = finish_mutation(&mut transaction, input.workspace_id, &input.audit).await?;
        transaction.commit().await?;
        Ok(state)
    }

    async fn upsert_custom_catalog_translation_audited(
        &self,
        input: &AuditedCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        validate_audit_workspace(input.workspace_id, &input.audit)?;
        let mut transaction = self.pool().begin().await?;
        lock_expected_state(
            &mut transaction,
            input.workspace_id,
            input.expected_revision,
        )
        .await?;
        if official_identity_exists(
            &mut transaction,
            input.workspace_id,
            input.value.identity().key(),
        )
        .await?
        {
            return Err(
                ControlPlaneError::Conflict("custom_i18n_catalog_official_identity").into(),
            );
        }
        sqlx::query(
            r#"
            insert into workspace_i18n_catalog_custom_translations
              (workspace_id, key, locale, translation)
            values ($1, $2, $3, $4)
            on conflict (workspace_id, key, locale) do update
            set translation = excluded.translation, updated_at = now()
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.value.identity().key())
        .bind(input.value.locale().as_str())
        .bind(input.value.translation())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            insert into workspace_i18n_catalog_custom_translations
              (workspace_id, key, locale, translation)
            select $1, $2, 'en_US', $2
            where not exists (
              select 1 from workspace_i18n_catalog_overrides
              where workspace_id = $1 and key = $2 and locale = 'en_US'
            )
            on conflict (workspace_id, key, locale) do nothing
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.value.identity().key())
        .execute(&mut *transaction)
        .await?;
        let state = finish_mutation(&mut transaction, input.workspace_id, &input.audit).await?;
        transaction.commit().await?;
        Ok(state)
    }

    async fn restore_official_catalog_translation(
        &self,
        input: &AuditedDeleteCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        validate_audit_workspace(input.workspace_id, &input.audit)?;
        let mut transaction = self.pool().begin().await?;
        lock_expected_state(
            &mut transaction,
            input.workspace_id,
            input.expected_revision,
        )
        .await?;
        let deleted = sqlx::query(
            r#"delete from workspace_i18n_catalog_overrides
               where workspace_id = $1 and key = $2 and locale = $3"#,
        )
        .bind(input.workspace_id)
        .bind(input.identity.key())
        .bind(input.locale.as_str())
        .execute(&mut *transaction)
        .await?;
        if deleted.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("i18n_catalog_override").into());
        }
        let state = finish_mutation(&mut transaction, input.workspace_id, &input.audit).await?;
        transaction.commit().await?;
        Ok(state)
    }

    async fn restore_all_official_catalog_overrides(
        &self,
        input: &AuditedRestoreAllCatalogOverridesInput,
    ) -> Result<WorkspaceCatalogState> {
        validate_audit_workspace(input.workspace_id, &input.audit)?;
        let mut transaction = self.pool().begin().await?;
        lock_expected_state(
            &mut transaction,
            input.workspace_id,
            input.expected_revision,
        )
        .await?;
        sqlx::query("delete from workspace_i18n_catalog_overrides where workspace_id = $1")
            .bind(input.workspace_id)
            .execute(&mut *transaction)
            .await?;
        let state = finish_mutation(&mut transaction, input.workspace_id, &input.audit).await?;
        transaction.commit().await?;
        Ok(state)
    }

    async fn delete_custom_catalog_message_audited(
        &self,
        input: &AuditedDeleteCustomCatalogMessageInput,
    ) -> Result<WorkspaceCatalogState> {
        validate_audit_workspace(input.workspace_id, &input.audit)?;
        let mut transaction = self.pool().begin().await?;
        lock_expected_state(
            &mut transaction,
            input.workspace_id,
            input.expected_revision,
        )
        .await?;
        let deleted = sqlx::query(
            r#"delete from workspace_i18n_catalog_custom_translations
               where workspace_id = $1 and key = $2"#,
        )
        .bind(input.workspace_id)
        .bind(input.identity.key())
        .execute(&mut *transaction)
        .await?;
        if deleted.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("custom_i18n_catalog_message").into());
        }
        let state = finish_mutation(&mut transaction, input.workspace_id, &input.audit).await?;
        transaction.commit().await?;
        Ok(state)
    }
}
