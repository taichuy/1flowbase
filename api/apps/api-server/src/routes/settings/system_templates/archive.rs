//! Transport codec for split portable definition packages; installation remains the service's job.
use anyhow::{ensure, Context, Result};
use control_plane::portable_template::PortableTemplatePackage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    path::Path,
};

pub(crate) const SCHEMA: &str = "1flowbase.application-template-archive/v1";
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const MAX_DEPTH: usize = 128;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub schema_version: String,
    pub package: Value,
    pub files: Vec<ManifestFile>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestFile {
    pub path: String,
    pub sha256: String,
}
pub(crate) fn checksum(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn safe_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && !path.contains(['\\', '\0', ':']) && !path.starts_with('/'),
        "application_template_archive_path"
    );
    ensure!(
        path.split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".."),
        "application_template_archive_path"
    );
    ensure!(
        path.split('/').count() <= MAX_DEPTH,
        "application_template_archive_depth"
    );
    Ok(())
}
pub(crate) fn read_manifest(bytes: &[u8]) -> Result<Manifest> {
    ensure!(
        bytes.len() <= MAX_BYTES,
        "application_template_archive_size"
    );
    let manifest: Manifest = serde_json::from_slice(bytes)?;
    ensure!(
        manifest.schema_version == SCHEMA,
        "application_template_archive_schema"
    );
    ensure!(
        manifest.files.len() <= MAX_FILES,
        "application_template_archive_file_count"
    );
    let mut previous: Option<&str> = None;
    for file in &manifest.files {
        safe_path(&file.path)?;
        ensure!(
            file.path != "manifest.json" && previous.is_none_or(|p| p < file.path.as_str()),
            "application_template_archive_file_order"
        );
        previous = Some(&file.path);
    }
    Ok(manifest)
}
fn materialize(
    manifest: Manifest,
    files: &BTreeMap<String, Vec<u8>>,
) -> Result<PortableTemplatePackage> {
    ensure!(
        files.len() == manifest.files.len(),
        "application_template_archive_extra_file"
    );
    let mut total = 0usize;
    for file in &manifest.files {
        let bytes = files
            .get(&file.path)
            .context("application_template_archive_missing_file")?;
        total = total
            .checked_add(bytes.len())
            .context("application_template_archive_size")?;
        ensure!(
            total <= MAX_BYTES && checksum(bytes) == file.sha256,
            "application_template_archive_checksum"
        );
    }
    fn resolve(
        value: Value,
        files: &BTreeMap<String, Vec<u8>>,
        active: &mut BTreeSet<String>,
        used: &mut BTreeSet<String>,
        depth: usize,
        expanded: &mut usize,
    ) -> Result<Value> {
        ensure!(depth <= MAX_DEPTH, "application_template_archive_depth");
        match value {
            Value::Object(mut object) if object.len() == 1 && object.contains_key("$file") => {
                ensure!(object.len() == 1, "application_template_archive_reference");
                let path = object
                    .remove("$file")
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .context("application_template_archive_reference")?;
                ensure!(
                    active.insert(path.clone()),
                    "application_template_archive_cycle"
                );
                let bytes = files
                    .get(&path)
                    .context("application_template_archive_undeclared_reference")?;
                *expanded = expanded
                    .checked_add(bytes.len())
                    .context("application_template_archive_size")?;
                ensure!(
                    *expanded <= MAX_BYTES,
                    "application_template_archive_expanded_size"
                );
                used.insert(path.clone());
                let result = resolve(
                    serde_json::from_slice(bytes)?,
                    files,
                    active,
                    used,
                    depth + 1,
                    expanded,
                )?;
                active.remove(&path);
                Ok(result)
            }
            Value::Object(object) => Ok(Value::Object(
                object
                    .into_iter()
                    .map(|(k, v)| Ok((k, resolve(v, files, active, used, depth + 1, expanded)?)))
                    .collect::<Result<_>>()?,
            )),
            Value::Array(array) => Ok(Value::Array(
                array
                    .into_iter()
                    .map(|v| resolve(v, files, active, used, depth + 1, expanded))
                    .collect::<Result<_>>()?,
            )),
            scalar => Ok(scalar),
        }
    }
    let mut used = BTreeSet::new();
    let value = resolve(
        manifest.package,
        files,
        &mut BTreeSet::new(),
        &mut used,
        0,
        &mut 0,
    )?;
    ensure!(
        used.len() == files.len(),
        "application_template_archive_unreferenced_file"
    );
    let package: PortableTemplatePackage = serde_json::from_value(value)?;
    ensure!(
        package.schema_version
            == control_plane_contracts::portable_template::PORTABLE_TEMPLATE_SCHEMA_VERSION,
        "application_template_schema"
    );
    Ok(package)
}
pub(crate) fn decode(bytes: &[u8]) -> Result<PortableTemplatePackage> {
    ensure!(
        bytes.len() <= MAX_BYTES,
        "application_template_archive_size"
    );
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    ensure!(
        archive.len() <= MAX_FILES + 1024,
        "application_template_archive_file_count"
    );
    let mut files = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut total = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().trim_end_matches('/').to_owned();
        safe_path(&name)?;
        ensure!(
            names.insert(name.clone()),
            "application_template_archive_duplicate"
        );
        let mode = entry.unix_mode().unwrap_or(0) & 0o170000;
        ensure!(
            mode == 0 || mode == 0o100000 || (entry.is_dir() && mode == 0o040000),
            "application_template_archive_nonregular"
        );
        if entry.is_dir() {
            continue;
        }
        ensure!(
            matches!(
                entry.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ),
            "application_template_archive_compression"
        );
        ensure!(
            entry.size() <= MAX_BYTES as u64,
            "application_template_archive_size"
        );
        let mut data = Vec::new();
        entry
            .by_ref()
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut data)?;
        total = total
            .checked_add(data.len())
            .context("application_template_archive_size")?;
        ensure!(total <= MAX_BYTES, "application_template_archive_size");
        files.insert(name, data);
    }
    let manifest = read_manifest(
        &files
            .remove("manifest.json")
            .context("application_template_archive_manifest")?,
    )?;
    materialize(manifest, &files)
}
pub(crate) fn load_directory(root: &Path) -> Result<PortableTemplatePackage> {
    let manifest = read_manifest(&std::fs::read(root.join("manifest.json"))?)?;
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for file in &manifest.files {
        let mut path = root.to_path_buf();
        for part in file.path.split('/') {
            path.push(part);
            ensure!(
                !std::fs::symlink_metadata(&path)?.file_type().is_symlink(),
                "application_template_archive_symlink"
            );
        }
        let metadata = std::fs::metadata(&path)?;
        total = total
            .checked_add(metadata.len())
            .context("application_template_archive_size")?;
        ensure!(
            metadata.is_file() && total <= MAX_BYTES as u64,
            "application_template_archive_size"
        );
        files.insert(file.path.clone(), std::fs::read(path)?);
    }
    materialize(manifest, &files)
}
fn put(files: &mut BTreeMap<String, Vec<u8>>, path: String, value: Value) -> Result<Value> {
    safe_path(&path)?;
    let mut bytes = serde_json::to_vec_pretty(&value)?;
    bytes.push(b'\n');
    ensure!(
        files.insert(path.clone(), bytes).is_none(),
        "application_template_archive_duplicate"
    );
    Ok(json!({"$file":path}))
}
fn identity(value: &Value, field: &str) -> Result<String> {
    let id = value
        .get(field)
        .and_then(Value::as_str)
        .context("application_template_archive_identity")?;
    // Hash names for identifiers that cannot be represented as one portable path segment.
    Ok(
        if id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
            && !id.is_empty()
        {
            id.to_owned()
        } else {
            format!("{:x}", Sha256::digest(id.as_bytes()))
        },
    )
}
pub(crate) fn source_files(package: &PortableTemplatePackage) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut skeleton = serde_json::to_value(package)?;
    let mut files = BTreeMap::new();
    for category in ["pages", "applications", "data_models"] {
        let resources = skeleton[category]
            .as_array_mut()
            .context("application_template_archive_resources")?;
        for resource in resources {
            let id = identity(resource, "id")?;
            let prefix = format!(
                "{}/{id}",
                if category == "data_models" {
                    "data-models"
                } else {
                    category
                }
            );
            if category == "applications" {
                resource["flow_document"] = put(
                    &mut files,
                    format!("{prefix}/flow-document.json"),
                    resource["flow_document"].take(),
                )?;
                if resource["published"].is_object() {
                    resource["published"]["flow_document"] = put(
                        &mut files,
                        format!("{prefix}/published-flow-document.json"),
                        resource["published"]["flow_document"].take(),
                    )?;
                }
            }
            if category == "pages" {
                if let Some(tabs) = resource["tabs"].as_array_mut() {
                    for tab in tabs {
                        let tab_id = identity(tab, "id")?;
                        tab["document_payload"] = put(
                            &mut files,
                            format!("{prefix}/tabs/{tab_id}/document.json"),
                            tab["document_payload"].take(),
                        )?;
                        *tab = put(
                            &mut files,
                            format!("{prefix}/tabs/{tab_id}.json"),
                            tab.take(),
                        )?;
                    }
                }
            }
            let path = match category {
                "pages" => format!("{prefix}/page.json"),
                "applications" => format!("{prefix}/application.json"),
                _ => format!("{prefix}.json"),
            };
            *resource = put(&mut files, path, resource.take())?;
        }
    }
    skeleton["plugins"] = put(
        &mut files,
        "plugins/dependencies.json".into(),
        skeleton["plugins"].take(),
    )?;
    if let Some(bundle) = skeleton.get_mut("mcp_bundle").filter(|v| v.is_object()) {
        for (kind, field) in [
            ("tools", "tool_id"),
            ("instances", "instance_id"),
            ("connections", "connection_id"),
        ] {
            if let Some(resources) = bundle[kind].as_array_mut() {
                for resource in resources {
                    let id = identity(resource, field)?;
                    let path = if kind == "tools" {
                        let hash = format!(
                            "{:x}",
                            Sha256::digest(resource[field].as_str().unwrap().as_bytes())
                        );
                        format!("mcp/tools/{}/{id}.json", &hash[..2])
                    } else {
                        format!("mcp/{kind}/{id}.json")
                    };
                    *resource = put(&mut files, path, resource.take())?;
                }
            }
        }
        bundle["manifest"] = put(
            &mut files,
            "mcp/manifest.json".into(),
            bundle["manifest"].take(),
        )?;
    }
    let manifest = Manifest {
        schema_version: SCHEMA.into(),
        package: skeleton,
        files: files
            .iter()
            .map(|(path, bytes)| ManifestFile {
                path: path.clone(),
                sha256: checksum(bytes),
            })
            .collect(),
    };
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec_pretty(&manifest)?,
    );
    Ok(files)
}
pub(crate) fn encode(package: &PortableTemplatePackage) -> Result<Vec<u8>> {
    let files = source_files(package)?;
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (path, bytes) in files {
        writer.start_file(path, options)?;
        writer.write_all(&bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}
