use super::{archive, releases::load_packages};
use serde_json::json;

#[tokio::test]
async fn discovery_uses_scoped_packages_ignores_receipts_and_rejects_duplicate_ids() {
    let root = std::env::temp_dir().join(format!("application-templates-{}", uuid::Uuid::new_v4()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    assert!(load_packages(root.to_str().unwrap())
        .await
        .unwrap()
        .is_empty());
    let directory = root.join("@taichuy/gateway-demo");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(root.join("receipt.json"), "this is not a package").unwrap();
    let package = json!({"schema_version":"1flowbase.portable-template/v1", "pages":[], "applications":[], "data_models":[],
        "release":{"template_id":"@taichuy/gateway-demo", "release_version":1,"name":"Gateway demo","description":"Demo", "exported_from_system_version":"0.4.1","exported_at":"2026-10-03T00:00:00Z"}});
    let package = serde_json::from_value(package).unwrap();
    let bytes = archive::encode(&package).unwrap();
    std::fs::write(directory.join("template.zip"), &bytes).unwrap();
    assert_eq!(
        load_packages(root.to_str().unwrap()).await.unwrap().len(),
        1
    );
    let duplicate = root.join("@taichuy/duplicate");
    std::fs::create_dir_all(&duplicate).unwrap();
    std::fs::write(duplicate.join("template.zip"), bytes).unwrap();
    assert!(load_packages(root.to_str().unwrap())
        .await
        .unwrap_err()
        .to_string()
        .contains("duplicate_id"));
}

#[tokio::test]
async fn split_source_and_zip_have_the_same_install_package() {
    let root = std::env::temp_dir().join(format!("template-split-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let package=serde_json::from_value(json!({"schema_version":"1flowbase.portable-template/v1","pages":[],"applications":[],"data_models":[],"plugins":[]})).unwrap();
    for (path, bytes) in archive::source_files(&package).unwrap() {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    assert_eq!(
        serde_json::to_value(archive::load_directory(&root).await.unwrap()).unwrap(),
        serde_json::to_value(archive::decode(&archive::encode(&package).unwrap()).unwrap())
            .unwrap()
    );
    tokio::fs::write(root.join("plugins/dependencies.json"), b"[42]")
        .await
        .unwrap();
    assert!(archive::load_directory(&root)
        .await
        .unwrap_err()
        .to_string()
        .contains("checksum"));
    tokio::fs::remove_file(root.join("manifest.json"))
        .await
        .unwrap();
    let missing = archive::load_directory(&root).await.unwrap_err();
    assert_eq!(
        missing.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::NotFound
    );
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn discovery_reads_metadata_from_the_same_packages_the_codec_accepts() {
    let root =
        std::env::temp_dir().join(format!("template-large-manifest-{}", uuid::Uuid::new_v4()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let directory = root.join("@fixture/large-metadata");
    std::fs::create_dir_all(&directory).unwrap();
    let description = "metadata".repeat(524_289);
    let package = serde_json::from_value(json!({
        "schema_version":"1flowbase.portable-template/v1",
        "pages":[],"applications":[],"data_models":[],"plugins":[],
        "release":{"template_id":"@fixture/large-metadata","release_version":1,
            "name":"Large metadata","description":description,
            "exported_at":"2026-10-03T00:00:00Z","exported_from_system_version":"0.4.1"}
    }))
    .unwrap();
    std::fs::write(
        directory.join("template.zip"),
        archive::encode(&package).unwrap(),
    )
    .unwrap();
    let releases = super::releases::discover(root.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].release.description, description);
    assert_eq!(
        serde_json::to_value(releases[0].load().await.unwrap()).unwrap(),
        serde_json::to_value(package).unwrap()
    );
}
