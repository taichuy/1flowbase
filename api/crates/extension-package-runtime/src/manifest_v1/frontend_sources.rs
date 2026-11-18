use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

use super::*;

const TEMPLATE_SOURCE_LIMIT: usize = 256 * 1024;
const TYPE_DECLARATIONS_LIMIT: usize = 2 * 1024 * 1024;

pub(crate) fn validate_package_source_path(path: &str) -> FrameworkResult<()> {
    if path.contains(['\\', ':', '\0'])
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(invalid(format!(
            "frontend source reference must be a package-relative file path: {path}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_template_source(
    block: &FrontendBlockContributionManifest,
) -> FrameworkResult<()> {
    if block.code_template.is_some() && block.code_template_file.is_some() {
        return Err(invalid(
            "code_template and code_template_file are mutually exclusive",
        ));
    }
    if let Some(source) = &block.code_template {
        validate_non_empty(source, "block_contributions[].code_template")?;
        if source.len() > TEMPLATE_SOURCE_LIMIT {
            return Err(invalid(
                "block_contributions[].code_template exceeds 262144 bytes",
            ));
        }
    }
    let has_source = block.code_template.is_some() || block.code_template_file.is_some();
    if !has_source {
        if block.code_template_version.is_some() || block.code_template_language.is_some() {
            return Err(invalid(
                "code_template or code_template_file is required with source metadata",
            ));
        }
        return Ok(());
    }
    let version = block.code_template_version.as_deref().ok_or_else(|| {
        invalid("block_contributions[].code_template_version is required with source")
    })?;
    validate_non_empty(version, "block_contributions[].code_template_version")?;
    let language = block.code_template_language.as_deref().ok_or_else(|| {
        invalid("block_contributions[].code_template_language is required with source")
    })?;
    validate_allowed(
        language,
        "block_contributions[].code_template_language",
        &["jsx", "tsx"],
    )?;
    if let Some(path) = &block.code_template_file {
        validate_package_source_path(path)?;
        if Path::new(path).extension().and_then(|ext| ext.to_str()) != Some(language) {
            return Err(invalid(
                "code_template_file extension must match code_template_language",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_module_type_source(
    module: &FrontendBlockCodeModuleManifest,
) -> FrameworkResult<()> {
    if let Some(path) = &module.type_declarations_file {
        if !module.type_declarations.is_empty() {
            return Err(invalid(
                "type_declarations and type_declarations_file are mutually exclusive",
            ));
        }
        validate_package_source_path(path)?;
        if !path.ends_with(".d.ts") {
            return Err(invalid(
                "type_declarations_file must reference a .d.ts file",
            ));
        }
        return Ok(());
    }
    validate_non_empty(
        &module.type_declarations,
        "block_contributions[].code_modules[].type_declarations",
    )
}

pub(crate) fn read_package_source(
    package_root: &Path,
    path: &str,
    limit: usize,
) -> FrameworkResult<String> {
    validate_package_source_path(path)?;
    let root = package_root
        .canonicalize()
        .map_err(|error| invalid(error.to_string()))?;
    let resolved = root
        .join(path)
        .canonicalize()
        .map_err(|error| invalid(format!("frontend source {path} is unavailable: {error}")))?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return Err(invalid(format!(
            "frontend source {path} must be a file within its package"
        )));
    }
    let file = File::open(&resolved).map_err(|error| invalid(error.to_string()))?;
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > limit {
        return Err(invalid(format!(
            "frontend source {path} exceeds {limit} bytes"
        )));
    }
    let source = String::from_utf8(bytes).map_err(|error| invalid(error.to_string()))?;
    if source.trim().is_empty() {
        return Err(invalid(format!("frontend source {path} is empty")));
    }
    Ok(source)
}

/// Resolve package files into owned installation snapshots; never rewrite the signed manifest.
/// The cache is local to this preparation, so sibling contributions share identical file bytes.
pub fn resolve_plugin_frontend_sources(
    package_root: &Path,
    manifest: &mut PluginManifestV1,
) -> FrameworkResult<()> {
    let mut types = BTreeMap::new();
    for block in &mut manifest.block_contributions {
        validate_template_source(block)?;
        if let Some(path) = &block.code_template_file {
            block.code_template = Some(read_package_source(
                package_root,
                path,
                TEMPLATE_SOURCE_LIMIT,
            )?);
            block.code_template_file = None;
        }
        resolve_module_sources(package_root, &mut block.code_modules, &mut types)?;
    }
    for page in &mut manifest.settings_pages {
        resolve_module_sources(package_root, &mut page.code_modules, &mut types)?;
    }
    Ok(())
}

/// Settings-page registries load the same module contract from a native declaration.
pub fn resolve_frontend_code_modules(
    package_root: &Path,
    modules: &mut [FrontendBlockCodeModuleManifest],
) -> FrameworkResult<()> {
    resolve_module_sources(package_root, modules, &mut BTreeMap::new())
}

fn resolve_module_sources(
    package_root: &Path,
    modules: &mut [FrontendBlockCodeModuleManifest],
    types: &mut BTreeMap<String, String>,
) -> FrameworkResult<()> {
    for module in modules {
        validate_module_type_source(module)?;
        if let Some(path) = &module.type_declarations_file {
            let source = match types.get(path) {
                Some(source) => source.clone(),
                None => {
                    let source = read_package_source(package_root, path, TYPE_DECLARATIONS_LIMIT)?;
                    types.insert(path.clone(), source.clone());
                    source
                }
            };
            module.type_declarations = source;
            module.type_declarations_file = None;
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}
