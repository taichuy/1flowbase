use std::{collections::{BTreeMap, BTreeSet}, io::{Cursor, Read}};
use control_plane::errors::ControlPlaneError;
use sha2::{Digest, Sha256};
use zip::ZipArchive;
pub const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_BUNDLE_FILES: usize = 256;
pub fn parse_bundle_archive(bytes: &[u8]) -> Result<domain::McpBundlePackage, ControlPlaneError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    if archive.is_empty() || archive.len() > MAX_BUNDLE_FILES {
        return Err(ControlPlaneError::InvalidInput("mcp_bundle_file_count"));
    }

    let mut entries = BTreeMap::<String, Vec<u8>>::new();
    let mut total_size = 0_usize;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
        if file.is_dir() {
            continue;
        }
        let path = file
            .enclosed_name()
            .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_path"))?
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_empty() || entries.contains_key(&path) {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_path"));
        }
        let declared_size = usize::try_from(file.size())
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_size"))?;
        total_size = total_size
            .checked_add(declared_size)
            .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_size"))?;
        if total_size > MAX_BUNDLE_BYTES {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_size"));
        }
        let mut content = Vec::with_capacity(declared_size);
        file.read_to_end(&mut content)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
        entries.insert(path, content);
    }

    let manifest_bytes = entries
        .remove("manifest.json")
        .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_manifest"))?;
    let manifest = serde_json::from_slice::<domain::McpBundleManifest>(&manifest_bytes)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_manifest"))?;
    if manifest.files.len() != entries.len() {
        return Err(ControlPlaneError::InvalidInput("mcp_bundle_files"));
    }

    let mut declared_paths = BTreeSet::new();
    let mut tools = Vec::new();
    let mut instances = Vec::new();
    let mut connections = Vec::new();
    for file in &manifest.files {
        if !declared_paths.insert(file.path.as_str()) {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_files"));
        }
        let content = entries
            .get(&file.path)
            .ok_or(ControlPlaneError::InvalidInput("mcp_bundle_files"))?;
        let actual_sha256 = format!(
            "sha256:{:x}",
            base16ct::HexDisplay(&Sha256::digest(content))
        );
        if actual_sha256 != file.sha256 {
            return Err(ControlPlaneError::InvalidInput("mcp_bundle_checksum"));
        }
        match file.kind {
            domain::McpBundleFileKind::Tool if file.path.starts_with("tools/") => {
                tools.push(
                    serde_json::from_slice(content)
                        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_tool"))?,
                );
            }
            domain::McpBundleFileKind::Instance if file.path.starts_with("instances/") => {
                instances.push(
                    serde_json::from_slice(content)
                        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_instance"))?,
                );
            }
            domain::McpBundleFileKind::Connection if file.path.starts_with("connections/") => {
                connections.push(
                    serde_json::from_slice(content)
                        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_connection"))?,
                );
            }
            _ => return Err(ControlPlaneError::InvalidInput("mcp_bundle_file_kind")),
        }
    }

    Ok(domain::McpBundlePackage {
        manifest,
        tools,
        instances,
        connections,
    })
}

#[cfg(test)]
#[path = "_tests/mcp_bundle_archive.rs"]
mod tests;
