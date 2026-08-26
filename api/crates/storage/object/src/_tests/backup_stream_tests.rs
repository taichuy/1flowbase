use std::{
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use storage_object::{
    drivers::{
        local::LocalFileStorageDriver,
        rustfs::{read_declared_part, RustfsFileStorageDriver, RUSTFS_MULTIPART_CHUNK_BYTES},
    },
    FileStorageDriver, FileStorageError, FileStoragePutInput, FileStoragePutStreamInput,
    OpenReadInput, VerifyReadUnchangedInput, FILE_STORAGE_STREAM_BUFFER_BYTES,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadBuf};
use uuid::Uuid;

struct ObservedReader {
    bytes: Vec<u8>,
    offset: usize,
    maximum_request: Arc<AtomicUsize>,
}

impl AsyncRead for ObservedReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.maximum_request
            .fetch_max(buffer.remaining(), Ordering::SeqCst);
        let available = self.bytes.len().saturating_sub(self.offset);
        let read = available.min(buffer.remaining());
        if read > 0 {
            let end = self.offset + read;
            buffer.put_slice(&self.bytes[self.offset..end]);
            self.offset = end;
        }
        Poll::Ready(Ok(()))
    }
}

fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!("storage-object-backup-stream-{}", Uuid::now_v7()))
}

#[tokio::test]
async fn local_large_object_round_trip_uses_a_fixed_copy_buffer() {
    let driver = LocalFileStorageDriver;
    let root = temp_root();
    let config = serde_json::json!({ "root_path": root.display().to_string() });
    let bytes = vec![0x5a; FILE_STORAGE_STREAM_BUFFER_BYTES * 3 + 17];
    let maximum_request = Arc::new(AtomicUsize::new(0));
    driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "workspace/attachments/large.bin",
            content_type: Some("application/octet-stream"),
            content_length: bytes.len() as u64,
            reader: Box::pin(ObservedReader {
                bytes: bytes.clone(),
                offset: 0,
                maximum_request: maximum_request.clone(),
            }),
        })
        .await
        .unwrap();
    assert!(maximum_request.load(Ordering::SeqCst) <= FILE_STORAGE_STREAM_BUFFER_BYTES);

    let mut opened = driver
        .open_read_stream(OpenReadInput {
            config_json: &config,
            object_path: "workspace/attachments/large.bin",
        })
        .await
        .unwrap();
    assert_eq!(opened.snapshot.content_length, bytes.len() as u64);
    let mut restored = Vec::new();
    opened.reader.read_to_end(&mut restored).await.unwrap();
    driver
        .verify_read_unchanged(VerifyReadUnchangedInput {
            config_json: &config,
            object_path: "workspace/attachments/large.bin",
            snapshot: &opened.snapshot,
        })
        .await
        .unwrap();
    assert_eq!(restored, bytes);
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn local_zero_byte_object_is_a_valid_stream_component() {
    let driver = LocalFileStorageDriver;
    let root = temp_root();
    let config = serde_json::json!({ "root_path": root.display().to_string() });
    driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "workspace/attachments/empty.bin",
            content_type: Some("application/octet-stream"),
            content_length: 0,
            reader: Box::pin(std::io::Cursor::new(Vec::<u8>::new())),
        })
        .await
        .unwrap();
    let opened = driver
        .open_read_stream(OpenReadInput {
            config_json: &config,
            object_path: "workspace/attachments/empty.bin",
        })
        .await
        .unwrap();
    assert_eq!(opened.snapshot.content_length, 0);
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn local_missing_and_length_mismatch_fail_closed() {
    let driver = LocalFileStorageDriver;
    let root = temp_root();
    let config = serde_json::json!({ "root_path": root.display().to_string() });
    let missing = match driver
        .open_read_stream(OpenReadInput {
            config_json: &config,
            object_path: "missing.bin",
        })
        .await
    {
        Ok(_) => panic!("missing object unexpectedly opened"),
        Err(error) => error,
    };
    assert!(matches!(missing, FileStorageError::ObjectNotFound));

    let mismatch = driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "short.bin",
            content_type: None,
            content_length: 4,
            reader: Box::pin(std::io::Cursor::new(vec![1_u8; 3])),
        })
        .await
        .unwrap_err();
    assert!(matches!(mismatch, FileStorageError::ObjectLengthMismatch));
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test]
async fn local_mutation_after_open_invalidates_the_read_snapshot() {
    let driver = LocalFileStorageDriver;
    let root = temp_root();
    let config = serde_json::json!({ "root_path": root.display().to_string() });
    driver
        .put_object(FileStoragePutInput {
            config_json: &config,
            object_path: "runtime/debug.json",
            content_type: Some("application/json"),
            bytes: &[7_u8; 512],
        })
        .await
        .unwrap();
    let mut opened = driver
        .open_read_stream(OpenReadInput {
            config_json: &config,
            object_path: "runtime/debug.json",
        })
        .await
        .unwrap();
    let mut prefix = [0_u8; 32];
    opened.reader.read_exact(&mut prefix).await.unwrap();
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(root.join("runtime/debug.json"))
        .await
        .unwrap();
    file.write_all(b"changed").await.unwrap();
    file.flush().await.unwrap();
    let mut remainder = Vec::new();
    opened.reader.read_to_end(&mut remainder).await.unwrap();
    let error = driver
        .verify_read_unchanged(VerifyReadUnchangedInput {
            config_json: &config,
            object_path: "runtime/debug.json",
            snapshot: &opened.snapshot,
        })
        .await
        .unwrap_err();
    assert!(matches!(error, FileStorageError::ObjectChanged));
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn rustfs_rejects_streams_beyond_the_fixed_multipart_boundary_before_network_io() {
    let driver = RustfsFileStorageDriver;
    let config = serde_json::json!({
        "endpoint": "http://127.0.0.1:1",
        "bucket": "fixtures",
        "access_key": "fixture",
        "secret_key": "fixture",
    });
    let error = driver
        .put_object_stream(FileStoragePutStreamInput {
            config_json: &config,
            object_path: "too-large.bin",
            content_type: None,
            content_length: RUSTFS_MULTIPART_CHUNK_BYTES as u64 * 10_000 + 1,
            reader: Box::pin(std::io::Cursor::new(Vec::<u8>::new())),
        })
        .await
        .unwrap_err();
    assert!(matches!(error, FileStorageError::ObjectTooLarge));
}

#[tokio::test]
async fn mock_rustfs_part_reader_never_requests_more_than_one_fixed_part() {
    let maximum_request = Arc::new(AtomicUsize::new(0));
    let mut reader: storage_object::FileStorageStreamReader = Box::pin(ObservedReader {
        bytes: vec![0x3c; RUSTFS_MULTIPART_CHUNK_BYTES * 2 + 17],
        offset: 0,
        maximum_request: maximum_request.clone(),
    });
    assert_eq!(
        read_declared_part(&mut reader, RUSTFS_MULTIPART_CHUNK_BYTES)
            .await
            .unwrap()
            .len(),
        RUSTFS_MULTIPART_CHUNK_BYTES
    );
    assert_eq!(
        read_declared_part(&mut reader, RUSTFS_MULTIPART_CHUNK_BYTES)
            .await
            .unwrap()
            .len(),
        RUSTFS_MULTIPART_CHUNK_BYTES
    );
    assert_eq!(read_declared_part(&mut reader, 17).await.unwrap().len(), 17);
    assert!(maximum_request.load(Ordering::SeqCst) <= RUSTFS_MULTIPART_CHUNK_BYTES);
}
