use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use control_plane::ports::{
    CreateUiCodeTemplateInput, CreateUiComponentRecordInput, OfficialUiComponentCatalogRecord,
    ReviseUiCodeTemplateInput, ReviseUiComponentContractInput, UiComponentCatalogRepository,
    UiComponentRecordPatch, UiManagementRepository,
};
use domain::{
    UiCodeTemplate, UiCodeTemplateLanguage, UiCodeTemplateRevision, UiComponentLocator,
    UiComponentOverride, UiComponentOverrideState, UiComponentRecord, UiComponentRecordOrigin,
    UiComponentRecordUpstream, SYSTEM_SCOPE_ID,
};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn map_template(row: sqlx::postgres::PgRow) -> Result<UiCodeTemplate> {
    let language = UiCodeTemplateLanguage::try_from(row.get::<&str, _>("latest_language"))?;
    let published_revision = row
        .get::<Option<Uuid>, _>("published_revision_id")
        .map(|id| -> Result<_> {
            Ok(UiCodeTemplateRevision {
                id,
                template_id: row.get("id"),
                revision: row.get("published_revision"),
                source: row.get("published_source"),
                language: UiCodeTemplateLanguage::try_from(
                    row.get::<&str, _>("published_language"),
                )?,
                is_latest: row.get("published_is_latest"),
                is_published: true,
                created_by: row.get("published_created_by"),
                created_at: row.get("published_created_at"),
            })
        })
        .transpose()?;
    Ok(UiCodeTemplate {
        id: row.get("id"),
        scope_id: row.get("scope_id"),
        provider_code: row.get("provider_code"),
        contribution_code: row.get("contribution_code"),
        name: row.get("name"),
        latest_revision: UiCodeTemplateRevision {
            id: row.get("latest_revision_id"),
            template_id: row.get("id"),
            revision: row.get("latest_revision"),
            source: row.get("latest_source"),
            language,
            is_latest: true,
            is_published: row.get("latest_is_published"),
            created_by: row.get("latest_created_by"),
            created_at: row.get("latest_created_at"),
        },
        published_revision,
        is_default: row.get("is_default"),
        archived_at: row.get("archived_at"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

const TEMPLATE_SELECT: &str = r#"
select t.id, t.scope_id, t.provider_code, t.contribution_code, t.name, t.archived_at,
    t.created_by, t.updated_by, t.created_at, t.updated_at,
    latest.id latest_revision_id, latest.revision latest_revision, latest.source latest_source,
    latest.language latest_language, latest.is_published latest_is_published,
    latest.created_by latest_created_by, latest.created_at latest_created_at,
    published.id published_revision_id, published.revision published_revision,
    published.source published_source, published.language published_language,
    published.is_latest published_is_latest, published.created_by published_created_by,
    published.created_at published_created_at, (defaults.template_id is not null) is_default
from ui_code_templates t
join ui_code_template_revisions latest on latest.template_id = t.id and latest.is_latest
left join ui_code_template_revisions published on published.template_id = t.id and published.is_published
left join ui_code_template_defaults defaults on defaults.template_id = t.id
"#;

async fn load_template(
    tx: &mut Transaction<'_, Postgres>,
    template_id: Uuid,
) -> Result<Option<UiCodeTemplate>> {
    let query = format!("{TEMPLATE_SELECT} where t.id = $1");
    sqlx::query(&query)
        .bind(template_id)
        .fetch_optional(&mut **tx)
        .await?
        .map(map_template)
        .transpose()
}

fn map_component_record(row: sqlx::postgres::PgRow) -> Result<UiComponentRecord> {
    Ok(UiComponentRecord {
        id: row.get("id"),
        scope_id: row.get("scope_id"),
        component_code: row.get("component_code"),
        name: row.get("name"),
        description: row.get("description"),
        import_code: row.get("import_code"),
        source_code: row.get("source_code"),
        origin: UiComponentRecordOrigin::try_from(row.get::<&str, _>("origin"))?,
        source: row.get("source"),
        group: row.get("group"),
        upstream: UiComponentRecordUpstream {
            identity: row.get("upstream_identity"),
            version: row.get("upstream_version"),
        },
        version: row.get("version"),
        keywords: row.get("keywords"),
        catalog_updated_at: row.get("catalog_updated_at"),
        source_locator: row.get("source_locator"),
        source_checksum: row.get("source_checksum"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

const COMPONENT_RECORD_COLUMNS: &str = r#"id, scope_id, component_code, name, description, import_code, source_code, origin,
    source, "group", upstream_identity, upstream_version, version, keywords,
    catalog_updated_at, source_locator, source_checksum,
    created_by, updated_by, created_at, updated_at"#;

fn component_record_select() -> String {
    format!("select {COMPONENT_RECORD_COLUMNS} from ui_component_records")
}

fn validate_official_catalog_record(record: &OfficialUiComponentCatalogRecord) -> Result<()> {
    domain::validate_ui_component_record_fields(
        &record.component_code,
        &record.name,
        &record.description,
        &record.import_code,
        &record.source_code,
        UiComponentRecordOrigin::Official,
        &record.source,
        &record.group,
        &record.upstream,
        &record.version,
        &record.keywords,
    )?;
    let expected_prefix = format!("ui_components/@{}/{}/", record.source, record.group);
    if !record.source_locator.starts_with(&expected_prefix)
        || !record.source_locator.ends_with(".json")
    {
        bail!("official ui component source locator does not match source/group");
    }
    Ok(())
}

async fn upsert_official_catalog_record(
    tx: &mut Transaction<'_, Postgres>,
    record: &OfficialUiComponentCatalogRecord,
    actor_user_id: Uuid,
) -> Result<()> {
    let changed = sqlx::query(
        r#"insert into ui_component_records
        (id, scope_id, component_code, name, description, import_code, source_code, origin,
         source, "group", upstream_identity, upstream_version, version, keywords,
         catalog_updated_at, source_locator, source_checksum, created_by, updated_by)
        values ($1,$2,$3,$4,$5,$6,$7,'official',$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$17)
        on conflict (scope_id, component_code) do update set
            name=excluded.name, description=excluded.description,
            import_code=excluded.import_code, source_code=excluded.source_code,
            upstream_identity=excluded.upstream_identity,
            upstream_version=excluded.upstream_version, version=excluded.version,
            keywords=excluded.keywords, catalog_updated_at=excluded.catalog_updated_at,
            source_locator=excluded.source_locator, source_checksum=excluded.source_checksum,
            updated_by=excluded.updated_by, updated_at=now()
        where ui_component_records.origin='official'
          and ui_component_records.source=excluded.source
          and ui_component_records."group"=excluded."group""#,
    )
    .bind(Uuid::now_v7())
    .bind(SYSTEM_SCOPE_ID)
    .bind(&record.component_code)
    .bind(&record.name)
    .bind(&record.description)
    .bind(&record.import_code)
    .bind(&record.source_code)
    .bind(&record.source)
    .bind(&record.group)
    .bind(&record.upstream.identity)
    .bind(&record.upstream.version)
    .bind(&record.version)
    .bind(&record.keywords)
    .bind(record.catalog_updated_at)
    .bind(&record.source_locator)
    .bind(&record.source_checksum)
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await?;
    if changed.rows_affected() != 1 {
        bail!("ui component identity is owned by a custom record or another source/group");
    }
    Ok(())
}

async fn replace_official_group_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    source: &str,
    group: &str,
    records: &[OfficialUiComponentCatalogRecord],
    actor_user_id: Uuid,
) -> Result<()> {
    if records
        .iter()
        .any(|record| record.source != source || record.group != group)
    {
        bail!("official catalog records do not match authoritative source/group");
    }
    let mut codes = BTreeSet::new();
    for record in records {
        validate_official_catalog_record(record)?;
        if !codes.insert(record.component_code.clone()) {
            bail!("official catalog contains duplicate component_code");
        }
    }
    for record in records {
        upsert_official_catalog_record(tx, record, actor_user_id).await?;
    }
    let codes = codes.into_iter().collect::<Vec<_>>();
    sqlx::query(
        r#"delete from ui_component_records
        where scope_id=$1 and origin='official' and source=$2 and "group"=$3
          and not (component_code = any($4))"#,
    )
    .bind(SYSTEM_SCOPE_ID)
    .bind(source)
    .bind(group)
    .bind(&codes)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[async_trait]
impl UiManagementRepository for PgControlPlaneStore {
    async fn list_ui_code_templates(&self, include_archived: bool) -> Result<Vec<UiCodeTemplate>> {
        let query =
            format!("{TEMPLATE_SELECT} where ($1 or t.archived_at is null) order by t.name, t.id");
        sqlx::query(&query)
            .bind(include_archived)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(map_template)
            .collect()
    }

    async fn get_ui_code_template(&self, template_id: Uuid) -> Result<Option<UiCodeTemplate>> {
        let mut tx = self.pool().begin().await?;
        load_template(&mut tx, template_id).await
    }

    async fn create_ui_code_template(
        &self,
        input: &CreateUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate> {
        domain::validate_ui_code_template(&input.name, &input.source)?;
        let mut tx = self.pool().begin().await?;
        let id = Uuid::now_v7();
        sqlx::query("insert into ui_code_templates (id, scope_id, provider_code, contribution_code, name, created_by, updated_by) values ($1,$2,$3,$4,$5,$6,$6)")
            .bind(id).bind(SYSTEM_SCOPE_ID).bind(&input.provider_code).bind(&input.contribution_code)
            .bind(input.name.trim()).bind(input.actor_user_id).execute(&mut *tx).await?;
        sqlx::query("insert into ui_code_template_revisions (id, template_id, revision, source, language, is_latest, created_by) values ($1,$2,1,$3,$4,true,$5)")
            .bind(Uuid::now_v7()).bind(id).bind(&input.source).bind(input.language.as_str())
            .bind(input.actor_user_id).execute(&mut *tx).await?;
        let value = load_template(&mut tx, id)
            .await?
            .context("created template missing")?;
        tx.commit().await?;
        Ok(value)
    }

    async fn revise_ui_code_template(
        &self,
        input: &ReviseUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate> {
        domain::validate_ui_code_template(&input.name, &input.source)?;
        let mut tx = self.pool().begin().await?;
        let exists = sqlx::query("select id from ui_code_templates where id=$1 for update")
            .bind(input.template_id)
            .fetch_optional(&mut *tx)
            .await?;
        if exists.is_none() {
            bail!("ui code template not found");
        }
        let next: i32 = sqlx::query_scalar("select coalesce(max(revision),0)+1 from ui_code_template_revisions where template_id=$1")
            .bind(input.template_id).fetch_one(&mut *tx).await?;
        sqlx::query("update ui_code_template_revisions set is_latest=false where template_id=$1 and is_latest")
            .bind(input.template_id).execute(&mut *tx).await?;
        let changed = sqlx::query(
            "update ui_code_templates set name=$2, updated_by=$3, updated_at=now() where id=$1",
        )
        .bind(input.template_id)
        .bind(input.name.trim())
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 0 {
            bail!("ui code template not found");
        }
        sqlx::query("insert into ui_code_template_revisions (id, template_id, revision, source, language, is_latest, created_by) values ($1,$2,$3,$4,$5,true,$6)")
            .bind(Uuid::now_v7()).bind(input.template_id).bind(next).bind(&input.source)
            .bind(input.language.as_str()).bind(input.actor_user_id).execute(&mut *tx).await?;
        let value = load_template(&mut tx, input.template_id)
            .await?
            .context("revised template missing")?;
        tx.commit().await?;
        Ok(value)
    }

    async fn publish_ui_code_template_revision(
        &self,
        template_id: Uuid,
        revision: i32,
        actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("update ui_code_template_revisions set is_published=false where template_id=$1 and is_published")
            .bind(template_id).execute(&mut *tx).await?;
        let changed = sqlx::query("update ui_code_template_revisions set is_published=true where template_id=$1 and revision=$2")
            .bind(template_id).bind(revision).execute(&mut *tx).await?;
        if changed.rows_affected() == 0 {
            bail!("ui code template revision not found");
        }
        sqlx::query("update ui_code_templates set updated_by=$2, updated_at=now() where id=$1")
            .bind(template_id)
            .bind(actor_user_id)
            .execute(&mut *tx)
            .await?;
        let value = load_template(&mut tx, template_id)
            .await?
            .context("published template missing")?;
        tx.commit().await?;
        Ok(value)
    }

    async fn set_ui_code_template_default(
        &self,
        template_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query("select t.provider_code, t.contribution_code from ui_code_templates t join ui_code_template_revisions r on r.template_id=t.id and r.is_published where t.id=$1 and t.archived_at is null")
            .bind(template_id).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            bail!("only a published active template can be default");
        };
        sqlx::query("insert into ui_code_template_defaults (scope_id,provider_code,contribution_code,template_id,updated_by) values ($1,$2,$3,$4,$5) on conflict (scope_id,provider_code,contribution_code) do update set template_id=excluded.template_id,updated_by=excluded.updated_by,updated_at=now()")
            .bind(SYSTEM_SCOPE_ID).bind(row.get::<String,_>("provider_code")).bind(row.get::<String,_>("contribution_code"))
            .bind(template_id).bind(actor_user_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn reset_ui_code_template_default(
        &self,
        provider_code: &str,
        contribution_code: &str,
    ) -> Result<()> {
        sqlx::query("delete from ui_code_template_defaults where scope_id=$1 and provider_code=$2 and contribution_code=$3")
            .bind(SYSTEM_SCOPE_ID).bind(provider_code).bind(contribution_code).execute(self.pool()).await?;
        Ok(())
    }

    async fn set_ui_code_template_archived(
        &self,
        template_id: Uuid,
        archived: bool,
        actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        let mut tx = self.pool().begin().await?;
        if archived {
            sqlx::query("delete from ui_code_template_defaults where template_id=$1")
                .bind(template_id)
                .execute(&mut *tx)
                .await?;
        }
        let changed = sqlx::query("update ui_code_templates set archived_at=case when $2 then now() else null end,updated_by=$3,updated_at=now() where id=$1")
            .bind(template_id).bind(archived).bind(actor_user_id).execute(&mut *tx).await?;
        if changed.rows_affected() == 0 {
            bail!("ui code template not found");
        }
        let value = load_template(&mut tx, template_id)
            .await?
            .context("template missing")?;
        tx.commit().await?;
        Ok(value)
    }

    async fn list_ui_component_overrides(&self) -> Result<Vec<UiComponentOverride>> {
        // Temporary D4 compile boundary: Frontstage still consumes the legacy projection.
        // Its retired tables are gone, so it receives no overrides and uses official manifests.
        Ok(Vec::new())
    }

    async fn get_ui_component_override(
        &self,
        _locator: &UiComponentLocator,
    ) -> Result<Option<UiComponentOverride>> {
        Ok(None)
    }

    async fn revise_ui_component_contract(
        &self,
        _input: &ReviseUiComponentContractInput,
    ) -> Result<UiComponentOverride> {
        bail!("legacy ui component contract writes were removed by WP-D2")
    }

    async fn set_ui_component_state(
        &self,
        _locator: &UiComponentLocator,
        _state: UiComponentOverrideState,
        _actor_user_id: Uuid,
    ) -> Result<UiComponentOverride> {
        bail!("legacy ui component state writes were removed by WP-D2")
    }

    async fn list_ui_component_records(&self) -> Result<Vec<UiComponentRecord>> {
        let query = format!(
            "{} where scope_id = $1 order by name, component_code",
            component_record_select()
        );
        sqlx::query(&query)
            .bind(SYSTEM_SCOPE_ID)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(map_component_record)
            .collect()
    }

    async fn get_ui_component_record(&self, id: Uuid) -> Result<Option<UiComponentRecord>> {
        let query = format!(
            "{} where scope_id = $1 and id = $2",
            component_record_select()
        );
        sqlx::query(&query)
            .bind(SYSTEM_SCOPE_ID)
            .bind(id)
            .fetch_optional(self.pool())
            .await?
            .map(map_component_record)
            .transpose()
    }

    async fn create_ui_component_record(
        &self,
        input: &CreateUiComponentRecordInput,
    ) -> Result<UiComponentRecord> {
        domain::validate_ui_component_record_fields(
            &input.component_code,
            &input.name,
            &input.description,
            &input.import_code,
            &input.source_code,
            UiComponentRecordOrigin::Custom,
            &input.source,
            &input.group,
            &input.upstream,
            &input.version,
            &input.keywords,
        )?;
        let id = Uuid::now_v7();
        let query = format!(
            r#"insert into ui_component_records
            (id, scope_id, component_code, name, description, import_code, source_code, origin,
             source, "group", upstream_identity, upstream_version, version, keywords, created_by, updated_by)
            values ($1,$2,$3,$4,$5,$6,$7,'custom',$8,$9,$10,$11,$12,$13,$14,$14)
            returning {COMPONENT_RECORD_COLUMNS}"#
        );
        let row = sqlx::query(&query)
            .bind(id)
            .bind(SYSTEM_SCOPE_ID)
            .bind(&input.component_code)
            .bind(&input.name)
            .bind(&input.description)
            .bind(&input.import_code)
            .bind(&input.source_code)
            .bind(&input.source)
            .bind(&input.group)
            .bind(&input.upstream.identity)
            .bind(&input.upstream.version)
            .bind(&input.version)
            .bind(&input.keywords)
            .bind(input.actor_user_id)
            .fetch_one(self.pool())
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .and_then(|database| database.constraint())
                    == Some("ui_component_records_identity_unique")
                {
                    anyhow::Error::new(control_plane::errors::ControlPlaneError::Conflict(
                        "ui_component_code",
                    ))
                } else {
                    error.into()
                }
            })?;
        map_component_record(row)
    }

    async fn update_ui_component_record(
        &self,
        id: Uuid,
        patch: &UiComponentRecordPatch,
    ) -> Result<UiComponentRecord> {
        let current = self
            .get_ui_component_record(id)
            .await?
            .context("ui component record not found")?;
        if current.origin != UiComponentRecordOrigin::Custom {
            bail!("official ui component records are read-only");
        }
        domain::validate_ui_component_record_fields(
            &current.component_code,
            &patch.name,
            &patch.description,
            &patch.import_code,
            &patch.source_code,
            current.origin,
            &patch.source,
            &patch.group,
            &patch.upstream,
            &patch.version,
            &patch.keywords,
        )?;
        let query = format!(
            r#"update ui_component_records set name=$3, description=$4,
            import_code=$5, source_code=$6, source=$7, "group"=$8, upstream_identity=$9,
            upstream_version=$10, version=$11, keywords=$12, updated_by=$13, updated_at=now()
            where scope_id=$1 and id=$2 and origin='custom' returning {COMPONENT_RECORD_COLUMNS}"#
        );
        sqlx::query(&query)
            .bind(SYSTEM_SCOPE_ID)
            .bind(id)
            .bind(&patch.name)
            .bind(&patch.description)
            .bind(&patch.import_code)
            .bind(&patch.source_code)
            .bind(&patch.source)
            .bind(&patch.group)
            .bind(&patch.upstream.identity)
            .bind(&patch.upstream.version)
            .bind(&patch.version)
            .bind(&patch.keywords)
            .bind(patch.actor_user_id)
            .fetch_optional(self.pool())
            .await?
            .map(map_component_record)
            .transpose()?
            .context("custom ui component record not found")
    }

    async fn delete_ui_component_record(&self, id: Uuid) -> Result<bool> {
        Ok(sqlx::query(
            "delete from ui_component_records where scope_id=$1 and id=$2 and origin='custom'",
        )
        .bind(SYSTEM_SCOPE_ID)
        .bind(id)
        .execute(self.pool())
        .await?
        .rows_affected()
            == 1)
    }
}

#[async_trait]
impl UiComponentCatalogRepository for PgControlPlaneStore {
    async fn count_ui_component_records(&self) -> Result<usize> {
        let count: i64 =
            sqlx::query_scalar("select count(*) from ui_component_records where scope_id=$1")
                .bind(SYSTEM_SCOPE_ID)
                .fetch_one(self.pool())
                .await?;
        usize::try_from(count).context("ui component record count exceeds usize")
    }

    async fn list_official_ui_component_records(&self) -> Result<Vec<UiComponentRecord>> {
        let query = format!(
            "{} where scope_id=$1 and origin='official' order by source, \"group\", component_code",
            component_record_select()
        );
        sqlx::query(&query)
            .bind(SYSTEM_SCOPE_ID)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(map_component_record)
            .collect()
    }

    async fn upsert_official_ui_component_record(
        &self,
        record: &OfficialUiComponentCatalogRecord,
        actor_user_id: Uuid,
    ) -> Result<()> {
        validate_official_catalog_record(record)?;
        let mut tx = self.pool().begin().await?;
        upsert_official_catalog_record(&mut tx, record, actor_user_id).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn replace_official_ui_component_source_group(
        &self,
        source: &str,
        group: &str,
        records: &[OfficialUiComponentCatalogRecord],
        actor_user_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        replace_official_group_in_transaction(&mut tx, source, group, records, actor_user_id)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn replace_official_ui_component_catalog_groups(
        &self,
        records: &[OfficialUiComponentCatalogRecord],
        actor_user_id: Uuid,
    ) -> Result<bool> {
        let mut groups = BTreeMap::<(String, String), Vec<_>>::new();
        for record in records {
            groups
                .entry((record.source.clone(), record.group.clone()))
                .or_default()
                .push(record.clone());
        }
        let mut tx = self.pool().begin().await?;
        sqlx::query("lock table ui_component_records in share row exclusive mode")
            .execute(&mut *tx)
            .await?;
        let existing: i64 =
            sqlx::query_scalar("select count(*) from ui_component_records where scope_id=$1")
                .bind(SYSTEM_SCOPE_ID)
                .fetch_one(&mut *tx)
                .await?;
        if existing != 0 {
            tx.rollback().await?;
            return Ok(false);
        }
        for ((source, group), records) in groups {
            replace_official_group_in_transaction(
                &mut tx,
                &source,
                &group,
                &records,
                actor_user_id,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }
}
