use super::archive;
use control_plane::portable_template::PortableTemplatePackage;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};

fn package() -> PortableTemplatePackage {
    serde_json::from_value(json!({
        "schema_version":"1flowbase.portable-template/v1","pages":[],"applications":[],
        "data_models":[{"id":"019f0000-0000-7000-8000-000000000001","code":"demo","title":"Demo","description":null,"scope_kind":"workspace","template_provider":"core","template_code":"general","template_version":"v1","status":"draft","builtin":false,"fields":[]}],
        "plugins":[],"release":{"template_id":"@test/demo","release_version":2,"name":"Demo","description":"split","exported_at":"2026-10-03T00:00:00Z","exported_from_system_version":"0.4.1"}
    })).unwrap()
}
fn zip(files: BTreeMap<String, Vec<u8>>, method: zip::CompressionMethod) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, bytes) in files {
        writer
            .start_file(
                path,
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
#[test]
fn archive_roundtrip_preserves_definitions_and_is_deterministic() {
    let package = package();
    let bytes = archive::encode(&package).unwrap();
    assert_eq!(bytes, archive::encode(&package).unwrap());
    assert_eq!(
        serde_json::to_value(archive::decode(&bytes).unwrap()).unwrap(),
        serde_json::to_value(&package).unwrap()
    );
    let files = archive::source_files(&package).unwrap();
    assert!(!files.contains_key("template.json"));
    assert!(files.keys().any(|path| path.starts_with("data-models/")));
    let deflated = zip(files, zip::CompressionMethod::Deflated);
    assert_eq!(
        serde_json::to_value(archive::decode(&deflated).unwrap()).unwrap(),
        serde_json::to_value(package).unwrap()
    );
}
#[test]
fn archive_rejects_tampered_missing_undeclared_and_traversal_entries() {
    let original = archive::source_files(&package()).unwrap();
    let key = original
        .keys()
        .find(|p| p.starts_with("data-models/"))
        .unwrap()
        .clone();
    let mut tampered = original.clone();
    tampered.get_mut(&key).unwrap().push(b' ');
    assert!(
        archive::decode(&zip(tampered, zip::CompressionMethod::Stored))
            .unwrap_err()
            .to_string()
            .contains("checksum")
    );
    let mut missing = original.clone();
    missing.remove(&key);
    assert!(archive::decode(&zip(missing, zip::CompressionMethod::Stored)).is_err());
    for path in [
        "extra.json",
        "../escape.json",
        "/absolute.json",
        "a\\b.json",
    ] {
        let mut files = original.clone();
        files.insert(path.into(), b"{}".to_vec());
        assert!(
            archive::decode(&zip(files, zip::CompressionMethod::Stored)).is_err(),
            "{path}"
        );
    }
}
#[test]
fn archive_rejects_cycles_and_unreferenced_declared_files() {
    let mut files = archive::source_files(&package()).unwrap();
    let path = "plugins/dependencies.json";
    let value = serde_json::to_vec(&json!({"$file":path})).unwrap();
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["path"] == path)
        .unwrap()["sha256"] = json!(archive::checksum(&value));
    files.insert(path.into(), value);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(archive::decode(&zip(files, zip::CompressionMethod::Stored))
        .unwrap_err()
        .to_string()
        .contains("cycle"));
    let mut files = archive::source_files(&package()).unwrap();
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["package"]["plugins"] = json!([]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(archive::decode(&zip(files, zip::CompressionMethod::Stored))
        .unwrap_err()
        .to_string()
        .contains("unreferenced"));
}
#[test]
fn reference_marker_with_other_keys_is_ordinary_user_data() {
    let mut files = archive::source_files(&package()).unwrap();
    let path = files
        .keys()
        .find(|p| p.starts_with("data-models/"))
        .unwrap()
        .clone();
    let mut model: Value = serde_json::from_slice(&files[&path]).unwrap();
    model["fields"] = json!([{"id":"019f0000-0000-7000-8000-000000000002","code":"value","title":"Value","description":null,"field_kind":"json","is_system":false,"is_required":false,"api_required":false,"is_unique":false,"default_value":{"$file":"business-value","other":"retained"},"display_interface":null,"display_options":{},"relation_target_model_id":null,"relation_options":{}}]);
    let bytes = serde_json::to_vec(&model).unwrap();
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["path"] == path)
        .unwrap()["sha256"] = json!(archive::checksum(&bytes));
    files.insert(path, bytes);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let decoded = archive::decode(&zip(files, zip::CompressionMethod::Stored)).unwrap();
    assert_eq!(
        decoded.data_models[0].fields[0]
            .default_value
            .as_ref()
            .unwrap()["$file"],
        "business-value"
    );
}

// Build a manifest from the actual small fixture files so rejection tests isolate
// the reference graph rather than failing checksum or declaration validation first.
fn rewrite_manifest(files: &mut BTreeMap<String, Vec<u8>>, package: Value) {
    let manifest = archive::Manifest {
        schema_version: archive::SCHEMA.into(),
        package,
        files: files
            .iter()
            .filter(|(path, _)| path.as_str() != "manifest.json")
            .map(|(path, bytes)| archive::ManifestFile {
                path: path.clone(),
                sha256: archive::checksum(bytes),
            })
            .collect(),
    };
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
}

#[test]
fn archive_accepts_long_reference_chains_many_files_and_deep_safe_paths() {
    let expected = package();
    let mut skeleton = serde_json::to_value(&expected).unwrap();
    let mut files = BTreeMap::new();
    let paths: Vec<_> = (0..10_001)
        .map(|index| format!("references/{index:05}.json"))
        .collect();
    skeleton["plugins"] = json!({"$file":paths[0]});
    for (index, path) in paths.iter().enumerate() {
        let value = paths
            .get(index + 1)
            .map(|next| json!({"$file":next}))
            .unwrap_or_else(|| json!([]));
        files.insert(path.clone(), serde_json::to_vec(&value).unwrap());
    }
    // Path components are validated for traversal, not against a business depth.
    let deep_path = format!("{}models.json", "safe/".repeat(129));
    files.insert(
        deep_path.clone(),
        serde_json::to_vec(&skeleton["data_models"]).unwrap(),
    );
    skeleton["data_models"] = json!({"$file":deep_path});
    rewrite_manifest(&mut files, skeleton);
    assert_eq!(
        serde_json::to_value(archive::decode(&zip(files, zip::CompressionMethod::Stored)).unwrap())
            .unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn archive_encode_decode_roundtrip_accepts_more_than_ten_thousand_files() {
    let mut value = serde_json::to_value(package()).unwrap();
    let prototype = value["data_models"][0].clone();
    value["data_models"] = Value::Array(
        (0..10_001)
            .map(|index| {
                let mut model = prototype.clone();
                model["id"] = json!(format!("019f0000-0000-7000-8000-{index:012x}"));
                model["code"] = json!(format!("model_{index}"));
                model
            })
            .collect(),
    );
    let package: PortableTemplatePackage = serde_json::from_value(value.clone()).unwrap();
    let encoded = archive::encode(&package).unwrap();
    assert_eq!(
        serde_json::to_value(archive::decode(&encoded).unwrap()).unwrap(),
        value
    );
}

#[test]
fn archive_rejects_multi_file_cycles_and_undeclared_references() {
    for (second, expected_error) in [
        (json!({"$file":"first.json"}), "cycle"),
        (json!({"$file":"missing.json"}), "undeclared_reference"),
    ] {
        let mut skeleton = serde_json::to_value(package()).unwrap();
        skeleton["plugins"] = json!({"$file":"first.json"});
        let mut files = BTreeMap::from([
            (
                "first.json".into(),
                serde_json::to_vec(&json!({"$file":"second.json"})).unwrap(),
            ),
            ("second.json".into(), serde_json::to_vec(&second).unwrap()),
        ]);
        rewrite_manifest(&mut files, skeleton);
        assert!(archive::decode(&zip(files, zip::CompressionMethod::Stored))
            .unwrap_err()
            .to_string()
            .contains(expected_error));
    }
}

#[test]
fn archive_allows_repeated_references_on_separate_branches() {
    let mut skeleton = serde_json::to_value(package()).unwrap();
    // Both typed collections accept an empty array. A reference becomes inactive
    // when its branch finishes; reuse is not a graph cycle.
    skeleton["plugins"] = json!({"$file":"empty.json"});
    skeleton["applications"] = json!({"$file":"empty.json"});
    let mut files = BTreeMap::from([("empty.json".into(), b"[]".to_vec())]);
    rewrite_manifest(&mut files, skeleton);
    let decoded = archive::decode(&zip(files, zip::CompressionMethod::Stored)).unwrap();
    assert!(decoded.plugins.is_empty());
    assert!(decoded.applications.is_empty());
}

#[test]
fn archive_rejects_duplicate_manifest_paths_and_zip_entries() {
    let original = archive::source_files(&package()).unwrap();
    let mut files = original.clone();
    let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    let duplicate = manifest["files"][0].clone();
    manifest["files"]
        .as_array_mut()
        .unwrap()
        .insert(0, duplicate);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(archive::decode(&zip(files, zip::CompressionMethod::Stored))
        .unwrap_err()
        .to_string()
        .contains("file_order"));

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    // A directory and a file share the same normalized name; ZIP permits their
    // distinct entry names, while the codec must reject the collision.
    writer
        .add_directory(
            "plugins/dependencies.json/",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    for (path, bytes) in original {
        writer
            .start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    assert!(archive::decode(&writer.finish().unwrap().into_inner())
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
}

#[test]
fn official_download_verifies_exact_zip_bytes_and_pinned_release() {
    use crate::official_extension_catalog::{
        DownloadedOfficialExtensionArtifact, OfficialExtensionArtifactDescriptor,
    };
    use base64::{engine::general_purpose::STANDARD, Engine};
    use ed25519_dalek::{
        pkcs8::{spki::der::pem::LineEnding, EncodePublicKey},
        Signer, SigningKey,
    };
    let key = SigningKey::from_bytes(&[33; 32]);
    let bytes = archive::encode(&package()).unwrap();
    let trusted = vec![plugin_framework::TrustedPublicKey {
        key_id: "fixture".into(),
        algorithm: "ed25519".into(),
        public_key_pem: key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap(),
    }];
    let mut downloaded = DownloadedOfficialExtensionArtifact {
        file_name: "demo.zip".into(),
        descriptor: OfficialExtensionArtifactDescriptor {
            locator_kind: "github_release_asset".into(),
            locator: "https://example.test/demo.zip".into(),
            expected_checksum: Some(archive::checksum(&bytes)),
            signature: Some(
                json!({"algorithm":"ed25519","key_id":"fixture","signature":STANDARD.encode(key.sign(&bytes).to_bytes())}),
            ),
            platform: None,
        },
        artifact_bytes: bytes,
    };
    assert!(
        super::catalog::decode_verified_archive(&downloaded, "@test/demo", 2, &trusted).is_ok()
    );
    assert!(
        super::catalog::decode_verified_archive(&downloaded, "@test/demo", 3, &trusted).is_err()
    );
    assert!(
        super::catalog::decode_verified_archive(&downloaded, "@other/demo", 2, &trusted).is_err()
    );
    assert!(super::catalog::decode_verified_archive(&downloaded, "@test/demo", 2, &[]).is_err());
    downloaded.artifact_bytes.push(0);
    assert!(
        super::catalog::decode_verified_archive(&downloaded, "@test/demo", 2, &trusted).is_err()
    );
    // Updating only the public checksum must not allow a forged archive.
    downloaded.descriptor.expected_checksum = Some(archive::checksum(&downloaded.artifact_bytes));
    assert!(
        super::catalog::decode_verified_archive(&downloaded, "@test/demo", 2, &trusted).is_err()
    );
}

#[tokio::test]
async fn asynchronous_upload_decode_preserves_package_and_invalid_input_errors() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let expected = package();
    let encoded = STANDARD.encode(archive::encode(&expected).unwrap());
    let actual = super::catalog::decode_uploaded_archive(encoded)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    for invalid in [
        "invalid base64!".to_owned(),
        STANDARD.encode(b"invalid zip"),
    ] {
        let error = super::catalog::decode_uploaded_archive(invalid)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
                Some(control_plane::errors::ControlPlaneError::InvalidInput(
                    "application_template_archive_invalid"
                ))
            ),
            "{error:?}"
        );
    }
}

#[test]
fn archive_roundtrip_translation_only_v2_and_legacy_checksum_compatibility() {
    let legacy = package();
    let serialized = serde_json::to_value(&legacy).unwrap();
    assert!(serialized.get("i18n_entries").is_none());
    let legacy_files = archive::source_files(&legacy).unwrap();
    assert!(!legacy_files.keys().any(|path| path.starts_with("i18n/")));
    let mut explicit_empty = serialized.clone();
    explicit_empty["i18n_entries"] = json!([]);
    let explicit_empty: PortableTemplatePackage = serde_json::from_value(explicit_empty).unwrap();
    assert_eq!(
        archive::encode(&legacy).unwrap(),
        archive::encode(&explicit_empty).unwrap()
    );
    assert_eq!(
        control_plane::portable_template::application_template_checksum(&legacy).unwrap(),
        control_plane::portable_template::application_template_checksum(&explicit_empty).unwrap(),
    );

    let mut translations = legacy;
    translations.data_models.clear();
    translations.schema_version =
        control_plane::portable_template::PORTABLE_TEMPLATE_I18N_SCHEMA_VERSION.into();
    translations.i18n_entries = vec![control_plane::portable_template::PortableI18nEntry {
        key: "template.demo.title".into(),
        locale: "zh_Hans".into(),
        translation: "模板标题".into(),
    }];
    let files = archive::source_files(&translations).unwrap();
    assert!(files.keys().any(|path| path.starts_with("i18n/zh_Hans/")));
    let decoded = archive::decode(&archive::encode(&translations).unwrap()).unwrap();
    assert_eq!(decoded.i18n_entries, translations.i18n_entries);
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::to_value(translations).unwrap()
    );
}
