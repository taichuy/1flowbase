use super::releases::load_packages;
use serde_json::json;

#[test]
fn discovery_uses_scoped_packages_ignores_receipts_and_rejects_duplicate_ids() {
    let root = std::env::temp_dir().join(format!("application-templates-{}", uuid::Uuid::new_v4()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    assert!(load_packages(root.to_str().unwrap()).unwrap().is_empty());
    let directory = root.join("@taichuy/gateway-demo");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(root.join("receipt.json"), "this is not a package").unwrap();
    let package = json!({"schema_version":"1flowbase.portable-template/v1", "pages":[], "applications":[], "data_models":[],
        "release":{"template_id":"@taichuy/gateway-demo", "release_version":1,"name":"Gateway demo","description":"Demo", "exported_from_system_version":"0.4.1","exported_at":"2026-10-03T00:00:00Z"}});
    std::fs::write(directory.join("template.json"), package.to_string()).unwrap();
    assert_eq!(load_packages(root.to_str().unwrap()).unwrap().len(), 1);
    let duplicate = root.join("@taichuy/duplicate");
    std::fs::create_dir_all(&duplicate).unwrap();
    std::fs::write(duplicate.join("template.json"), package.to_string()).unwrap();
    assert!(load_packages(root.to_str().unwrap())
        .unwrap_err()
        .to_string()
        .contains("duplicate_id"));
}
