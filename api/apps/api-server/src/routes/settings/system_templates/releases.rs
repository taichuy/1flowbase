//! Filesystem release discovery and boot composition; installation shares the typed API command.
use super::{
    interface::{TemplateAdapter, TemplateInput},
    plugins::TemplateDependencies,
};
use crate::app_state::ApiState;
use anyhow::{ensure, Context, Result};
use control_plane::{
    portable_template::{
        application_template_checksum, validate_application_template_release,
        PortableTemplateIdentityRepository, PortableTemplatePackage,
    },
    ports::FrontstagePageRepository,
};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path, sync::Arc};
use uuid::Uuid;

/// Discover only ROOT/@organization/name/template.json; receipts and unrelated files are ignored.
pub(crate) fn load_packages(root: &str) -> Result<Vec<PortableTemplatePackage>> {
    if root.is_empty() || !Path::new(root).exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for organization in std::fs::read_dir(root)? {
        let organization = organization?;
        if !organization.file_type()?.is_dir()
            || !organization.file_name().to_string_lossy().starts_with('@')
        {
            continue;
        }
        for template in std::fs::read_dir(organization.path())? {
            let template = template?;
            if template.file_type()?.is_dir() {
                let path = template.path().join("template.json");
                ensure!(
                    path.is_file(),
                    "application_template_package_missing:{}",
                    path.display()
                );
                paths.push(path);
            }
        }
    }
    paths.sort();
    let mut packages = Vec::new();
    let mut ids = BTreeSet::new();
    for path in paths {
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path)
            .with_context(|| format!("read application template {}", path.display()))?;
        let package: PortableTemplatePackage = serde_json::from_slice(&bytes)
            .with_context(|| format!("decode application template {}", path.display()))?;
        let release = package
            .release
            .as_ref()
            .context("application_template_release_missing")?;
        validate_application_template_release(release)?;
        ensure!(
            ids.insert(release.template_id.clone()),
            "application_template_duplicate_id:{}",
            release.template_id
        );
        packages.push(package);
    }
    Ok(packages)
}

pub(crate) async fn catalog<R: PortableTemplateIdentityRepository>(
    root: &str,
    repository: &R,
    workspace_id: Uuid,
) -> Result<Value> {
    let mut entries = Vec::new();
    for package in load_packages(root)? {
        let release = package
            .release
            .as_ref()
            .context("application_template_release_missing")?;
        let checksum = application_template_checksum(&package)?;
        let records = repository
            .load_application_template_releases(workspace_id, &release.template_id)
            .await?;
        let installed = records
            .iter()
            .filter(|r| r.successful)
            .max_by_key(|r| r.release_version);
        entries.push(json!({
            "template_id": release.template_id,
            "release_version": release.release_version,
            "name": release.name,
            "description": release.description,
            "checksum": checksum,
            "installed_release_version": installed.map(|r| r.release_version),
            "installed_checksum": installed.map(|r| &r.checksum),
            "package": package,
        }));
    }
    Ok(Value::Array(entries))
}

pub(crate) async fn synchronize_at_startup(
    state: &Arc<ApiState>,
    actor_user_id: Uuid,
    workspace_id: Uuid,
) -> Result<()> {
    let actor = FrontstagePageRepository::load_actor_context_for_workspace(
        &state.store,
        actor_user_id,
        workspace_id,
    )
    .await?;
    let dependencies = TemplateDependencies {
        store: state.store.clone(),
        application_template_root: state.application_template_root.clone(),
        runtime_registry_sync: crate::runtime_registry_sync::ApiRuntimeRegistrySync::new(
            state.store.clone(),
            state.runtime_engine.registry().clone(),
        ),
        provider_runtime: state.provider_runtime.clone(),
        official_plugin_source: state.official_plugin_source.clone(),
        official_catalog_source: state.official_extension_catalog_source.clone(),
        cache_store: state.infrastructure.cache_store(),
        provider_install_root: state.provider_install_root.clone(),
        api_node_id: state.api_node_id.clone(),
        mcp_interface_catalog:
            crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                store: state.store.clone(),
                openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                    store: state.store.clone(),
                    console_operations: state.console_operation_registry.inventory().clone(),
                    interface_registry: None,
                    api_docs: Arc::clone(&state.api_docs),
                    template_catalog: state.runtime_engine.template_catalog().clone(),
                },
            },
    };
    let adapter = TemplateAdapter(dependencies);
    for package in load_packages(&state.application_template_root)? {
        let template_id = package
            .release
            .as_ref()
            .context("application_template_release_missing")?
            .template_id
            .clone();
        match adapter
            .execute_inner(&actor, TemplateInput::Install(package))
            .await
        {
            Ok(output) if output.0.get("complete").and_then(Value::as_bool) == Some(true) => {
                tracing::info!(%template_id, "application template synchronized")
            }
            Ok(output) => {
                tracing::error!(%template_id, result = %output.0, "application template incomplete; retry on next startup or manual install")
            }
            Err(error) => {
                tracing::error!(%template_id, error = ?error, "application template failed; retry on next startup or manual install")
            }
        }
    }
    Ok(())
}
