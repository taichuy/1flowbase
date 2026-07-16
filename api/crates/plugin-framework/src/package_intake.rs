use std::{
    fs,
    io::Cursor,
    path::{Component, Path, PathBuf},
};

use ed25519_dalek::{pkcs8::DecodePublicKey, Signature, Verifier, VerifyingKey};
use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tar::Archive;
use uuid::Uuid;
use zip::ZipArchive;

use crate::{
    error::PluginFrameworkError,
    manifest_v1::{parse_plugin_manifest, PluginManifestV1},
    provider_package::ProviderPackage,
};

#[derive(Debug, Clone)]
pub struct TrustedPublicKey {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_pem: String,
}

#[derive(Debug, Clone)]
pub struct PackageIntakePolicy {
    pub source_kind: String,
    pub trust_mode: String,
    pub expected_artifact_sha256: Option<String>,
    pub trusted_public_keys: Vec<TrustedPublicKey>,
    pub original_filename: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PackageIntakeResult {
    pub extracted_root: PathBuf,
    pub manifest: PluginManifestV1,
    pub source_kind: String,
    pub trust_level: String,
    pub signature_status: String,
    pub checksum: Option<String>,
    pub signature_algorithm: Option<String>,
    pub signing_key_id: Option<String>,
}

#[derive(Debug)]
struct ExtractedPackage {
    temp_dir: PathBuf,
    package_root: PathBuf,
}

#[derive(Debug, Clone)]
struct SignatureVerificationResult {
    status: String,
    artifact_sha256: Option<String>,
    signature_algorithm: Option<String>,
    signing_key_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OfficialReleaseDocument {
    #[allow(dead_code)]
    schema_version: u32,
    #[allow(dead_code)]
    plugin_id: String,
    #[allow(dead_code)]
    provider_code: String,
    #[allow(dead_code)]
    version: String,
    #[allow(dead_code)]
    contract_version: String,
    #[allow(dead_code)]
    artifact_sha256: String,
    payload_sha256: String,
    signature_algorithm: String,
    signing_key_id: String,
    #[allow(dead_code)]
    issued_at: String,
}

#[derive(Debug, Clone, Copy)]
enum ArchiveFormat {
    TarGz,
    Zip,
}

pub async fn intake_package_bytes(
    package_bytes: &[u8],
    policy: &PackageIntakePolicy,
) -> Result<PackageIntakeResult, PluginFrameworkError> {
    let extracted = safe_unpack_to_temp_dir(package_bytes, policy.original_filename.as_deref())?;
    let manifest_path = extracted.package_root.join("manifest.yaml");
    let manifest_raw = match fs::read_to_string(&manifest_path) {
        Ok(raw) => raw,
        Err(error) => {
            let _ = fs::remove_dir_all(&extracted.temp_dir);
            return Err(PluginFrameworkError::io(
                Some(&manifest_path),
                error.to_string(),
            ));
        }
    };
    let manifest = match parse_plugin_manifest(&manifest_raw) {
        Ok(manifest) => manifest,
        Err(error) => {
            let _ = fs::remove_dir_all(&extracted.temp_dir);
            return Err(error);
        }
    };
    match ProviderPackage::load_from_dir(&extracted.package_root) {
        Ok(_) => {}
        Err(error)
            if matches!(
                manifest.contract_version.as_str(),
                "1flowbase.provider/v1" | "1flowbase.provider/v2"
            ) =>
        {
            let _ = fs::remove_dir_all(&extracted.temp_dir);
            return Err(error);
        }
        Err(_) => {}
    }
    let signature = match verify_official_release_signature(
        &extracted.package_root,
        &manifest,
        package_bytes,
        policy,
    ) {
        Ok(signature) => signature,
        Err(error) => {
            let _ = fs::remove_dir_all(&extracted.temp_dir);
            return Err(error);
        }
    };
    if let Err(error) = reject_signature_required_failure(policy, &signature) {
        let _ = fs::remove_dir_all(&extracted.temp_dir);
        return Err(error);
    }

    Ok(PackageIntakeResult {
        extracted_root: extracted.package_root,
        manifest,
        source_kind: policy.source_kind.clone(),
        trust_level: derive_trust_level(policy, &signature),
        signature_status: signature.status,
        checksum: signature.artifact_sha256,
        signature_algorithm: signature.signature_algorithm,
        signing_key_id: signature.signing_key_id,
    })
}

fn safe_unpack_to_temp_dir(
    package_bytes: &[u8],
    original_filename: Option<&str>,
) -> Result<ExtractedPackage, PluginFrameworkError> {
    let temp_dir = std::env::temp_dir().join(format!("plugin-package-intake-{}", Uuid::now_v7()));
    fs::create_dir_all(&temp_dir)
        .map_err(|error| PluginFrameworkError::io(Some(&temp_dir), error.to_string()))?;

    let unpack_result = (|| -> Result<PathBuf, PluginFrameworkError> {
        match detect_archive_format(package_bytes, original_filename)? {
            ArchiveFormat::TarGz => unpack_tar_gz(package_bytes, &temp_dir)?,
            ArchiveFormat::Zip => unpack_zip(package_bytes, &temp_dir)?,
        }
        resolve_package_root(&temp_dir)
    })();

    match unpack_result {
        Ok(package_root) => Ok(ExtractedPackage {
            temp_dir,
            package_root,
        }),
        Err(error) => {
            let _ = fs::remove_dir_all(&temp_dir);
            Err(error)
        }
    }
}

fn detect_archive_format(
    package_bytes: &[u8],
    original_filename: Option<&str>,
) -> Result<ArchiveFormat, PluginFrameworkError> {
    let lower_name = original_filename
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if package_bytes.starts_with(b"PK\x03\x04") || lower_name.ends_with(".zip") {
        return Ok(ArchiveFormat::Zip);
    }
    if package_bytes.starts_with(&[0x1f, 0x8b])
        || lower_name.ends_with(".1flowbasepkg")
        || lower_name.ends_with(".tar.gz")
        || lower_name.ends_with(".tgz")
    {
        return Ok(ArchiveFormat::TarGz);
    }

    Err(PluginFrameworkError::invalid_provider_package(
        "unsupported package archive format",
    ))
}

fn unpack_tar_gz(package_bytes: &[u8], destination: &Path) -> Result<(), PluginFrameworkError> {
    let decoder = GzDecoder::new(Cursor::new(package_bytes));
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;

    for entry in entries {
        let mut entry = entry
            .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;
        let path = entry
            .path()
            .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?
            .to_path_buf();
        validate_relative_path(&path)?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "package archive cannot contain links",
            ));
        }
        entry
            .unpack_in(destination)
            .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;
    }

    Ok(())
}

fn unpack_zip(package_bytes: &[u8], destination: &Path) -> Result<(), PluginFrameworkError> {
    let cursor = Cursor::new(package_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;
        let path = file.enclosed_name().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package("zip archive contains an unsafe path")
        })?;
        validate_relative_path(&path)?;
        let target = destination.join(path);
        if file.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| PluginFrameworkError::io(Some(&target), error.to_string()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| PluginFrameworkError::io(Some(parent), error.to_string()))?;
        }
        let mut output = fs::File::create(&target)
            .map_err(|error| PluginFrameworkError::io(Some(&target), error.to_string()))?;
        std::io::copy(&mut file, &mut output)
            .map_err(|error| PluginFrameworkError::io(Some(&target), error.to_string()))?;
    }

    Ok(())
}

fn resolve_package_root(temp_dir: &Path) -> Result<PathBuf, PluginFrameworkError> {
    if temp_dir.join("manifest.yaml").is_file() {
        return Ok(temp_dir.to_path_buf());
    }

    let mut children = fs::read_dir(temp_dir)
        .map_err(|error| PluginFrameworkError::io(Some(temp_dir), error.to_string()))?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| PluginFrameworkError::io(Some(temp_dir), error.to_string()))?;
    children.sort();

    let child_dirs = children
        .into_iter()
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    if child_dirs.len() == 1 && child_dirs[0].join("manifest.yaml").is_file() {
        return Ok(child_dirs[0].clone());
    }

    Err(PluginFrameworkError::invalid_provider_package(
        "package manifest.yaml not found after extraction",
    ))
}

fn validate_relative_path(path: &Path) -> Result<(), PluginFrameworkError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "package archive contains an unsafe path",
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "package archive contains an unsafe path",
        ));
    }
    Ok(())
}

fn verify_official_release_signature(
    extracted_root: &Path,
    manifest: &PluginManifestV1,
    package_bytes: &[u8],
    policy: &PackageIntakePolicy,
) -> Result<SignatureVerificationResult, PluginFrameworkError> {
    let actual_artifact_sha256 = sha256_hex(package_bytes);
    if let Some(expected) = policy.expected_artifact_sha256.as_deref() {
        let expected_normalized = normalize_sha256(expected)?;
        if actual_artifact_sha256 != expected_normalized {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "package checksum mismatch: expected sha256:{expected_normalized}, got sha256:{actual_artifact_sha256}"
            )));
        }
    }

    let release_path = extracted_root.join("_meta/official-release.json");
    let signature_path = extracted_root.join("_meta/official-release.sig");
    if !release_path.is_file() && !signature_path.is_file() {
        return Ok(SignatureVerificationResult {
            status: "unsigned".to_string(),
            artifact_sha256: Some(format!("sha256:{actual_artifact_sha256}")),
            signature_algorithm: None,
            signing_key_id: None,
        });
    }
    if !release_path.is_file() {
        return Ok(SignatureVerificationResult {
            status: "missing_manifest".to_string(),
            artifact_sha256: Some(format!("sha256:{actual_artifact_sha256}")),
            signature_algorithm: None,
            signing_key_id: None,
        });
    }
    if !signature_path.is_file() {
        return Ok(SignatureVerificationResult {
            status: "unsigned".to_string(),
            artifact_sha256: Some(format!("sha256:{actual_artifact_sha256}")),
            signature_algorithm: None,
            signing_key_id: None,
        });
    }

    let artifact_sha256 = Some(format!("sha256:{actual_artifact_sha256}"));
    let release_bytes = fs::read(&release_path)
        .map_err(|error| PluginFrameworkError::io(Some(&release_path), error.to_string()))?;
    let release: OfficialReleaseDocument = match serde_json::from_slice(&release_bytes) {
        Ok(release) => release,
        Err(_) => {
            return Ok(SignatureVerificationResult {
                status: "malformed_signature".to_string(),
                artifact_sha256,
                signature_algorithm: None,
                signing_key_id: None,
            })
        }
    };
    let actual_payload_sha256 = payload_sha256(extracted_root)?;
    if normalize_sha256(&release.payload_sha256)? != actual_payload_sha256 {
        return Ok(SignatureVerificationResult {
            status: "invalid".to_string(),
            artifact_sha256,
            signature_algorithm: Some(release.signature_algorithm),
            signing_key_id: Some(release.signing_key_id),
        });
    }
    validate_release_identity(&release, manifest)?;

    let trusted_key = match policy
        .trusted_public_keys
        .iter()
        .find(|candidate| candidate.key_id == release.signing_key_id)
    {
        Some(key) => key,
        None => {
            return Ok(SignatureVerificationResult {
                status: "unknown_key".to_string(),
                artifact_sha256,
                signature_algorithm: Some(release.signature_algorithm),
                signing_key_id: Some(release.signing_key_id),
            })
        }
    };
    if trusted_key.algorithm != release.signature_algorithm {
        return Ok(SignatureVerificationResult {
            status: "unknown_key".to_string(),
            artifact_sha256,
            signature_algorithm: Some(release.signature_algorithm),
            signing_key_id: Some(release.signing_key_id),
        });
    }

    let verifying_key =
        VerifyingKey::from_public_key_pem(&trusted_key.public_key_pem).map_err(|error| {
            PluginFrameworkError::invalid_provider_package(format!(
                "trusted public key {} is invalid: {error}",
                trusted_key.key_id
            ))
        })?;
    let signature_bytes = fs::read(&signature_path)
        .map_err(|error| PluginFrameworkError::io(Some(&signature_path), error.to_string()))?;
    let signature = match parse_signature(&signature_bytes) {
        Ok(signature) => signature,
        Err(_) => {
            return Ok(SignatureVerificationResult {
                status: "malformed_signature".to_string(),
                artifact_sha256,
                signature_algorithm: Some(release.signature_algorithm),
                signing_key_id: Some(release.signing_key_id),
            })
        }
    };
    let status = if verifying_key.verify(&release_bytes, &signature).is_ok() {
        "verified"
    } else {
        "invalid"
    };

    Ok(SignatureVerificationResult {
        status: status.to_string(),
        artifact_sha256,
        signature_algorithm: Some(release.signature_algorithm),
        signing_key_id: Some(release.signing_key_id),
    })
}

fn validate_release_identity(
    release: &OfficialReleaseDocument,
    manifest: &PluginManifestV1,
) -> Result<(), PluginFrameworkError> {
    if release.plugin_id != manifest.plugin_id
        || release.provider_code != plugin_code_from_plugin_id(manifest)?
        || release.version != manifest.version
        || release.contract_version != manifest.contract_version
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "official release metadata must match package manifest identity",
        ));
    }

    Ok(())
}

fn plugin_code_from_plugin_id(manifest: &PluginManifestV1) -> Result<&str, PluginFrameworkError> {
    manifest.plugin_code()
}

fn parse_signature(bytes: &[u8]) -> Result<Signature, PluginFrameworkError> {
    let signature_bytes = bytes.try_into().map_err(|_| {
        PluginFrameworkError::invalid_provider_package("official signature must be 64 bytes")
    })?;
    Ok(Signature::from_bytes(signature_bytes))
}

fn payload_sha256(root: &Path) -> Result<String, PluginFrameworkError> {
    let mut files = Vec::new();
    collect_payload_files(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (relative_path, content) in files {
        hasher.update(relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(content);
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_payload_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), PluginFrameworkError> {
    let mut children = fs::read_dir(current)
        .map_err(|error| PluginFrameworkError::io(Some(current), error.to_string()))?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| PluginFrameworkError::io(Some(current), error.to_string()))?;
    children.sort();

    for path in children {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with("_meta/") {
            continue;
        }
        if path.is_dir() {
            collect_payload_files(root, &path, files)?;
            continue;
        }
        let content = fs::read(&path)
            .map_err(|error| PluginFrameworkError::io(Some(&path), error.to_string()))?;
        files.push((relative, content));
    }

    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn normalize_sha256(value: &str) -> Result<String, PluginFrameworkError> {
    let normalized = value
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or(value.trim())
        .to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() != Sha256::output_size() * 2
        || !normalized.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "sha256 checksum must be a non-empty hexadecimal string",
        ));
    }
    Ok(normalized)
}

fn reject_signature_required_failure(
    policy: &PackageIntakePolicy,
    signature: &SignatureVerificationResult,
) -> Result<(), PluginFrameworkError> {
    let signature_failed = matches!(
        signature.status.as_str(),
        "unsigned" | "invalid" | "unknown_key" | "missing_manifest" | "malformed_signature"
    );
    let registry_source = matches!(
        policy.source_kind.as_str(),
        "official_registry" | "mirror_registry"
    );
    if registry_source && policy.trust_mode == "signature_required" && signature_failed {
        return Err(PluginFrameworkError::invalid_provider_package(
            "official or mirror package requires a valid official signature",
        ));
    }
    Ok(())
}

fn derive_trust_level(
    policy: &PackageIntakePolicy,
    signature: &SignatureVerificationResult,
) -> String {
    match signature.status.as_str() {
        "verified" => "verified_official".to_string(),
        _ if policy.source_kind == "uploaded" || policy.trust_mode == "allow_unsigned" => {
            "unverified".to_string()
        }
        _ => "checksum_only".to_string(),
    }
}
