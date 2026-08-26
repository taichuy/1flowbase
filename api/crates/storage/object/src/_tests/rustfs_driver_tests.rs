use storage_object::{
    drivers::rustfs::RustfsFileStorageDriver, FileStorageDriver, GenerateAccessUrlInput,
};

#[test]
fn rustfs_driver_requires_endpoint_bucket_access_key_and_secret_key() {
    let driver = RustfsFileStorageDriver;

    assert!(driver
        .validate_config(&serde_json::json!({
            "endpoint": "http://127.0.0.1:39000",
            "bucket": "attachments"
        }))
        .is_err());

    assert!(driver
        .validate_config(&serde_json::json!({
            "endpoint": "http://127.0.0.1:39000",
            "bucket": "attachments",
            "access_key": "rustfsadmin",
            "secret_key": "rustfsadmin"
        }))
        .is_ok());
}

#[tokio::test]
async fn rustfs_healthcheck_reports_invalid_endpoint_before_network_io() {
    let driver = RustfsFileStorageDriver;
    let error = driver
        .healthcheck(&serde_json::json!({
            "endpoint": "",
            "bucket": "attachments",
            "access_key": "rustfsadmin",
            "secret_key": "rustfsadmin"
        }))
        .await
        .unwrap_err();

    assert_eq!(error.to_string(), "invalid file storage config: endpoint");
}

#[tokio::test]
async fn rustfs_generate_access_url_uses_public_base_url_when_configured() {
    let driver = RustfsFileStorageDriver;

    assert_eq!(
        driver
            .generate_access_url(GenerateAccessUrlInput {
                config_json: &serde_json::json!({
                    "endpoint": "http://127.0.0.1:39000",
                    "bucket": "attachments",
                    "access_key": "rustfsadmin",
                    "secret_key": "rustfsadmin",
                    "public_base_url": "https://cdn.example.com/files/"
                }),
                object_path: "attachments/2026/04/demo.txt",
            })
            .await
            .unwrap(),
        Some("https://cdn.example.com/files/attachments/2026/04/demo.txt".to_string())
    );
}
