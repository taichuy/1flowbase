use super::*;
use futures_util::TryStreamExt;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn asset_protocol_propagates_worker_failure_and_incomplete_length() {
    for (bytes, size, worker_result) in [
        (
            b"ok".as_slice(),
            2,
            Err(std::io::Error::other("worker failed")),
        ),
        (b"x".as_slice(), 2, Ok(())),
    ] {
        let (reader, mut writer) = tokio::io::duplex(64);
        writer.write_all(bytes).await.unwrap();
        drop(writer);
        let (finished, completion) = tokio::sync::oneshot::channel();
        finished.send(worker_result).unwrap();
        assert!(asset_stream(reader, completion, size)
            .try_collect::<Vec<_>>()
            .await
            .is_err());
    }
    let (reader, mut writer) = tokio::io::duplex(64);
    writer.write_all(b"ok").await.unwrap();
    drop(writer);
    let (finished, completion) = tokio::sync::oneshot::channel();
    finished.send(Ok(())).unwrap();
    assert_eq!(
        asset_stream(reader, completion, 2)
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .concat(),
        b"ok"
    );
}
