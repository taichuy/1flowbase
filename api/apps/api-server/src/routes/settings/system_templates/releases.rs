//! Built-in split releases and composition; installation shares the typed API command.
use super::{
    archive,
    interface::{TemplateAdapter, TemplateInput},
    plugins::TemplateDependencies,
};
use crate::app_state::ApiState;
use anyhow::{ensure, Context, Result};
use control_plane::{
    portable_template::{
        validate_application_template_release, PortableTemplatePackage, PortableTemplateRelease,
    },
    ports::FrontstagePageRepository,
};
use serde_json::Value;
use std::{collections::BTreeSet, io::Read, path::PathBuf, sync::Arc};
use uuid::Uuid;

pub(crate) struct BuiltinRelease {
    pub root: PathBuf,
    pub release: PortableTemplateRelease,
    pub checksum: String,
}
impl BuiltinRelease {
    pub async fn load(&self) -> Result<PortableTemplatePackage> {
        let zip_path = self.root.join("template.zip");
        if tokio::fs::metadata(&zip_path)
            .await
            .is_ok_and(|m| m.is_file())
        {
            let bytes = tokio::fs::read(zip_path).await?;
            tokio::task::spawn_blocking(move || archive::decode(&bytes))
                .await
                .context("application_template_builtin_decode_task")?
        } else {
            archive::load_directory(&self.root).await
        }
    }
}
pub(crate) async fn discover(root: &str) -> Result<Vec<BuiltinRelease>> {
    if root.is_empty() || !tokio::fs::try_exists(root).await? {
        return Ok(Vec::new());
    }
    enum MetadataInput {
        Zip(std::fs::File),
        Manifest(Vec<u8>),
    }
    let mut results = Vec::new();
    let mut ids = BTreeSet::new();
    let mut organizations = tokio::fs::read_dir(root).await?;
    while let Some(organization) = organizations.next_entry().await? {
        if !organization.file_type().await?.is_dir()
            || !organization.file_name().to_string_lossy().starts_with('@')
        {
            continue;
        }
        let mut templates = tokio::fs::read_dir(organization.path()).await?;
        while let Some(template) = templates.next_entry().await? {
            if !template.file_type().await?.is_dir() {
                continue;
            }
            let root = template.path();
            let zip_path = root.join("template.zip");
            let zipped = tokio::fs::metadata(&zip_path)
                .await
                .is_ok_and(|m| m.is_file());
            let input = if zipped {
                // Open asynchronously, then read only ZIP metadata on the blocking
                // pool instead of buffering every package while listing releases.
                MetadataInput::Zip(tokio::fs::File::open(zip_path).await?.into_std().await)
            } else {
                MetadataInput::Manifest(
                    tokio::fs::read(root.join("manifest.json"))
                        .await
                        .context("application_template_package_missing")?,
                )
            };
            let item = tokio::task::spawn_blocking(move || {
                let bytes = match input {
                    MetadataInput::Zip(file) => {
                        let mut zip = zip::ZipArchive::new(file)?;
                        let mut entry = zip.by_name("manifest.json")?;
                        let mut manifest = Vec::new();
                        entry.read_to_end(&mut manifest)?;
                        manifest
                    }
                    MetadataInput::Manifest(bytes) => bytes,
                };
                // Metadata identity is stable without materializing each package.
                let checksum = archive::checksum(&bytes);
                let manifest = archive::read_manifest(&bytes)?;
                let release: PortableTemplateRelease = serde_json::from_value(
                    manifest
                        .package
                        .get("release")
                        .cloned()
                        .context("application_template_release_missing")?,
                )?;
                validate_application_template_release(&release)?;
                Ok::<_, anyhow::Error>(BuiltinRelease {
                    root,
                    release,
                    checksum,
                })
            })
            .await
            .context("application_template_discovery_decode_task")??;
            ensure!(
                ids.insert(item.release.template_id.clone()),
                "application_template_duplicate_id:{}",
                item.release.template_id
            );
            results.push(item);
        }
    }
    results.sort_by(|a, b| a.release.template_id.cmp(&b.release.template_id));
    Ok(results)
}
pub(crate) async fn load_packages(root: &str) -> Result<Vec<PortableTemplatePackage>> {
    let mut packages = Vec::new();
    for release in discover(root).await? {
        packages.push(release.load().await?);
    }
    Ok(packages)
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
        bootstrap_workspace_id: state.bootstrap_workspace_id,
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
    for package in load_packages(&state.application_template_root).await? {
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
