use anyhow::Result;
use control_plane::ports::CommitPluginInstallationInput;
use uuid::Uuid;

use crate::{plugin_repository::map_installation, repositories::PgControlPlaneStore};

pub(crate) async fn commit_plugin_installation(
    store: &PgControlPlaneStore,
    input: &CommitPluginInstallationInput,
) -> Result<domain::PluginInstallationRecord> {
    let mut tx = store.pool().begin().await?;
    let installation = &input.installation;
    let row = sqlx::query(
        r#"
            insert into extension_installations (
                id, scope_id, category, organization, artifact_id, artifact_version,
                plugin_id, contract_version, protocol, display_name, source_kind, trust_level,
                verification_status, desired_state, expected_checksum, signature_status,
                signature_algorithm, signing_key_id, application_action, metadata_json,
                is_system_reserved, created_by, updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23
            )
            on conflict (plugin_id) where plugin_id is not null do update
            set category = excluded.category,
                organization = excluded.organization,
                artifact_id = excluded.artifact_id,
                artifact_version = excluded.artifact_version,
                contract_version = excluded.contract_version,
                protocol = excluded.protocol,
                display_name = excluded.display_name,
                source_kind = excluded.source_kind,
                trust_level = excluded.trust_level,
                verification_status = excluded.verification_status,
                desired_state = excluded.desired_state,
                expected_checksum = excluded.expected_checksum,
                signature_status = excluded.signature_status,
                signature_algorithm = excluded.signature_algorithm,
                signing_key_id = excluded.signing_key_id,
                receipt = extension_installations.receipt - 'legacy_manifest_compatibility',
                application_action = excluded.application_action,
                metadata_json = excluded.metadata_json,
                is_system_reserved = excluded.is_system_reserved,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning id, scope_id, category, organization, artifact_id as provider_code,
                plugin_id, artifact_version as plugin_version, contract_version, protocol,
                display_name, source_kind, trust_level, verification_status, desired_state,
                expected_checksum, signature_status, signature_algorithm, signing_key_id,
                receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility,
                metadata_json, is_system_reserved, created_by, updated_by, created_at, updated_at
            "#,
    )
    .bind(installation.installation_id)
    .bind(domain::SYSTEM_SCOPE_ID)
    .bind(installation.category.as_str())
    .bind(&installation.organization)
    .bind(&installation.provider_code)
    .bind(&installation.plugin_version)
    .bind(&installation.plugin_id)
    .bind(&installation.contract_version)
    .bind(&installation.protocol)
    .bind(&installation.display_name)
    .bind(&installation.source_kind)
    .bind(&installation.trust_level)
    .bind(installation.verification_status.as_str())
    .bind(installation.desired_state.as_str())
    .bind(installation.expected_checksum.as_deref())
    .bind(installation.signature_status.as_str())
    .bind(&installation.signature_algorithm)
    .bind(&installation.signing_key_id)
    .bind(
        if installation.category == domain::ExtensionCategory::RuntimeExtensions
            && installation
                .metadata_json
                .get("plugin_type")
                .and_then(serde_json::Value::as_str)
                == Some("model_provider")
        {
            "configure_model_provider"
        } else {
            "none"
        },
    )
    .bind(&installation.metadata_json)
    .bind(installation.is_system_reserved)
    .bind(installation.actor_user_id)
    .bind(installation.actor_user_id)
    .fetch_one(&mut *tx)
    .await?;
    let record = map_installation(row)?;
    let installation_id = record.id;

    let artifact = &input.artifact_instance;
    sqlx::query(
        r#"
            insert into extension_artifact_instances (
                node_id, installation_id, local_version, local_checksum, local_path,
                package_path, manifest_fingerprint, artifact_status, runtime_status,
                availability_status, checked_at, last_error, is_current
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            on conflict (node_id, installation_id) do update
            set local_version = excluded.local_version,
                local_checksum = excluded.local_checksum,
                local_path = excluded.local_path,
                package_path = excluded.package_path,
                manifest_fingerprint = excluded.manifest_fingerprint,
                artifact_status = excluded.artifact_status,
                runtime_status = excluded.runtime_status,
                availability_status = excluded.availability_status,
                checked_at = excluded.checked_at,
                last_error = excluded.last_error,
                is_current = excluded.is_current
            "#,
    )
    .bind(&artifact.node_id)
    .bind(installation_id)
    .bind(&artifact.local_version)
    .bind(&artifact.local_checksum)
    .bind(&artifact.local_path)
    .bind(&artifact.package_path)
    .bind(&artifact.manifest_fingerprint)
    .bind(artifact.artifact_status.as_str())
    .bind(artifact.runtime_status.as_str())
    .bind(artifact.availability_status.as_str())
    .bind(artifact.checked_at)
    .bind(&artifact.last_error)
    .bind(artifact.is_current)
    .execute(&mut *tx)
    .await?;

    if let Some(package_catalog) = &input.package_catalog {
        sqlx::query(
            r#"
                insert into plugin_package_catalog_projection (
                    installation_id, package_code, package_version, catalog_snapshot_json,
                    projection_status, last_error_message, refreshed_at
                ) values ($1, $2, $3, $4, $5, $6, $7)
                on conflict (installation_id) do update
                set package_code = excluded.package_code,
                    package_version = excluded.package_version,
                    catalog_snapshot_json = excluded.catalog_snapshot_json,
                    projection_status = excluded.projection_status,
                    last_error_message = excluded.last_error_message,
                    refreshed_at = excluded.refreshed_at,
                    updated_at = now()
                "#,
        )
        .bind(installation_id)
        .bind(&package_catalog.package_code)
        .bind(&package_catalog.package_version)
        .bind(&package_catalog.catalog_snapshot_json)
        .bind(package_catalog.projection_status.as_str())
        .bind(&package_catalog.last_error_message)
        .bind(package_catalog.refreshed_at)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("delete from node_contribution_registry where installation_id = $1")
        .bind(installation_id)
        .execute(&mut *tx)
        .await?;
    for entry in &input.node_contributions.entries {
        sqlx::query(
            r#"
                insert into node_contribution_registry (
                    id, scope_id, installation_id, provider_code, plugin_unique_identifier,
                    package_id, plugin_id, plugin_version, contribution_code, node_shell,
                    category, title, description, icon, schema_ui, schema_version, output_schema,
                    contribution_checksum, compiled_contribution_hash, output_schema_snapshot,
                    side_effect_policy, infra_contracts, required_auth, visibility, experimental,
                    dependency_installation_kind, dependency_plugin_version_range
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                    $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27
                )
                "#,
        )
        .bind(Uuid::now_v7())
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(installation_id)
        .bind(&input.node_contributions.provider_code)
        .bind(&entry.plugin_unique_identifier)
        .bind(&entry.package_id)
        .bind(&input.node_contributions.plugin_id)
        .bind(&input.node_contributions.plugin_version)
        .bind(&entry.contribution_code)
        .bind(&entry.node_shell)
        .bind(&entry.category)
        .bind(&entry.title)
        .bind(&entry.description)
        .bind(&entry.icon)
        .bind(&entry.schema_ui)
        .bind(&entry.schema_version)
        .bind(&entry.output_schema)
        .bind(&entry.contribution_checksum)
        .bind(&entry.compiled_contribution_hash)
        .bind(&entry.output_schema_snapshot)
        .bind(&entry.side_effect_policy)
        .bind(serde_json::to_value(&entry.infra_contracts)?)
        .bind(serde_json::to_value(&entry.required_auth)?)
        .bind(&entry.visibility)
        .bind(entry.experimental)
        .bind(&entry.dependency_installation_kind)
        .bind(&entry.dependency_plugin_version_range)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("delete from js_dependency_registry where installation_id = $1")
        .bind(installation_id)
        .execute(&mut *tx)
        .await?;
    for entry in &input.js_dependencies.entries {
        sqlx::query(
            r#"
                insert into js_dependency_registry (
                    id, scope_id, installation_id, provider_code, plugin_id, plugin_version,
                    alias, package, version, target, artifact_path, integrity,
                    permission_network, permission_filesystem, permission_env
                ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                "#,
        )
        .bind(Uuid::now_v7())
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(installation_id)
        .bind(&input.js_dependencies.provider_code)
        .bind(&input.js_dependencies.plugin_id)
        .bind(&input.js_dependencies.plugin_version)
        .bind(&entry.alias)
        .bind(&entry.package)
        .bind(&entry.version)
        .bind(&entry.target)
        .bind(&entry.artifact_path)
        .bind(&entry.integrity)
        .bind(&entry.permissions.network)
        .bind(&entry.permissions.filesystem)
        .bind(&entry.permissions.env)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("delete from frontend_block_catalog where installation_id = $1")
        .bind(installation_id)
        .execute(&mut *tx)
        .await?;
    for entry in &input.frontend_blocks.entries {
        let context_contract = serde_json::json!({
            "primitives": entry.context_contract.primitives,
            "input_schema": entry.context_contract.input_schema,
        });
        sqlx::query(
            r#"
                insert into frontend_block_catalog (
                    id, scope_id, installation_id, provider_code, plugin_id, plugin_version,
                    contribution_code, title, runtime, entry, code_template,
                    code_template_version, code_template_language, code_modules, context_contract,
                    permission_network, permission_storage, permission_secrets, ui_capabilities
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                    $14, $15, $16, $17, $18, $19
                )
                "#,
        )
        .bind(Uuid::now_v7())
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(installation_id)
        .bind(&input.frontend_blocks.provider_code)
        .bind(&input.frontend_blocks.plugin_id)
        .bind(&input.frontend_blocks.plugin_version)
        .bind(&entry.contribution_code)
        .bind(&entry.title)
        .bind(&entry.runtime)
        .bind(&entry.entry)
        .bind(&entry.code_template)
        .bind(&entry.code_template_version)
        .bind(&entry.code_template_language)
        .bind(serde_json::to_value(&entry.code_modules)?)
        .bind(context_contract)
        .bind(&entry.permissions.network)
        .bind(&entry.permissions.storage)
        .bind(&entry.permissions.secrets)
        .bind(serde_json::to_value(&entry.ui_capabilities)?)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(record)
}
