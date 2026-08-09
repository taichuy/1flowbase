use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use control_plane::ports::{
    CreateUiCodeTemplateInput, ReviseUiCodeTemplateInput, ReviseUiComponentContractInput,
    UiManagementRepository,
};
use domain::{
    UiCodeTemplate, UiCodeTemplateLanguage, UiCodeTemplateRevision, UiComponentContractRevision,
    UiComponentLocator, UiComponentOverride, UiComponentOverrideState, SYSTEM_SCOPE_ID,
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

fn map_component(row: sqlx::postgres::PgRow) -> Result<UiComponentOverride> {
    let id: Uuid = row.get("id");
    let latest_revision = row
        .get::<Option<Uuid>, _>("latest_revision_id")
        .map(|revision_id| -> Result<_> {
            Ok(UiComponentContractRevision {
                id: revision_id,
                component_override_id: id,
                revision: row.get("latest_revision"),
                contract: serde_json::from_value(row.get("latest_contract"))?,
                is_latest: true,
                is_published: row.get("latest_is_published"),
                created_by: row.get("latest_created_by"),
                created_at: row.get("latest_created_at"),
            })
        })
        .transpose()?;
    let published_revision = row
        .get::<Option<Uuid>, _>("published_revision_id")
        .map(|revision_id| -> Result<_> {
            Ok(UiComponentContractRevision {
                id: revision_id,
                component_override_id: id,
                revision: row.get("published_revision"),
                contract: serde_json::from_value(row.get("published_contract"))?,
                is_latest: row.get("published_is_latest"),
                is_published: true,
                created_by: row.get("published_created_by"),
                created_at: row.get("published_created_at"),
            })
        })
        .transpose()?;
    Ok(UiComponentOverride {
        id,
        scope_id: row.get("scope_id"),
        locator: UiComponentLocator {
            provider_code: row.get("provider_code"),
            contribution_code: row.get("contribution_code"),
            module_source: row.get("module_source"),
            export_name: row.get("export_name"),
        },
        state: UiComponentOverrideState::try_from(row.get::<&str, _>("state"))?,
        latest_revision,
        published_revision,
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

const COMPONENT_SELECT: &str = r#"
select o.id, o.scope_id, o.provider_code, o.contribution_code, o.module_source, o.export_name,
    o.state, o.created_by, o.updated_by, o.created_at, o.updated_at,
    latest.id latest_revision_id, latest.revision latest_revision, latest.contract latest_contract,
    latest.is_published latest_is_published, latest.created_by latest_created_by,
    latest.created_at latest_created_at, published.id published_revision_id,
    published.revision published_revision, published.contract published_contract,
    published.is_latest published_is_latest, published.created_by published_created_by,
    published.created_at published_created_at
from ui_component_overrides o
left join ui_component_contract_revisions latest
    on latest.component_override_id = o.id and latest.is_latest
left join ui_component_contract_revisions published
    on published.component_override_id = o.id and published.is_published
"#;

async fn load_component(
    tx: &mut Transaction<'_, Postgres>,
    locator: &UiComponentLocator,
) -> Result<Option<UiComponentOverride>> {
    let query = format!(
        "{COMPONENT_SELECT} where o.scope_id = $1 and o.provider_code = $2 and o.contribution_code = $3 and o.module_source = $4 and o.export_name = $5"
    );
    sqlx::query(&query)
        .bind(SYSTEM_SCOPE_ID)
        .bind(&locator.provider_code)
        .bind(&locator.contribution_code)
        .bind(&locator.module_source)
        .bind(&locator.export_name)
        .fetch_optional(&mut **tx)
        .await?
        .map(map_component)
        .transpose()
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
        let query = format!("{COMPONENT_SELECT} order by o.module_source,o.export_name");
        sqlx::query(&query)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(map_component)
            .collect()
    }

    async fn get_ui_component_override(
        &self,
        locator: &UiComponentLocator,
    ) -> Result<Option<UiComponentOverride>> {
        let mut tx = self.pool().begin().await?;
        load_component(&mut tx, locator).await
    }

    async fn revise_ui_component_contract(
        &self,
        input: &ReviseUiComponentContractInput,
    ) -> Result<UiComponentOverride> {
        domain::validate_ui_component_contract(&input.locator, &input.contract)?;
        let mut tx = self.pool().begin().await?;
        let id:Uuid=sqlx::query_scalar("insert into ui_component_overrides (id,scope_id,provider_code,contribution_code,module_source,export_name,created_by,updated_by) values ($1,$2,$3,$4,$5,$6,$7,$7) on conflict (scope_id,provider_code,contribution_code,module_source,export_name) do update set updated_by=excluded.updated_by,updated_at=now() returning id")
            .bind(Uuid::now_v7()).bind(SYSTEM_SCOPE_ID).bind(&input.locator.provider_code).bind(&input.locator.contribution_code)
            .bind(&input.locator.module_source).bind(&input.locator.export_name).bind(input.actor_user_id).fetch_one(&mut *tx).await?;
        let next:i32=sqlx::query_scalar("select coalesce(max(revision),0)+1 from ui_component_contract_revisions where component_override_id=$1")
            .bind(id).fetch_one(&mut *tx).await?;
        sqlx::query("update ui_component_contract_revisions set is_latest=false where component_override_id=$1 and is_latest").bind(id).execute(&mut *tx).await?;
        sqlx::query("insert into ui_component_contract_revisions (id,component_override_id,revision,contract,is_latest,created_by) values ($1,$2,$3,$4,true,$5)")
            .bind(Uuid::now_v7()).bind(id).bind(next).bind(serde_json::to_value(&input.contract)?).bind(input.actor_user_id).execute(&mut *tx).await?;
        let value = load_component(&mut tx, &input.locator)
            .await?
            .context("component override missing")?;
        tx.commit().await?;
        Ok(value)
    }

    async fn set_ui_component_state(
        &self,
        locator: &UiComponentLocator,
        state: UiComponentOverrideState,
        actor_user_id: Uuid,
    ) -> Result<UiComponentOverride> {
        let mut tx = self.pool().begin().await?;
        let id:Uuid=sqlx::query_scalar("insert into ui_component_overrides (id,scope_id,provider_code,contribution_code,module_source,export_name,state,created_by,updated_by) values ($1,$2,$3,$4,$5,$6,$7,$8,$8) on conflict (scope_id,provider_code,contribution_code,module_source,export_name) do update set state=excluded.state,updated_by=excluded.updated_by,updated_at=now() returning id")
            .bind(Uuid::now_v7()).bind(SYSTEM_SCOPE_ID).bind(&locator.provider_code).bind(&locator.contribution_code)
            .bind(&locator.module_source).bind(&locator.export_name).bind(state.as_str()).bind(actor_user_id).fetch_one(&mut *tx).await?;
        if state == UiComponentOverrideState::Published {
            let changed=sqlx::query("update ui_component_contract_revisions set is_published=false where component_override_id=$1 and is_published").bind(id).execute(&mut *tx).await?;
            let latest=sqlx::query("update ui_component_contract_revisions set is_published=true where component_override_id=$1 and is_latest").bind(id).execute(&mut *tx).await?;
            let _ = changed;
            if latest.rows_affected() == 0 {
                bail!("component contract must be revised before publish");
            }
        }
        let value = load_component(&mut tx, locator)
            .await?
            .context("component override missing")?;
        tx.commit().await?;
        Ok(value)
    }
}
