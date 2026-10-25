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
            bytes: b"hello".to_vec(),
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

#[tokio::test]
async fn local_driver_preserves_empty_and_binary_objects_with_metadata() {
    let driver = LocalFileStorageDriver;
    let root = temp_root();
    let config_json = serde_json::json!({
        "root_path": root.display().to_string(),
        "public_base_url": " https://files.example.test/root/ "
    });
    for (object_path, bytes, content_type) in [
        ("empty.bin", Vec::new(), None),
        (
            "nested/binary.bin",
            (0..=255).collect::<Vec<u8>>(),
            Some("application/octet-stream"),
        ),
    ] {
        let expected = bytes.clone();
        let stored = driver
            .put_object(FileStoragePutInput {
                config_json: &config_json,
                object_path,
                content_type,
                bytes,
            })
            .await
            .unwrap();
        assert_eq!(stored.path, object_path);
        assert_eq!(
            stored.url,
            Some(format!("https://files.example.test/root/{object_path}"))
        );
        assert_eq!(
            stored.metadata_json,
            serde_json::json!({"driver_type": "local", "content_type": content_type})
        );
        let read = driver
            .open_read(OpenReadInput {
                config_json: &config_json,
                object_path,
            })
            .await
            .unwrap();
        assert_eq!(read.bytes, expected);
        assert_eq!(read.content_type.as_deref(), content_type);
        assert_eq!(
            fs::metadata(root.join(object_path)).unwrap().len(),
            expected.len() as u64
        );
        let sidecar: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join(format!("{object_path}.metadata.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(sidecar, serde_json::json!({"content_type": content_type}));
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn local_driver_put_preserves_validation_and_write_failure_order() {
    use storage_object::FileStorageError;

    let driver = LocalFileStorageDriver;
    let root = temp_root();
    let config_json = serde_json::json!({"root_path": root.display().to_string()});
    for object_path in ["", "../escape.bin", "/absolute.bin"] {
        let error = driver
            .put_object(FileStoragePutInput {
                config_json: &config_json,
                object_path,
                content_type: None,
                bytes: vec![0, 255],
            })
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            FileStorageError::InvalidConfig("object_path")
        ));
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);

    // A content-write failure must not proceed to the metadata write.
    fs::create_dir(root.join("blocked.bin")).unwrap();
    let error = driver
        .put_object(FileStoragePutInput {
            config_json: &config_json,
            object_path: "blocked.bin",
            content_type: Some("application/octet-stream"),
            bytes: vec![0, 255],
        })
        .await
        .unwrap_err();
    assert!(matches!(error, FileStorageError::Other(_)));
    assert!(!root.join("blocked.bin.metadata.json").exists());

    // A metadata-write failure retains the previously written content, as before.
    fs::create_dir(root.join("partial.bin.metadata.json")).unwrap();
    let error = driver
        .put_object(FileStoragePutInput {
            config_json: &config_json,
            object_path: "partial.bin",
            content_type: Some("application/octet-stream"),
            bytes: vec![0, 255],
        })
        .await
        .unwrap_err();
    assert!(matches!(error, FileStorageError::Other(_)));
    assert_eq!(fs::read(root.join("partial.bin")).unwrap(), [0, 255]);
    assert!(root.join("partial.bin.metadata.json").is_dir());
    fs::remove_dir_all(root).unwrap();
}
