use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        CatalogResolutionCandidate, CatalogResolutionRepository, DeleteCatalogTranslationInput,
        DeleteCustomCatalogMessageInput, I18nCatalogRepository, RuntimeCatalogMessage,
        RuntimeCatalogProjection, RuntimeI18nCatalogRepository, StoredI18nCatalogReleaseDescriptor,
        UpsertCatalogTranslationInput,
    },
};
use domain::{
    ActiveOfficialCatalogMessage, CatalogLocale, CatalogMessageIdentity, CatalogTranslation,
    ObsoleteCatalogMessage, OfficialCatalogMessage, WorkspaceCatalogRevision,
    WorkspaceCatalogState, I18N_CATALOG_SEED_SCHEMA_VERSION, I18N_CATALOG_SOURCE_LOCALE,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::PgControlPlaneStore;

mod management;

fn state_from_row(row: &sqlx::postgres::PgRow) -> Result<WorkspaceCatalogState> {
    Ok(WorkspaceCatalogState::restored(
        row.get("workspace_id"),
        row.get("active_release_id"),
        WorkspaceCatalogRevision::new(row.get("revision"))?,
    ))
}

fn translation_from_row(row: &sqlx::postgres::PgRow) -> Result<CatalogTranslation> {
    let identity = CatalogMessageIdentity::new(row.get::<String, _>("key"))?;
    CatalogTranslation::new(
        identity,
        CatalogLocale::new(row.get::<String, _>("locale"))?,
        row.get::<String, _>("translation"),
    )
    .map_err(Into::into)
}

fn obsolete_from_row(row: &sqlx::postgres::PgRow) -> Result<ObsoleteCatalogMessage> {
    let identity = CatalogMessageIdentity::new(row.get::<String, _>("key"))?;
    Ok(ObsoleteCatalogMessage::restored(
        identity,
        row.get("obsolete_since_release_id"),
    ))
}

pub(super) async fn lock_expected_state(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    expected_revision: WorkspaceCatalogRevision,
) -> Result<WorkspaceCatalogState> {
    let row = sqlx::query(
        r#"
        select workspace_id, active_release_id, revision
        from workspace_i18n_catalog_states
        where workspace_id = $1
        for update
        "#,
    )
    .bind(workspace_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ControlPlaneError::NotFound("workspace_i18n_catalog_state"))?;
    let state = state_from_row(&row)?;
    if state.revision() != expected_revision {
        return Err(ControlPlaneError::Conflict("i18n_catalog_revision").into());
    }
    Ok(state)
}

pub(super) async fn increment_state_revision(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
) -> Result<WorkspaceCatalogState> {
    let row = sqlx::query(
        r#"
        update workspace_i18n_catalog_states
        set revision = revision + 1, updated_at = now()
        where workspace_id = $1
        returning workspace_id, active_release_id, revision
        "#,
    )
    .bind(workspace_id)
    .fetch_one(&mut **transaction)
    .await?;
    state_from_row(&row)
}

pub(super) async fn insert_catalog_audit(
    transaction: &mut Transaction<'_, Postgres>,
    audit: &domain::AuditLogRecord,
) -> Result<()> {
    let scope_id = audit.workspace_id.unwrap_or(domain::SYSTEM_SCOPE_ID);
    sqlx::query(
        r#"
        insert into audit_logs (
          id, workspace_id, scope_id, actor_user_id, target_type, target_id,
          event_code, payload, created_by, updated_by, created_at, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $4, $4, $9, $9)
        "#,
    )
    .bind(audit.id)
    .bind(audit.workspace_id)
    .bind(scope_id)
    .bind(audit.actor_user_id)
    .bind(&audit.target_type)
    .bind(audit.target_id)
    .bind(&audit.event_code)
    .bind(&audit.payload)
    .bind(audit.created_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn reconcile_obsolete_rows(
    transaction: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    active_release_id: Uuid,
    superseded_release_id: Option<Uuid>,
) -> Result<()> {
    sqlx::query(
        r#"
        delete from workspace_i18n_catalog_obsolete_messages obsolete
        using i18n_catalog_release_messages active_message
        where obsolete.workspace_id = $1
          and active_message.release_id = $2
          and active_message.key = obsolete.key
        "#,
    )
    .bind(workspace_id)
    .bind(active_release_id)
    .execute(&mut **transaction)
    .await?;

    let Some(superseded_release_id) = superseded_release_id else {
        return Ok(());
    };
    sqlx::query(
        r#"
        insert into workspace_i18n_catalog_obsolete_messages (
          workspace_id, key, obsolete_since_release_id
        )
        select $1, historical.key, $2
        from i18n_catalog_release_messages historical
        join i18n_catalog_releases release on release.id = historical.release_id
        where historical.release_id = $3
          and release.workspace_id = $1
          and not exists (
            select 1
            from i18n_catalog_release_messages active_message
            where active_message.release_id = $2
              and active_message.key = historical.key
          )
        on conflict (workspace_id, key) do update
        set obsolete_since_release_id = excluded.obsolete_since_release_id,
            marked_at = now()
        "#,
    )
    .bind(workspace_id)
    .bind(active_release_id)
    .bind(superseded_release_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn list_obsolete(pool: &PgPool, workspace_id: Uuid) -> Result<Vec<ObsoleteCatalogMessage>> {
    sqlx::query(
        r#"
        select key, obsolete_since_release_id
        from workspace_i18n_catalog_obsolete_messages
        where workspace_id = $1
        order by key
        "#,
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(obsolete_from_row)
    .collect()
}

pub(crate) async fn insert_verified_release(
    transaction: &mut Transaction<'_, Postgres>,
    release: &domain::VerifiedCatalogRelease,
) -> Result<()> {
    let locales = release
        .locales()
        .iter()
        .map(|locale| locale.as_str().to_owned())
        .collect::<Vec<_>>();
    sqlx::query(
        r#"
        insert into i18n_catalog_releases (
          id, workspace_id, schema_version, catalog_version, source_locale,
          locales, generated_at, semantic_sha256
        ) values ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(release.id())
    .bind(release.workspace_id())
    .bind(I18N_CATALOG_SEED_SCHEMA_VERSION)
    .bind(release.catalog_version().as_str())
    .bind(I18N_CATALOG_SOURCE_LOCALE)
    .bind(locales)
    .bind(release.generated_at())
    .bind(release.semantic_sha256().as_str())
    .execute(&mut **transaction)
    .await?;

    for file in release.files() {
        sqlx::query(
            r#"
            insert into i18n_catalog_release_files (release_id, locale, path, sha256)
            values ($1, $2, $3, $4)
            "#,
        )
        .bind(release.id())
        .bind(file.locale().as_str())
        .bind(file.path())
        .bind(file.sha256().as_str())
        .execute(&mut **transaction)
        .await?;
    }
    for message in release.messages() {
        sqlx::query(
            r#"
            insert into i18n_catalog_release_messages (release_id, key)
            values ($1, $2)
            "#,
        )
        .bind(release.id())
        .bind(message.identity().key())
        .execute(&mut **transaction)
        .await?;
        for (locale, translation) in message.translations() {
            sqlx::query(
                r#"
                insert into i18n_catalog_release_translations (
                  release_id, key, locale, translation
                ) values ($1, $2, $3, $4)
                "#,
            )
            .bind(release.id())
            .bind(message.identity().key())
            .bind(locale.as_str())
            .bind(translation)
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(())
}

#[async_trait]
impl I18nCatalogRepository for PgControlPlaneStore {
    async fn import_verified_release(
        &self,
        release: &domain::VerifiedCatalogRelease,
    ) -> Result<()> {
        let mut transaction = self.pool().begin().await?;
        insert_verified_release(&mut transaction, release).await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn bootstrap_workspace_catalog_state(
        &self,
        workspace_id: Uuid,
    ) -> Result<WorkspaceCatalogState> {
        let row = sqlx::query(
            r#"
            insert into workspace_i18n_catalog_states (workspace_id)
            values ($1)
            on conflict (workspace_id) do update set workspace_id = excluded.workspace_id
            returning workspace_id, active_release_id, revision
            "#,
        )
        .bind(workspace_id)
        .fetch_one(self.pool())
        .await?;
        state_from_row(&row)
    }

    async fn activate_verified_release(
        &self,
        workspace_id: Uuid,
        release_id: Uuid,
        expected_revision: WorkspaceCatalogRevision,
    ) -> Result<WorkspaceCatalogState> {
        let mut transaction = self.pool().begin().await?;
        let previous_state =
            lock_expected_state(&mut transaction, workspace_id, expected_revision).await?;
        let release_exists: bool = sqlx::query_scalar(
            "select exists(select 1 from i18n_catalog_releases where workspace_id = $1 and id = $2)",
        )
        .bind(workspace_id)
        .bind(release_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !release_exists {
            return Err(ControlPlaneError::NotFound("verified_i18n_catalog_release").into());
        }
        let row = sqlx::query(
            r#"
            update workspace_i18n_catalog_states
            set active_release_id = $2, revision = revision + 1, updated_at = now()
            where workspace_id = $1
            returning workspace_id, active_release_id, revision
            "#,
        )
        .bind(workspace_id)
        .bind(release_id)
        .fetch_one(&mut *transaction)
        .await?;
        reconcile_obsolete_rows(
            &mut transaction,
            workspace_id,
            release_id,
            previous_state.active_release_id(),
        )
        .await?;
        let state = state_from_row(&row)?;
        transaction.commit().await?;
        Ok(state)
    }

    async fn get_workspace_catalog_state(
        &self,
        workspace_id: Uuid,
    ) -> Result<Option<WorkspaceCatalogState>> {
        sqlx::query(
            r#"
            select workspace_id, active_release_id, revision
            from workspace_i18n_catalog_states where workspace_id = $1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(state_from_row)
        .transpose()
    }

    async fn get_i18n_catalog_release_descriptor(
        &self,
        workspace_id: Uuid,
        release_id: Uuid,
    ) -> Result<Option<StoredI18nCatalogReleaseDescriptor>> {
        let row = sqlx::query(
            r#"
            select catalog_version, semantic_sha256, source_locale, locales
            from i18n_catalog_releases
            where workspace_id = $1 and id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(release_id)
        .fetch_optional(self.pool())
        .await?;
        row.map(|row| {
            Ok(StoredI18nCatalogReleaseDescriptor {
                catalog_version: domain::CatalogVersion::new(
                    row.get::<String, _>("catalog_version"),
                )?,
                semantic_sha256: domain::CatalogDigest::new(
                    row.get::<String, _>("semantic_sha256"),
                )?,
                source_locale: CatalogLocale::new(row.get::<String, _>("source_locale"))?,
                locales: row
                    .get::<Vec<String>, _>("locales")
                    .into_iter()
                    .map(CatalogLocale::new)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .transpose()
    }

    async fn list_active_official_messages(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<ActiveOfficialCatalogMessage>> {
        let rows = sqlx::query(
            r#"
            select message.release_id, message.key,
                   translation.locale, translation.translation
            from workspace_i18n_catalog_states state
            join i18n_catalog_release_messages message
              on message.release_id = state.active_release_id
            left join i18n_catalog_release_translations translation
              on translation.release_id = message.release_id
             and translation.key = message.key
            where state.workspace_id = $1
            order by message.key, translation.locale
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;
        let mut messages = std::collections::BTreeMap::<
            (Uuid, CatalogMessageIdentity),
            std::collections::BTreeMap<CatalogLocale, String>,
        >::new();
        for row in rows {
            let release_id = row.get("release_id");
            let identity = CatalogMessageIdentity::new(row.get::<String, _>("key"))?;
            let translations = messages.entry((release_id, identity)).or_default();
            if let (Some(locale), Some(translation)) = (
                row.get::<Option<String>, _>("locale"),
                row.get::<Option<String>, _>("translation"),
            ) {
                translations.insert(CatalogLocale::new(locale)?, translation);
            }
        }
        messages
            .into_iter()
            .map(|((release_id, identity), translations)| {
                Ok(ActiveOfficialCatalogMessage::restored(
                    release_id,
                    OfficialCatalogMessage::new(identity, translations)?,
                ))
            })
            .collect()
    }

    async fn list_catalog_overrides(&self, workspace_id: Uuid) -> Result<Vec<CatalogTranslation>> {
        list_workspace_translations(
            self.pool(),
            "workspace_i18n_catalog_overrides",
            workspace_id,
        )
        .await
    }

    async fn list_custom_catalog_translations(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<CatalogTranslation>> {
        list_workspace_translations(
            self.pool(),
            "workspace_i18n_catalog_custom_translations",
            workspace_id,
        )
        .await
    }

    async fn upsert_catalog_override(
        &self,
        input: &UpsertCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        upsert_workspace_translation(self.pool(), "workspace_i18n_catalog_overrides", input).await
    }

    async fn delete_catalog_override(
        &self,
        input: &DeleteCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        delete_workspace_translation(self.pool(), "workspace_i18n_catalog_overrides", input).await
    }

    async fn upsert_custom_catalog_translation(
        &self,
        input: &UpsertCatalogTranslationInput,
    ) -> Result<WorkspaceCatalogState> {
        upsert_workspace_translation(
            self.pool(),
            "workspace_i18n_catalog_custom_translations",
            input,
        )
        .await
    }

    async fn delete_custom_catalog_message(
        &self,
        input: &DeleteCustomCatalogMessageInput,
    ) -> Result<WorkspaceCatalogState> {
        let mut transaction = self.pool().begin().await?;
        lock_expected_state(
            &mut transaction,
            input.workspace_id,
            input.expected_revision,
        )
        .await?;
        let deleted = sqlx::query(
            r#"
            delete from workspace_i18n_catalog_custom_translations
            where workspace_id = $1 and key = $2
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.identity.key())
        .execute(&mut *transaction)
        .await?;
        if deleted.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("custom_i18n_catalog_message").into());
        }
        let state = increment_state_revision(&mut transaction, input.workspace_id).await?;
        transaction.commit().await?;
        Ok(state)
    }

    async fn mark_superseded_release_obsolete_against_active(
        &self,
        workspace_id: Uuid,
        superseded_release_id: Uuid,
    ) -> Result<Vec<ObsoleteCatalogMessage>> {
        let mut transaction = self.pool().begin().await?;
        let active_release_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            select active_release_id from workspace_i18n_catalog_states
            where workspace_id = $1 for update
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(&mut *transaction)
        .await?
        .flatten();
        let active_release_id =
            active_release_id.ok_or(ControlPlaneError::NotFound("active_i18n_catalog_release"))?;
        reconcile_obsolete_rows(
            &mut transaction,
            workspace_id,
            active_release_id,
            Some(superseded_release_id),
        )
        .await?;
        transaction.commit().await?;
        list_obsolete(self.pool(), workspace_id).await
    }

    async fn list_obsolete_catalog_messages(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<ObsoleteCatalogMessage>> {
        list_obsolete(self.pool(), workspace_id).await
    }
}

#[async_trait]
impl CatalogResolutionRepository for PgControlPlaneStore {
    async fn find_catalog_resolution_candidate(
        &self,
        workspace_id: Uuid,
        identity: &CatalogMessageIdentity,
        locale: &CatalogLocale,
    ) -> Result<CatalogResolutionCandidate> {
        let row = sqlx::query(
            r#"
            select
              coalesce(
                (select translation
                 from workspace_i18n_catalog_overrides
                 where workspace_id = $1 and key = $2 and locale = $3),
                (select translation
                 from workspace_i18n_catalog_custom_translations
                 where workspace_id = $1 and key = $2 and locale = $3)
              ) as root_override,
              (select translation.translation
               from workspace_i18n_catalog_states state
               join i18n_catalog_release_messages message
                 on message.release_id = state.active_release_id
                and message.key = $2
               join i18n_catalog_release_translations translation
                 on translation.release_id = message.release_id
                and translation.key = message.key
                and translation.locale = $3
               where state.workspace_id = $1) as active_official
            "#,
        )
        .bind(workspace_id)
        .bind(identity.key())
        .bind(locale.as_str())
        .fetch_one(self.pool())
        .await?;
        Ok(CatalogResolutionCandidate {
            root_override: row.get("root_override"),
            active_official: row.get("active_official"),
        })
    }
}

#[async_trait]
impl RuntimeI18nCatalogRepository for PgControlPlaneStore {
    async fn project_runtime_catalog(
        &self,
        workspace_id: Uuid,
        locale: &CatalogLocale,
    ) -> Result<RuntimeCatalogProjection> {
        let rows = sqlx::query(
            r#"
            with catalog_state as (
              select active_release_id, revision
              from workspace_i18n_catalog_states
              where workspace_id = $1
            ), identities as (
              select message.key
              from catalog_state state
              join i18n_catalog_release_messages message
                on message.release_id = state.active_release_id
              union
              select custom.key
              from workspace_i18n_catalog_custom_translations custom
              where custom.workspace_id = $1
            )
            select state.revision, identity.key,
                   coalesce(override_value.translation,
                            custom_value.translation,
                            official_value.translation,
                            identity.key) as value
            from catalog_state state
            left join identities identity on true
            left join workspace_i18n_catalog_overrides override_value
              on override_value.workspace_id = $1
             and override_value.key = identity.key
             and override_value.locale = $2
            left join workspace_i18n_catalog_custom_translations custom_value
              on custom_value.workspace_id = $1
             and custom_value.key = identity.key
             and custom_value.locale = $2
            left join i18n_catalog_release_translations official_value
              on official_value.release_id = state.active_release_id
             and official_value.key = identity.key
             and official_value.locale = $2
            order by identity.key
            "#,
        )
        .bind(workspace_id)
        .bind(locale.as_str())
        .fetch_all(self.pool())
        .await?;
        let first = rows
            .first()
            .ok_or(ControlPlaneError::NotFound("workspace_i18n_catalog_state"))?;
        let revision = WorkspaceCatalogRevision::new(first.get("revision"))?;
        let messages = rows
            .into_iter()
            .try_fold(Vec::new(), |mut messages, row| -> Result<_> {
                let Some(key) = row.get::<Option<String>, _>("key") else {
                    return Ok(messages);
                };
                messages.push(RuntimeCatalogMessage {
                    key,
                    value: row.get("value"),
                });
                Ok(messages)
            })?;
        Ok(RuntimeCatalogProjection { revision, messages })
    }
}

async fn list_workspace_translations(
    pool: &PgPool,
    table: &'static str,
    workspace_id: Uuid,
) -> Result<Vec<CatalogTranslation>> {
    let sql = format!(
        "select key, locale, translation from {table} where workspace_id = $1 order by key, locale"
    );
    sqlx::query(&sql)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?
        .iter()
        .map(translation_from_row)
        .collect()
}

async fn upsert_workspace_translation(
    pool: &PgPool,
    table: &'static str,
    input: &UpsertCatalogTranslationInput,
) -> Result<WorkspaceCatalogState> {
    let mut transaction = pool.begin().await?;
    lock_expected_state(
        &mut transaction,
        input.workspace_id,
        input.expected_revision,
    )
    .await?;
    let sql = format!(
        r#"
        insert into {table} (workspace_id, key, locale, translation)
        values ($1, $2, $3, $4)
        on conflict (workspace_id, key, locale) do update
        set translation = excluded.translation, updated_at = now()
        "#
    );
    sqlx::query(&sql)
        .bind(input.workspace_id)
        .bind(input.value.identity().key())
        .bind(input.value.locale().as_str())
        .bind(input.value.translation())
        .execute(&mut *transaction)
        .await?;
    let state = increment_state_revision(&mut transaction, input.workspace_id).await?;
    transaction.commit().await?;
    Ok(state)
}

async fn delete_workspace_translation(
    pool: &PgPool,
    table: &'static str,
    input: &DeleteCatalogTranslationInput,
) -> Result<WorkspaceCatalogState> {
    let mut transaction = pool.begin().await?;
    lock_expected_state(
        &mut transaction,
        input.workspace_id,
        input.expected_revision,
    )
    .await?;
    let sql = format!("delete from {table} where workspace_id = $1 and key = $2 and locale = $3");
    let deleted = sqlx::query(&sql)
        .bind(input.workspace_id)
        .bind(input.identity.key())
        .bind(input.locale.as_str())
        .execute(&mut *transaction)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(ControlPlaneError::NotFound("i18n_catalog_translation").into());
    }
    let state = increment_state_revision(&mut transaction, input.workspace_id).await?;
    transaction.commit().await?;
    Ok(state)
}
