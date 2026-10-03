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
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

pub(crate) struct BuiltinRelease {
    pub root: PathBuf,
    pub release: PortableTemplateRelease,
    pub checksum: String,
}
impl BuiltinRelease {
    pub fn load(&self) -> Result<PortableTemplatePackage> {
        if self.root.join("template.zip").is_file() {
            archive::decode(&std::fs::read(self.root.join("template.zip"))?)
        } else {
            archive::load_directory(&self.root)
        }
    }
}
pub(crate) fn discover(root: &str) -> Result<Vec<BuiltinRelease>> {
    if root.is_empty() || !Path::new(root).exists() {
        return Ok(Vec::new());
    }
    let mut results = Vec::new();
    let mut ids = BTreeSet::new();
    for organization in std::fs::read_dir(root)? {
        let organization = organization?;
        if !organization.file_type()?.is_dir()
            || !organization.file_name().to_string_lossy().starts_with('@')
        {
            continue;
        }
        for template in std::fs::read_dir(organization.path())? {
            let template = template?;
            if !template.file_type()?.is_dir() {
                continue;
            }
            let root = template.path();
            let (bytes, digest) = if root.join("template.zip").is_file() {
                let file = std::fs::File::open(root.join("template.zip"))?;
                let mut zip = zip::ZipArchive::new(file)?;
                let mut entry = zip.by_name("manifest.json")?;
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                // Metadata identity is stable without materializing each package.
                let digest = archive::checksum(&bytes);
                (bytes, digest)
            } else {
                let bytes = std::fs::read(root.join("manifest.json"))
                    .context("application_template_package_missing")?;
                let digest = archive::checksum(&bytes);
                (bytes, digest)
            };
            let manifest = archive::read_manifest(&bytes)?;
            let release: PortableTemplateRelease = serde_json::from_value(
                manifest
                    .package
                    .get("release")
                    .cloned()
                    .context("application_template_release_missing")?,
            )?;
            validate_application_template_release(&release)?;
            ensure!(
                ids.insert(release.template_id.clone()),
                "application_template_duplicate_id:{}",
                release.template_id
            );
            results.push(BuiltinRelease {
                root,
                release,
                checksum: digest,
            });
        }
    }
    results.sort_by(|a, b| a.release.template_id.cmp(&b.release.template_id));
    Ok(results)
}
pub(crate) fn load_packages(root: &str) -> Result<Vec<PortableTemplatePackage>> {
    discover(root)?.into_iter().map(|r| r.load()).collect()
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
