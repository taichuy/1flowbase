use std::{fs, path::PathBuf};

use storage_object::{
    drivers::local::LocalFileStorageDriver, DeleteObjectInput, FileStorageDriver,
    FileStoragePutInput, GenerateAccessUrlInput, OpenReadInput,
};
use uuid::Uuid;

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("storage-object-local-{}", Uuid::now_v7()));
    fs::create_dir_all(&root).unwrap();
    root
}

#[tokio::test]
async fn local_driver_put_read_delete_roundtrip() {
    let driver = LocalFileStorageDriver;
    let root = temp_root();
    let config_json = serde_json::json!({
        "root_path": root.display().to_string(),
        "public_base_url": null
    });

    let stored = driver
        .put_object(FileStoragePutInput {
            config_json: &config_json,
            object_path: "attachments/2026/04/demo.txt",
            content_type: Some("text/plain"),
            bytes: b"hello",
        })
        .await
        .unwrap();
    assert_eq!(stored.path, "attachments/2026/04/demo.txt");
    assert_eq!(stored.url, None);

    let read = driver
        .open_read(OpenReadInput {
            config_json: &config_json,
            object_path: &stored.path,
        })
        .await
        .unwrap();
    assert_eq!(read.bytes, b"hello");
    assert_eq!(read.content_type.as_deref(), Some("text/plain"));

    assert_eq!(
        driver
            .generate_access_url(GenerateAccessUrlInput {
                config_json: &config_json,
                object_path: &stored.path,
            })
            .await
            .unwrap(),
        None
    );

    driver
        .delete_object(DeleteObjectInput {
            config_json: &config_json,
            object_path: &stored.path,
        })
        .await
        .unwrap();

    assert!(driver
        .open_read(OpenReadInput {
            config_json: &config_json,
            object_path: &stored.path,
        })
        .await
        .is_err());
}
