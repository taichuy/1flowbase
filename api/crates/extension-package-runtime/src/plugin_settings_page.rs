//! Fixed native settings page declarations; route and API authority stays in the host registry.
use crate::{FrameworkResult, PluginFrameworkError};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSettingsPageManifest {
    pub feature_id: String,
    pub contribution_code: String,
    pub source_file: String,
    pub language: String,
}

pub fn validate_plugin_settings_pages(
    owner: &str,
    pages: &[PluginSettingsPageManifest],
) -> FrameworkResult<()> {
    let mut features = BTreeSet::new();
    let mut templates = BTreeSet::new();
    for page in pages {
        if !page.feature_id.starts_with(&format!("{owner}."))
            || page.feature_id.len() <= owner.len() + 1
            || !features.insert(&page.feature_id)
            || !templates.insert(&page.contribution_code)
            || page.contribution_code.is_empty()
            || !page
                .contribution_code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(invalid(
                "settings_pages requires unique plugin-owned features and template codes",
            ));
        }
        let path = Path::new(&page.source_file);
        if page.source_file.is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || !matches!(page.language.as_str(), "jsx" | "tsx")
            || path.extension().and_then(|ext| ext.to_str()) != Some(page.language.as_str())
        {
            return Err(invalid(
                "settings_pages source_file must be a package-relative jsx/tsx file",
            ));
        }
    }
    Ok(())
}

pub fn read_plugin_settings_page_source(
    package_root: &Path,
    page: &PluginSettingsPageManifest,
) -> FrameworkResult<String> {
    let root = package_root
        .canonicalize()
        .map_err(|error| invalid(&error.to_string()))?;
    let source_path = root
        .join(&page.source_file)
        .canonicalize()
        .map_err(|error| invalid(&error.to_string()))?;
    if !source_path.starts_with(&root) {
        return Err(invalid("settings page source escapes its package"));
    }
    // Bound the read itself, including packages changed concurrently with loading.
    use std::io::Read;
    let file = std::fs::File::open(source_path).map_err(|error| invalid(&error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(262145)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid(&error.to_string()))?;
    if bytes.len() > 262144 {
        return Err(invalid("settings page source exceeds 262144 bytes"));
    }
    let source = String::from_utf8(bytes).map_err(|error| invalid(&error.to_string()))?;
    if source.trim().is_empty() {
        return Err(invalid("settings page source is empty"));
    }
    Ok(source)
}

fn invalid(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}
