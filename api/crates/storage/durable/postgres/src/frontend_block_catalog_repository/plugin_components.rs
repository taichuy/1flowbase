//! Host-owned reusable component projection. Runs on the catalog/install transaction connection.
use anyhow::{bail, Result};
use control_plane_contracts::{
    ports::ReplaceInstallationFrontendBlocksInput, ControlPlaneContractError,
};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(crate) async fn sync_plugin_components(
    connection: &mut PgConnection,
    installation_id: Uuid,
    input: &ReplaceInstallationFrontendBlocksInput,
    actor_user_id: Uuid,
) -> Result<()> {
    let installation = sqlx::query(
        "select organization, plugin_id, artifact_version, artifact_id, source_kind from extension_installations where id=$1",
    )
    .bind(installation_id)
    .fetch_one(&mut *connection)
    .await?;
    if !input.entries.is_empty()
        && (installation.get::<&str, _>("plugin_id") != input.plugin_id
            || installation.get::<&str, _>("artifact_version") != input.plugin_version
            || installation.get::<&str, _>("artifact_id") != input.provider_code)
    {
        bail!("frontend component installation identity mismatch");
    }
    // Built-in definitions keep their existing Official/catalog ownership.
    if installation.get::<&str, _>("source_kind") == "builtin" {
        return Ok(());
    }
    // Publisher and artifact come from the durable installation, including empty replacements.
    // Neither field can contain '/', so the canonical unversioned owner is unambiguous.
    let plugin_owner = format!(
        "{}/{}",
        installation.get::<&str, _>("organization"),
        installation.get::<&str, _>("artifact_id")
    );
    let plugin_id = plugin_owner.as_str();
    // Version-specific installation admission locks do not serialize different package versions.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 2015))")
        .bind(plugin_id)
        .execute(&mut *connection)
        .await?;
    let mut contributions = Vec::new();
    for entry in &input.entries {
        // A full template is the reusable source, not an invented import of the entry symbol.
        let Some(source) = entry
            .code_template
            .as_deref()
            .filter(|source| !source.trim().is_empty())
        else {
            continue;
        };
        if entry.runtime != "native_react" {
            continue;
        }
        contributions.push(entry.contribution_code.clone());
        let identity = serde_json::to_vec(&(plugin_id, entry.contribution_code.as_str()))?;
        let code = format!(
            "plugin.{:x}",
            base16ct::HexDisplay(&Sha256::digest(identity))
        );
        // Studio copies import_code + source_code. The complete template already owns its
        // imports; synthesizing imports from module metadata would declare bindings twice.
        // Provenance is opaque code content, not a second module import or a source parser.
        let import_code = format!(
            "// Plugin component: {}",
            serde_json::to_string(&(plugin_id, entry.contribution_code.as_str()))?
        );
        let updated = sqlx::query(
            r#"insert into ui_component_records (
                id, scope_id, component_code, name, description, import_code, source_code,
                origin, source, "group", upstream_identity, upstream_version, version, keywords,
                owner_plugin_id, owner_contribution_code, owner_installation_id, created_by, updated_by
            ) values ($1,$2,$3,$4,$4,$5,$6,'plugin','plugin','components',$7,$8,$8,$9,$10,$11,$12,$13,$13)
            on conflict (scope_id,component_code) do update set
                name=excluded.name, description=excluded.description, import_code=excluded.import_code,
                source_code=excluded.source_code, upstream_identity=excluded.upstream_identity,
                upstream_version=excluded.upstream_version, version=excluded.version, keywords=excluded.keywords,
                owner_installation_id=excluded.owner_installation_id, updated_by=excluded.updated_by, updated_at=now()
            where ui_component_records.origin='plugin'
                and ui_component_records.owner_plugin_id=excluded.owner_plugin_id
                and ui_component_records.owner_contribution_code=excluded.owner_contribution_code
            returning id"#,
        )
        .bind(Uuid::now_v7()).bind(domain::SYSTEM_SCOPE_ID).bind(code).bind(&entry.title)
        .bind(import_code).bind(source).bind(format!("{plugin_id}:{}", entry.contribution_code))
        .bind(&input.plugin_version).bind(vec![plugin_id.to_string(), entry.contribution_code.clone()])
        .bind(plugin_id).bind(&entry.contribution_code).bind(installation_id).bind(actor_user_id)
        .fetch_optional(&mut *connection).await?;
        if updated.is_none() {
            return Err(
                ControlPlaneContractError::Conflict("plugin_ui_component_owner_conflict").into(),
            );
        }
    }
    sqlx::query(
        "delete from ui_component_records where scope_id=$1 and origin='plugin' and owner_plugin_id=$2 and not (owner_contribution_code=any($3))",
    )
    .bind(domain::SYSTEM_SCOPE_ID).bind(plugin_id).bind(contributions)
    .execute(&mut *connection).await?;
    Ok(())
}
