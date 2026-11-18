use control_plane::errors::ControlPlaneError;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::io::Write;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
pub fn build_bundle_archive(
    mut package: domain::McpBundlePackage,
) -> Result<Vec<u8>, ControlPlaneError> {
    let mut files = Vec::<(String, Vec<u8>, domain::McpBundleFileKind)>::new();
    for tool in &package.tools {
        let path = bundle_file_path("tools", &tool.tool_id);
        let content = serde_json::to_vec_pretty(tool)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_tool"))?;
        files.push((path, content, domain::McpBundleFileKind::Tool));
    }
    for instance in &package.instances {
        let path = bundle_file_path("instances", &instance.instance_id);
        let content = serde_json::to_vec_pretty(instance)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_instance"))?;
        files.push((path, content, domain::McpBundleFileKind::Instance));
    }
    for connection in &package.connections {
        let path = bundle_file_path("connections", &connection.connection_id.to_string());
        let content = serde_json::to_vec_pretty(connection)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_connection"))?;
        files.push((path, content, domain::McpBundleFileKind::Connection));
    }
    files.sort_by(|left, right| {
        bundle_file_kind_rank(left.2)
            .cmp(&bundle_file_kind_rank(right.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    package.manifest.files = files
        .iter()
        .map(|(path, content, kind)| domain::McpBundleFile {
            path: path.clone(),
            kind: *kind,
            sha256: format!(
                "sha256:{:x}",
                base16ct::HexDisplay(&Sha256::digest(content))
            ),
        })
        .collect();
    let manifest = serde_json::to_vec_pretty(&package.manifest)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_manifest"))?;

    let cursor = Cursor::new(Vec::new());
    let mut archive = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    archive
        .start_file("manifest.json", options)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    archive
        .write_all(&manifest)
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    for (path, content, _) in files {
        archive
            .start_file(path, options)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
        archive
            .write_all(&content)
            .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))?;
    }
    archive
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|_| ControlPlaneError::InvalidInput("mcp_bundle_archive"))
}

fn bundle_file_kind_rank(kind: domain::McpBundleFileKind) -> u8 {
    match kind {
        domain::McpBundleFileKind::Tool => 0,
        domain::McpBundleFileKind::Instance => 1,
        domain::McpBundleFileKind::Connection => 2,
    }
}

fn bundle_file_path(directory: &str, stable_id: &str) -> String {
    format!(
        "{directory}/{:x}.json",
        base16ct::HexDisplay(&Sha256::digest(stable_id.as_bytes()))
    )
}
