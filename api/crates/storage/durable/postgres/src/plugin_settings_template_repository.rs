//! Startup owns native template application; defaults are retained at installation only.
use crate::PgControlPlaneStore;
use anyhow::{bail, Result};
use control_plane_contracts::ports::PluginRepository;
use control_plane_contracts::ports::PluginSettingsTemplateInput;
use domain::{NativePluginTarget, PluginInstallationRecord, SYSTEM_SCOPE_ID};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(crate) async fn retain_settings_template_defaults(
    connection: &mut PgConnection,
    installation: &PluginInstallationRecord,
    defaults: &[PluginSettingsTemplateInput],
) -> Result<()> {
    if !defaults.is_empty() && installation.category != domain::ExtensionCategory::HostExtensions {
        bail!("settings templates require a native host installation");
    }
    let owner = installation.plugin_id.split('@').next().unwrap_or_default();
    sqlx::query("delete from plugin_settings_template_defaults where installation_id=$1")
        .bind(installation.id)
        .execute(&mut *connection)
        .await?;
    for page in defaults {
        if !page.feature_id.starts_with(&format!("{owner}.")) {
            bail!("settings template feature is not owned by installation");
        }
        domain::validate_ui_code_template(&page.contribution_code, &page.source)?;
        sqlx::query("insert into plugin_settings_template_defaults (installation_id,contribution_code,feature_id,source,language) values ($1,$2,$3,$4,$5)")
            .bind(installation.id).bind(&page.contribution_code).bind(&page.feature_id)
            .bind(&page.source).bind(page.language.as_str()).execute(&mut *connection).await?;
    }
    Ok(())
}

pub(crate) async fn is_applied(
    store: &PgControlPlaneStore,
    target: &NativePluginTarget,
) -> Result<bool> {
    Ok(sqlx::query_scalar("select exists(select 1 from plugin_settings_template_applications a join native_plugin_targets t using(scope_id,category,organization,artifact_id,application_generation,installation_id) where t.enabled and a.scope_id=$1 and a.category=$2 and a.organization=$3 and a.artifact_id=$4 and a.installation_id=$5 and a.application_generation=$6)")
        .bind(target.scope_id).bind(target.category.as_str()).bind(&target.organization).bind(&target.artifact_id)
        .bind(target.installation_id).bind(target.application_generation).fetch_one(store.pool()).await?)
}

pub(crate) async fn apply_at_startup(
    store: &PgControlPlaneStore,
    target: &NativePluginTarget,
) -> Result<()> {
    let installation = store
        .get_installation(target.installation_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("selected native installation missing"))?;
    let mut tx = store.pool().begin().await?;
    crate::native_plugin_target_repository::lock_current_target(&mut tx, target).await?;
    let connection = &mut *tx;
    let applied: bool = sqlx::query_scalar("select exists(select 1 from plugin_settings_template_applications where scope_id=$1 and category=$2 and organization=$3 and artifact_id=$4 and application_generation=$5 and installation_id=$6)")
        .bind(target.scope_id).bind(target.category.as_str()).bind(&target.organization).bind(&target.artifact_id)
        .bind(target.application_generation).bind(target.installation_id).fetch_one(&mut *connection).await?;
    if applied {
        tx.commit().await?;
        return Ok(());
    }
    let actor_user_id = installation.created_by;
    let owner = installation.plugin_id.split('@').next().unwrap_or_default();
    let pages = sqlx::query("select contribution_code,feature_id,source,language from plugin_settings_template_defaults where installation_id=$1 order by contribution_code")
        .bind(installation.id).fetch_all(&mut *connection).await?;
    for page in pages {
        let code: String = page.get("contribution_code");
        let feature: String = page.get("feature_id");
        let source: String = page.get("source");
        let language: String = page.get("language");
        let current = sqlx::query("select id,owner_feature_id,owner_organization,owner_artifact_id from ui_code_templates where scope_id=$1 and owner_plugin_code=$2 and contribution_code=$3 for update")
            .bind(SYSTEM_SCOPE_ID).bind(owner).bind(&code).fetch_optional(&mut *connection).await?;
        let template_id = if let Some(current) = current {
            if current.get::<String, _>("owner_feature_id") != feature
                || current.get::<String, _>("owner_organization") != target.organization
                || current.get::<String, _>("owner_artifact_id") != target.artifact_id
            {
                bail!("settings template feature ownership changed");
            }
            current.get::<Uuid, _>("id")
        } else {
            let id = Uuid::now_v7();
            sqlx::query("insert into ui_code_templates (id,scope_id,provider_code,contribution_code,name,owner_plugin_code,owner_feature_id,applied_plugin_version,created_by,updated_by,applied_installation_id,applied_application_generation,owner_category,owner_organization,owner_artifact_id) values ($1,$2,$3,$4,$5,$3,$6,$7,$8,$8,$9,$10,$11,$12,$13)")
                .bind(id).bind(SYSTEM_SCOPE_ID).bind(owner).bind(&code).bind(&feature)
                .bind(&feature).bind(&installation.plugin_version).bind(actor_user_id)
                .bind(target.installation_id).bind(target.application_generation).bind(target.category.as_str()).bind(&target.organization).bind(&target.artifact_id)
                .execute(&mut *connection).await?;
            id
        };
        // Never replace an unrelated template selected under the same public provider/code.
        let default: Option<Uuid> = sqlx::query_scalar("select template_id from ui_code_template_defaults where scope_id=$1 and provider_code=$2 and contribution_code=$3 for update")
            .bind(SYSTEM_SCOPE_ID).bind(owner).bind(&code).fetch_optional(&mut *connection).await?;
        if default.is_some_and(|id| id != template_id) {
            bail!("settings template default is owned by another template");
        }
        let revision: i32 = sqlx::query_scalar("select coalesce(max(revision),0)+1 from ui_code_template_revisions where template_id=$1")
            .bind(template_id).fetch_one(&mut *connection).await?;
        sqlx::query("update ui_code_template_revisions set is_latest=false,is_published=false where template_id=$1 and (is_latest or is_published)")
            .bind(template_id).execute(&mut *connection).await?;
        sqlx::query("insert into ui_code_template_revisions (id,template_id,revision,source,language,is_latest,is_published,created_by) values ($1,$2,$3,$4,$5,true,true,$6)")
            .bind(Uuid::now_v7()).bind(template_id).bind(revision).bind(source).bind(language).bind(actor_user_id)
            .execute(&mut *connection).await?;
        sqlx::query("update ui_code_templates set applied_plugin_version=$2,archived_at=null,updated_by=$3,updated_at=now(),applied_installation_id=$4,applied_application_generation=$5,owner_category=$6,owner_organization=$7,owner_artifact_id=$8 where id=$1")
            .bind(template_id).bind(&installation.plugin_version).bind(actor_user_id)
            .bind(target.installation_id).bind(target.application_generation).bind(target.category.as_str()).bind(&target.organization).bind(&target.artifact_id).execute(&mut *connection).await?;
        let default_write = sqlx::query("insert into ui_code_template_defaults (scope_id,provider_code,contribution_code,template_id,updated_by) values ($1,$2,$3,$4,$5) on conflict (scope_id,provider_code,contribution_code) do update set template_id=excluded.template_id,updated_by=excluded.updated_by,updated_at=now() where ui_code_template_defaults.template_id=excluded.template_id")
            .bind(SYSTEM_SCOPE_ID).bind(owner).bind(&code).bind(template_id).bind(actor_user_id).execute(&mut *connection).await?;
        if default_write.rows_affected() != 1 {
            bail!("settings template default changed concurrently");
        }
    }
    sqlx::query("insert into plugin_settings_template_applications(scope_id,category,organization,artifact_id,application_generation,installation_id) values($1,$2,$3,$4,$5,$6)")
        .bind(target.scope_id).bind(target.category.as_str()).bind(&target.organization).bind(&target.artifact_id)
        .bind(target.application_generation).bind(target.installation_id).execute(&mut *connection).await?;
    tx.commit().await?;
    Ok(())
}
