use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::pkcs8::{spki::der::pem::LineEnding, DecodePublicKey, EncodePublicKey};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use flate2::{write::GzEncoder, Compression};
use plugin_framework::{
    intake_package_bytes, verify_trusted_ed25519_artifact, PackageIntakePolicy, TrustedPublicKey,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tar::Builder;
use uuid::Uuid;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

#[derive(Clone, Copy)]
enum ArchiveFormat {
    TarGz,
    Zip,
}

#[test]
fn trusted_artifact_verifier_rejects_checksum_signature_and_unknown_key_tampering() {
    let bytes = br#"{"template_id":"fixture"}"#;
    let signing_key = SigningKey::from_bytes(&[23; 32]);
    let trusted = TrustedPublicKey {
        key_id: "fixture-key".into(),
        algorithm: "ed25519".into(),
        public_key_pem: signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    };
    let checksum = format!("sha256:{:x}", Sha256::digest(bytes));
    let signature = STANDARD.encode(signing_key.sign(bytes).to_bytes());

    verify_trusted_ed25519_artifact(
        bytes,
        &checksum,
        "ed25519",
        "fixture-key",
        &signature,
        std::slice::from_ref(&trusted),
    )
    .unwrap();
    assert!(verify_trusted_ed25519_artifact(
        b"tampered",
        &checksum,
        "ed25519",
        "fixture-key",
        &signature,
        std::slice::from_ref(&trusted),
    )
    .is_err());
    assert!(verify_trusted_ed25519_artifact(
        bytes,
        &checksum,
        "ed25519",
        "fixture-key",
        &STANDARD.encode([0; 64]),
        std::slice::from_ref(&trusted),
    )
    .is_err());
    assert!(verify_trusted_ed25519_artifact(
        bytes,
        &checksum,
        "ed25519",
        "unknown-key",
        &signature,
        &[trusted],
    )
    .is_err());
}

struct SignedFixtureInput<'a> {
    provider_code: &'a str,
    version: &'a str,
    source_kind: &'a str,
    trust_level: &'a str,
    include_signature: bool,
    tamper_signature: bool,
    archive_format: ArchiveFormat,
    release_plugin_id: Option<String>,
    release_provider_code: Option<String>,
    release_version: Option<String>,
}

struct SignedPackageFixture {
    package_bytes: Vec<u8>,
    artifact_sha256: String,
    public_key: TrustedPublicKey,
}

#[derive(Serialize)]
struct OfficialReleaseDocument<'a> {
    schema_version: u32,
    plugin_id: String,
    provider_code: &'a str,
    version: &'a str,
    contract_version: &'static str,
    artifact_sha256: &'a str,
    payload_sha256: String,
    signature_algorithm: &'static str,
    signing_key_id: &'static str,
    issued_at: &'static str,
}

struct TempFixtureDir {
    root: PathBuf,
}

impl TempFixtureDir {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("plugin-intake-tests-{}", Uuid::now_v7()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write_bytes(&self, relative_path: &str, content: &[u8]) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn write_str(&self, relative_path: &str, content: &str) {
        self.write_bytes(relative_path, content.as_bytes());
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn package_intake_verifies_signed_official_archive_and_exposes_manifest_snapshot() {
    let fixture = create_signed_package_fixture(SignedFixtureInput {
        provider_code: "openai_compatible",
        version: "0.2.0",
        source_kind: "official_registry",
        trust_level: "verified_official",
        include_signature: true,
        tamper_signature: false,
        archive_format: ArchiveFormat::TarGz,
        release_plugin_id: None,
        release_provider_code: None,
        release_version: None,
    });

    let result = intake_package_bytes(
        &fixture.package_bytes,
        &PackageIntakePolicy {
            source_kind: "official_registry".to_string(),
            trust_mode: "signature_required".to_string(),
            expected_artifact_sha256: Some(fixture.artifact_sha256.clone()),
            trusted_public_keys: vec![fixture.public_key.clone()],
            original_filename: Some("openai_compatible-0.2.0.1flowbasepkg".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(result.source_kind, "official_registry");
    assert_eq!(result.trust_level, "verified_official");
    assert_eq!(result.signature_status, "verified");
    assert_eq!(result.signature_algorithm.as_deref(), Some("ed25519"));
    assert_eq!(
        result.signing_key_id.as_deref(),
        Some("official-key-2026-04")
    );
    assert_eq!(result.manifest.plugin_id, "openai_compatible@0.2.0");
    assert_eq!(
        result.manifest.consumption_kind,
        plugin_framework::PluginConsumptionKind::RuntimeExtension
    );
    assert_eq!(
        result.manifest.runtime.entry,
        "bin/openai_compatible-provider"
    );
}

#[tokio::test]
async fn package_intake_rejects_unsigned_signature_required_mirror_archive() {
    let fixture = create_signed_package_fixture(SignedFixtureInput {
        provider_code: "openai_compatible",
        version: "0.2.0",
        source_kind: "mirror_registry",
        trust_level: "verified_official",
        include_signature: false,
        tamper_signature: false,
        archive_format: ArchiveFormat::TarGz,
        release_plugin_id: None,
        release_provider_code: None,
        release_version: None,
    });

    let error = intake_package_bytes(
        &fixture.package_bytes,
        &PackageIntakePolicy {
            source_kind: "mirror_registry".to_string(),
            trust_mode: "signature_required".to_string(),
            expected_artifact_sha256: Some(fixture.artifact_sha256.clone()),
            trusted_public_keys: vec![fixture.public_key.clone()],
            original_filename: Some("openai_compatible-0.2.0.1flowbasepkg".into()),
        },
    )
    .await
    .expect_err("unsigned mirror packages must be rejected");

    assert!(error
        .to_string()
        .contains("requires a valid official signature"));
}

#[tokio::test]
async fn package_intake_marks_unsigned_official_allow_unsigned_archive_as_unverified() {
    let fixture = create_signed_package_fixture(SignedFixtureInput {
        provider_code: "openai_compatible",
        version: "0.2.0",
        source_kind: "official_registry",
        trust_level: "verified_official",
        include_signature: false,
        tamper_signature: false,
        archive_format: ArchiveFormat::TarGz,
        release_plugin_id: None,
        release_provider_code: None,
        release_version: None,
    });

    let result = intake_package_bytes(
        &fixture.package_bytes,
        &PackageIntakePolicy {
            source_kind: "official_registry".to_string(),
            trust_mode: "allow_unsigned".to_string(),
            expected_artifact_sha256: Some(fixture.artifact_sha256.clone()),
            trusted_public_keys: vec![fixture.public_key.clone()],
            original_filename: Some("openai_compatible-0.2.0.1flowbasepkg".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(result.source_kind, "official_registry");
    assert_eq!(result.trust_level, "unverified");
    assert_eq!(result.signature_status, "unsigned");
    assert_eq!(result.checksum, Some(fixture.artifact_sha256));
}

#[tokio::test]
async fn package_intake_rejects_checksum_mismatch_when_signature_is_not_required() {
    let fixture = create_signed_package_fixture(SignedFixtureInput {
        provider_code: "openai_compatible",
        version: "0.2.0",
        source_kind: "official_registry",
        trust_level: "verified_official",
        include_signature: false,
        tamper_signature: false,
        archive_format: ArchiveFormat::TarGz,
        release_plugin_id: None,
        release_provider_code: None,
        release_version: None,
    });

    let error = intake_package_bytes(
        &fixture.package_bytes,
        &PackageIntakePolicy {
            source_kind: "official_registry".to_string(),
            trust_mode: "allow_unsigned".to_string(),
            expected_artifact_sha256: Some(format!("sha256:{}", "0".repeat(64))),
            trusted_public_keys: vec![fixture.public_key.clone()],
            original_filename: Some("openai_compatible-0.2.0.1flowbasepkg".into()),
        },
    )
    .await
    .expect_err("checksum mismatch must fail even when signature is optional");

    assert!(error.to_string().contains("package checksum mismatch"));
}

#[tokio::test]
async fn package_intake_marks_uploaded_unsigned_archive_as_unverified() {
    let fixture = create_signed_package_fixture(SignedFixtureInput {
        provider_code: "fixture_provider",
        version: "0.1.0",
        source_kind: "uploaded",
        trust_level: "unverified",
        include_signature: false,
        tamper_signature: false,
        archive_format: ArchiveFormat::Zip,
        release_plugin_id: None,
        release_provider_code: None,
        release_version: None,
    });

    let result = intake_package_bytes(
        &fixture.package_bytes,
        &PackageIntakePolicy {
            source_kind: "uploaded".to_string(),
            trust_mode: "allow_unsigned".to_string(),
            expected_artifact_sha256: None,
            trusted_public_keys: vec![fixture.public_key.clone()],
            original_filename: Some("fixture_provider-0.1.0.zip".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(result.source_kind, "uploaded");
    assert_eq!(result.trust_level, "unverified");
    assert_eq!(result.signature_status, "unsigned");
    assert_eq!(result.manifest.plugin_id, "fixture_provider@0.1.0");
}

#[tokio::test]
async fn ac_002_package_intake_rejects_legacy_provider_contract() {
    let fixture_dir = TempFixtureDir::new();
    write_provider_fixture(
        &fixture_dir,
        "fixture_provider",
        "0.1.0",
        "uploaded",
        "unverified",
    );
    let manifest_path = fixture_dir.path().join("manifest.yaml");
    let legacy_manifest = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("1flowbase.provider/v2", "1flowbase.provider/v1");
    fixture_dir.write_str("manifest.yaml", &legacy_manifest);

    let package_bytes = pack_zip(fixture_dir.path());
    let error = intake_package_bytes(
        &package_bytes,
        &PackageIntakePolicy {
            source_kind: "uploaded".to_string(),
            trust_mode: "allow_unsigned".to_string(),
            expected_artifact_sha256: None,
            trusted_public_keys: Vec::new(),
            original_filename: Some("fixture_provider-0.1.0.zip".into()),
        },
    )
    .await
    .expect_err("legacy provider contract must fail package intake");

    assert!(error
        .to_string()
        .contains("contract_version must be 1flowbase.provider/v2"));
}

#[tokio::test]
async fn package_intake_rejects_malformed_provider_parameter_form() {
    let fixture_dir = TempFixtureDir::new();
    write_provider_fixture(
        &fixture_dir,
        "fixture_provider",
        "0.1.0",
        "uploaded",
        "unverified",
    );
    fixture_dir.write_str(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: fixture_provider
protocol: openai_compatible
model_discovery: hybrid
parameter_form: definitely-not-a-schema
config_schema:
  - key: api_key
    type: string
    required: true
"#,
    );

    let package_bytes = pack_zip(fixture_dir.path());
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let error = intake_package_bytes(
        &package_bytes,
        &PackageIntakePolicy {
            source_kind: "uploaded".to_string(),
            trust_mode: "allow_unsigned".to_string(),
            expected_artifact_sha256: None,
            trusted_public_keys: vec![TrustedPublicKey {
                key_id: "official-key-2026-04".to_string(),
                algorithm: "ed25519".to_string(),
                public_key_pem: signing_key
                    .verifying_key()
                    .to_public_key_pem(LineEnding::LF)
                    .unwrap(),
            }],
            original_filename: Some("fixture_provider-0.1.0.zip".into()),
        },
    )
    .await
    .expect_err("malformed provider parameter form must fail intake");

    assert!(error.to_string().contains("parameter_form"));
}

#[tokio::test]
async fn package_intake_rejects_signed_archive_with_release_identity_mismatch() {
    let fixture = create_signed_package_fixture(SignedFixtureInput {
        provider_code: "openai_compatible",
        version: "0.2.0",
        source_kind: "official_registry",
        trust_level: "verified_official",
        include_signature: true,
        tamper_signature: false,
        archive_format: ArchiveFormat::TarGz,
        release_plugin_id: Some("different_provider@0.2.0".to_string()),
        release_provider_code: Some("different_provider".to_string()),
        release_version: None,
    });

    let error = intake_package_bytes(
        &fixture.package_bytes,
        &PackageIntakePolicy {
            source_kind: "official_registry".to_string(),
            trust_mode: "signature_required".to_string(),
            expected_artifact_sha256: Some(fixture.artifact_sha256.clone()),
            trusted_public_keys: vec![fixture.public_key.clone()],
            original_filename: Some("openai_compatible-0.2.0.1flowbasepkg".into()),
        },
    )
    .await
    .expect_err("mismatched release identity must be rejected");

    assert!(error
        .to_string()
        .contains("official release metadata must match package manifest identity"));
}

#[tokio::test]
async fn package_intake_verifies_signed_archive_with_matching_key_from_keyring() {
    let fixture = create_signed_package_fixture(SignedFixtureInput {
        provider_code: "openai_compatible",
        version: "0.2.0",
        source_kind: "official_registry",
        trust_level: "verified_official",
        include_signature: true,
        tamper_signature: false,
        archive_format: ArchiveFormat::TarGz,
        release_plugin_id: None,
        release_provider_code: None,
        release_version: None,
    });
    let unrelated_signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let unrelated_public_key = TrustedPublicKey {
        key_id: "unrelated-key".to_string(),
        algorithm: "ed25519".to_string(),
        public_key_pem: unrelated_signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    };

    let result = intake_package_bytes(
        &fixture.package_bytes,
        &PackageIntakePolicy {
            source_kind: "official_registry".to_string(),
            trust_mode: "signature_required".to_string(),
            expected_artifact_sha256: Some(fixture.artifact_sha256.clone()),
            trusted_public_keys: vec![unrelated_public_key, fixture.public_key.clone()],
            original_filename: Some("openai_compatible-0.2.0.1flowbasepkg".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(result.signature_status, "verified");
    assert_eq!(result.trust_level, "verified_official");
    assert_eq!(
        result.signing_key_id.as_deref(),
        Some("official-key-2026-04")
    );
}

#[test]
fn node_generated_ed25519_signature_verifies_with_rust() {
    let fixture_dir = TempFixtureDir::new();
    let message_path = fixture_dir.path().join("message.json");
    let signature_path = fixture_dir.path().join("message.sig");
    let private_key_path = fixture_dir.path().join("node-signing-key.pem");
    let public_key_path = fixture_dir.path().join("node-signing-public.pem");
    let message = br#"{"schema_version":1,"message":"node-to-rust"}"#;

    fs::write(&message_path, message).unwrap();
    generate_node_ed25519_keypair(&private_key_path, &public_key_path);
    sign_file_with_node(&private_key_path, &message_path, &signature_path);

    let public_key = fs::read_to_string(&public_key_path).unwrap();
    let verifying_key = VerifyingKey::from_public_key_pem(&public_key).unwrap();
    let signature_bytes = fs::read(&signature_path).unwrap();
    let signature = Signature::from_slice(&signature_bytes).unwrap();

    verifying_key.verify(message, &signature).unwrap();
}

#[tokio::test]
async fn package_intake_accepts_node_packaged_signed_archive() {
    let fixture_dir = TempFixtureDir::new();
    let output_dir = TempFixtureDir::new();
    let private_key_path = output_dir.path().join("node-signing-key.pem");
    let public_key_path = output_dir.path().join("node-signing-public.pem");
    let runtime_binary_path = output_dir.path().join("openai_compatible-provider");

    write_provider_fixture(
        &fixture_dir,
        "openai_compatible",
        "0.2.0",
        "official_registry",
        "verified_official",
    );
    write_fake_runtime_binary(&runtime_binary_path);
    generate_node_ed25519_keypair(&private_key_path, &public_key_path);

    let package = package_plugin_with_node(
        fixture_dir.path(),
        output_dir.path(),
        &runtime_binary_path,
        &private_key_path,
    );
    let package_bytes = fs::read(&package.package_file).unwrap();
    let public_key_pem = fs::read_to_string(&public_key_path).unwrap();

    let result = intake_package_bytes(
        &package_bytes,
        &PackageIntakePolicy {
            source_kind: "official_registry".to_string(),
            trust_mode: "signature_required".to_string(),
            expected_artifact_sha256: Some(format!("sha256:{}", package.checksum)),
            trusted_public_keys: vec![TrustedPublicKey {
                key_id: "official-key-2026-04".to_string(),
                algorithm: "ed25519".to_string(),
                public_key_pem,
            }],
            original_filename: Some(package.package_name),
        },
    )
    .await
    .expect("node-packaged signed archive should be accepted by rust intake");

    assert_eq!(result.signature_status, "verified");
    assert_eq!(result.trust_level, "verified_official");
    assert_eq!(result.manifest.plugin_id, "openai_compatible@0.2.0");
}

fn create_signed_package_fixture(input: SignedFixtureInput<'_>) -> SignedPackageFixture {
    let fixture_dir = TempFixtureDir::new();
    write_provider_fixture(
        &fixture_dir,
        input.provider_code,
        input.version,
        input.source_kind,
        input.trust_level,
    );

    let payload_sha256 = payload_sha256(fixture_dir.path());
    let signing_key = SigningKey::from_bytes(&[7u8; 32]);
    let public_key = TrustedPublicKey {
        key_id: "official-key-2026-04".to_string(),
        algorithm: "ed25519".to_string(),
        public_key_pem: signing_key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    };

    if input.include_signature {
        let release = OfficialReleaseDocument {
            schema_version: 1,
            plugin_id: input
                .release_plugin_id
                .clone()
                .unwrap_or_else(|| format!("{}@{}", input.provider_code, input.version)),
            provider_code: input
                .release_provider_code
                .as_deref()
                .unwrap_or(input.provider_code),
            version: input.release_version.as_deref().unwrap_or(input.version),
            contract_version: "1flowbase.provider/v2",
            artifact_sha256: "sha256:fixture-artifact",
            payload_sha256,
            signature_algorithm: "ed25519",
            signing_key_id: "official-key-2026-04",
            issued_at: "2026-04-19T13:00:00Z",
        };
        let release_bytes = serde_json::to_vec(&release).unwrap();
        let mut signature = signing_key.sign(&release_bytes).to_bytes().to_vec();
        if input.tamper_signature {
            signature[0] ^= 0xFF;
        }
        fixture_dir.write_bytes("_meta/official-release.json", &release_bytes);
        fixture_dir.write_bytes("_meta/official-release.sig", &signature);
    }

    let package_bytes = match input.archive_format {
        ArchiveFormat::TarGz => pack_tar_gz(fixture_dir.path()),
        ArchiveFormat::Zip => pack_zip(fixture_dir.path()),
    };

    SignedPackageFixture {
        artifact_sha256: format!("sha256:{:x}", Sha256::digest(&package_bytes)),
        package_bytes,
        public_key,
    }
}

fn write_provider_fixture(
    dir: &TempFixtureDir,
    provider_code: &str,
    version: &str,
    source_kind: &str,
    trust_level: &str,
) {
    dir.write_str(
        "manifest.yaml",
        &format!(
            r#"manifest_version: 1
plugin_id: {provider_code}@{version}
version: {version}
publisher_namespace: taichuy
vendor: taichuy
display_name: {provider_code}
description: provider package
icon: icon.svg
source_kind: {source_kind}
trust_level: {trust_level}
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/{provider_code}-provider
  limits:
    timeout_ms: 30000
    memory_bytes: 268435456
node_contributions: []
"#
        ),
    );
    dir.write_str(
        &format!("provider/{provider_code}.yaml"),
        &format!(
            r#"provider_code: {provider_code}
display_name: {provider_code}
protocol: openai_compatible
model_discovery: hybrid
config_schema:
  - key: api_key
    type: string
    required: true
"#
        ),
    );
    dir.write_str(
        &format!("bin/{provider_code}-provider"),
        "#!/usr/bin/env bash\nexit 0\n",
    );
    dir.write_str(
        "i18n/en_US.json",
        "{ \"plugin\": { \"label\": \"Acme\" } }\n",
    );
}

fn pack_tar_gz(root: &Path) -> Vec<u8> {
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(encoder);
    append_dir_to_tar(&mut builder, root, root);
    builder.finish().unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn pack_zip(root: &Path) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    append_dir_to_zip(&mut writer, root, root, options);
    writer.finish().unwrap().into_inner()
}

fn payload_sha256(root: &Path) -> String {
    let mut files = Vec::new();
    collect_payload_files(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (relative_path, content) in files {
        hasher.update(relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(content);
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn collect_payload_files(root: &Path, current: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    let mut children = fs::read_dir(current)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());

    for entry in children {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with("_meta/") {
            continue;
        }
        if path.is_dir() {
            collect_payload_files(root, &path, files);
            continue;
        }
        files.push((relative, fs::read(&path).unwrap()));
    }
}

fn append_dir_to_tar(builder: &mut Builder<GzEncoder<Vec<u8>>>, root: &Path, current: &Path) {
    let mut children = fs::read_dir(current)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());
    for entry in children {
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap();
        if path.is_dir() {
            builder.append_dir(relative, &path).unwrap();
            append_dir_to_tar(builder, root, &path);
            continue;
        }
        builder.append_path_with_name(&path, relative).unwrap();
    }
}

fn append_dir_to_zip(
    writer: &mut ZipWriter<Cursor<Vec<u8>>>,
    root: &Path,
    current: &Path,
    options: SimpleFileOptions,
) {
    let mut children = fs::read_dir(current)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| entry.path());
    for entry in children {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            writer
                .add_directory(format!("{relative}/"), options)
                .unwrap();
            append_dir_to_zip(writer, root, &path, options);
            continue;
        }
        writer.start_file(relative, options).unwrap();
        writer.write_all(&fs::read(&path).unwrap()).unwrap();
    }
}

#[derive(serde::Deserialize)]
struct NodePackageResult {
    #[serde(rename = "packageFile")]
    package_file: String,
    #[serde(rename = "packageName")]
    package_name: String,
    checksum: String,
}

struct NodePackagedArchive {
    package_file: PathBuf,
    package_name: String,
    checksum: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap()
}

fn generate_node_ed25519_keypair(private_key_path: &Path, public_key_path: &Path) {
    let script = r#"
const crypto = require('node:crypto');
const fs = require('node:fs');
const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');
fs.writeFileSync(process.env.PRIVATE_KEY_PATH, privateKey.export({ format: 'pem', type: 'pkcs8' }));
fs.writeFileSync(process.env.PUBLIC_KEY_PATH, publicKey.export({ format: 'pem', type: 'spki' }));
"#;
    let status = Command::new("node")
        .arg("-e")
        .arg(script)
        .env("PRIVATE_KEY_PATH", private_key_path)
        .env("PUBLIC_KEY_PATH", public_key_path)
        .status()
        .unwrap();

    assert!(status.success(), "node key generation failed");
}

fn sign_file_with_node(private_key_path: &Path, message_path: &Path, signature_path: &Path) {
    let script = r#"
const crypto = require('node:crypto');
const fs = require('node:fs');
const privateKey = crypto.createPrivateKey(fs.readFileSync(process.env.PRIVATE_KEY_PATH, 'utf8'));
const message = fs.readFileSync(process.env.MESSAGE_PATH);
const signature = crypto.sign(null, message, privateKey);
fs.writeFileSync(process.env.SIGNATURE_PATH, signature);
"#;
    let status = Command::new("node")
        .arg("-e")
        .arg(script)
        .env("PRIVATE_KEY_PATH", private_key_path)
        .env("MESSAGE_PATH", message_path)
        .env("SIGNATURE_PATH", signature_path)
        .status()
        .unwrap();

    assert!(status.success(), "node signing failed");
}

fn package_plugin_with_node(
    plugin_path: &Path,
    output_dir: &Path,
    runtime_binary_path: &Path,
    private_key_path: &Path,
) -> NodePackagedArchive {
    let script = r#"
const core = require(process.env.CORE_JS_PATH);
const result = core.createPluginPackage(process.env.PLUGIN_PATH, process.env.OUTPUT_DIR, {
  runtimeBinaryFile: process.env.RUNTIME_BINARY_PATH,
  targetTriple: 'x86_64-unknown-linux-musl',
  signingKeyPemFile: process.env.PRIVATE_KEY_PATH,
  signingKeyId: 'official-key-2026-04',
  issuedAt: '2026-04-21T09:30:00Z',
});
process.stdout.write(JSON.stringify(result));
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .env(
            "CORE_JS_PATH",
            repo_root().join("scripts/node/plugin/core.js"),
        )
        .env("PLUGIN_PATH", plugin_path)
        .env("OUTPUT_DIR", output_dir)
        .env("RUNTIME_BINARY_PATH", runtime_binary_path)
        .env("PRIVATE_KEY_PATH", private_key_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "node package failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let result: NodePackageResult = serde_json::from_slice(&output.stdout).unwrap();
    NodePackagedArchive {
        package_file: PathBuf::from(result.package_file),
        package_name: result.package_name,
        checksum: result.checksum,
    }
}

fn write_fake_runtime_binary(path: &Path) {
    fs::write(path, b"#!/usr/bin/env sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}
