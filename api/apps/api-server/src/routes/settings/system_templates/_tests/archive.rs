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
