use super::*;
use std::io::{Cursor, Write};

fn fixture_manifest() -> domain::ClientCollectorManifest {
    let names = [
        "install.sh",
        "install.ps1",
        "README.md",
        "README.en.md",
        "release.json",
        "checksums.txt",
        "checksums.txt.sig",
        "signing-public-key.pem",
        "linux-x64.tar.gz",
    ];
    let mut manifest = domain::ClientCollectorManifest {
        schema_version: "1flowbase.client-collector/v1".into(),
        distribution_kind: domain::ClientCollectorDistributionKind::ClientCollector,
        organization: "taichuy".into(),
        artifact_id: "codex-logs-collector".into(),
        collector_code: "codex-logs-collector".into(),
        source_client: "codex".into(),
        display_name: "Codex".into(),
        version: "0.1.0".into(),
        execution_target: "client".into(),
        protocol_version: "1flowbase.agent-logs/v1".into(),
        entry: "codex-logs-collector".into(),
        minimum_host_version: "0.5.3".into(),
        description: BTreeMap::from([
            ("en_US".into(), "Local logs".into()),
            ("zh_Hans".into(), "本机日志".into()),
        ]),
        assets: names
            .into_iter()
            .map(|name| domain::ClientCollectorAsset {
                name: name.into(),
                size: 6,
                sha256: format!("{:x}", Sha256::digest(b"opaque")),
            })
            .collect(),
    };
    refresh_release_asset(&mut manifest);
    manifest
}
fn fixture_release(
    manifest: &domain::ClientCollectorManifest,
) -> domain::ClientCollectorReleaseManifest {
    domain::ClientCollectorReleaseManifest {
        schema_version: "1flowbase.client-collector-release/v1".into(),
        collector_code: manifest.collector_code.clone(),
        version: manifest.version.clone(),
        source_sha: "fixture-source".into(),
        signature_algorithm: "ed25519".into(),
        signing_key_id: "fixture-key".into(),
        artifacts: manifest
            .assets
            .iter()
            .filter(|asset| asset.name.ends_with(".tar.gz") || asset.name.ends_with(".zip"))
            .map(|asset| domain::ClientCollectorReleaseArtifact {
                os: "linux".into(),
                arch: "amd64".into(),
                rust_target: "x86_64-unknown-linux-musl".into(),
                name: asset.name.clone(),
                sha256: asset.sha256.clone(),
                size: asset.size,
            })
            .collect(),
    }
}
fn refresh_release_asset(manifest: &mut domain::ClientCollectorManifest) {
    let bytes = serde_json::to_vec(&fixture_release(manifest)).unwrap();
    let asset = manifest
        .assets
        .iter_mut()
        .find(|asset| asset.name == "release.json")
        .unwrap();
    asset.size = bytes.len() as u64;
    asset.sha256 = format!("{:x}", Sha256::digest(bytes));
}
fn archive(
    manifest: &domain::ClientCollectorManifest,
    extra: Option<(&str, tar::EntryType)>,
    corrupt: bool,
    missing: bool,
) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    fn append(builder: &mut tar::Builder<Vec<u8>>, name: &str, bytes: &[u8], kind: tar::EntryType) {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(kind);
        if kind.is_symlink() || kind.is_hard_link() {
            header.set_link_name("install.sh").unwrap();
        }
        header.set_cksum();
        builder.append_data(&mut header, name, bytes).unwrap();
    }
    append(
        &mut builder,
        "collector-manifest.json",
        &serde_json::to_vec(manifest).unwrap(),
        tar::EntryType::Regular,
    );
    for (index, asset) in manifest.assets.iter().enumerate() {
        if missing && index == 0 {
            continue;
        }
        let bytes = if asset.name == "release.json" {
            serde_json::to_vec(&fixture_release(manifest)).unwrap()
        } else if corrupt && index == 0 {
            b"broken".to_vec()
        } else {
            b"opaque".to_vec()
        };
        append(&mut builder, &asset.name, &bytes, tar::EntryType::Regular);
    }
    if let Some((name, kind)) = extra {
        append(
            &mut builder,
            name,
            if kind.is_file() { b"extra" } else { b"" },
            kind,
        );
    }
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&builder.into_inner().unwrap()).unwrap();
    gzip.finish().unwrap()
}
#[test]
fn validates_opaque_assets_and_rejects_integrity_and_member_mutations() {
    let manifest = fixture_manifest();
    assert_eq!(
        inspect_client_collector_archive(Cursor::new(archive(&manifest, None, false, false)))
            .unwrap()
            .descriptor(),
        manifest.descriptor()
    );
    for bytes in [
        archive(&manifest, None, true, false),
        archive(&manifest, None, false, true),
        archive(
            &manifest,
            Some(("undeclared", tar::EntryType::Regular)),
            false,
            false,
        ),
        archive(
            &manifest,
            Some(("install.sh", tar::EntryType::Regular)),
            false,
            false,
        ),
        archive(
            &manifest,
            Some(("linked", tar::EntryType::Symlink)),
            false,
            false,
        ),
        archive(
            &manifest,
            Some(("linked", tar::EntryType::Link)),
            false,
            false,
        ),
    ] {
        assert!(inspect_client_collector_archive(bytes.as_slice()).is_err());
    }
    let mut duplicate = manifest.clone();
    duplicate.assets.push(duplicate.assets[0].clone());
    assert!(validate_client_collector_manifest(&duplicate).is_err());
    let mut unsafe_name = manifest.clone();
    unsafe_name.assets[0].name = "../install.sh".into();
    assert!(validate_client_collector_manifest(&unsafe_name).is_err());
    let mut server = manifest.clone();
    server.execution_target = "server".into();
    assert!(validate_client_collector_manifest(&server).is_err());
    let identity = domain::ExtensionInstallationIdentity {
        category: domain::ExtensionCategory::RuntimeExtensions,
        organization: "other".into(),
        artifact_id: manifest.artifact_id.clone(),
        version: manifest.version.clone(),
    };
    assert!(ensure_client_collector_identity(&manifest, &identity).is_err());
}
#[test]
fn flat_public_selector_rejects_paths_and_manifest_selector() {
    for name in [
        "../install.sh",
        "./install.sh",
        "a/b",
        "a\\b",
        "",
        ".",
        "..",
        "a%2fb",
        "a?b",
    ] {
        assert!(!domain::client_collector_flat_name(name));
    }
    assert!(domain::client_collector_flat_name("README.en.md"));
}

pub(crate) fn collector_fixture_archive(artifact_id: &str, version: &str) -> Vec<u8> {
    let mut manifest = fixture_manifest();
    manifest.artifact_id = artifact_id.into();
    manifest.version = version.into();
    refresh_release_asset(&mut manifest);
    archive(&manifest, None, false, false)
}

#[test]
fn release_manifest_rejects_empty_duplicate_or_inconsistent_native_assets() {
    let manifest = fixture_manifest();
    let release = fixture_release(&manifest);
    validate_client_collector_release(&manifest, &release).unwrap();
    let mut empty = release.clone();
    empty.artifacts.clear();
    assert!(validate_client_collector_release(&manifest, &empty).is_err());
    let mut duplicate = release.clone();
    duplicate.artifacts.push(duplicate.artifacts[0].clone());
    assert!(validate_client_collector_release(&manifest, &duplicate).is_err());
    let mut inconsistent = release.clone();
    inconsistent.artifacts[0].size += 1;
    assert!(validate_client_collector_release(&manifest, &inconsistent).is_err());
    let mut mismatched = release;
    mismatched.version = "0.2.0".into();
    assert!(validate_client_collector_release(&manifest, &mismatched).is_err());
    let mut no_native = manifest;
    no_native
        .assets
        .retain(|asset| !asset.name.ends_with(".tar.gz"));
    refresh_release_asset(&mut no_native);
    assert!(
        inspect_client_collector_archive(archive(&no_native, None, false, false).as_slice())
            .is_err()
    );
}
