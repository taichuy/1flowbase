use super::*;
use std::{
    io::Cursor,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};
struct Observed {
    inner: Cursor<Vec<u8>>,
    drops: Arc<AtomicUsize>,
    pending: bool,
    started: Arc<AtomicBool>,
}
impl Drop for Observed {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl AsyncRead for Observed {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.started.store(true, Ordering::SeqCst);
        if self.pending {
            return Poll::Pending;
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
struct Port {
    allowed: AtomicBool,
    length: u64,
    actual: usize,
    pending: bool,
    drops: Arc<AtomicUsize>,
    started: Arc<AtomicBool>,
}
impl FileReadPort for Port {
    fn authorize<'a>(
        &'a self,
        _: &'a FileReadBinding,
    ) -> runtime_core::runtime_backend::FileReadFuture<'a, ()> {
        Box::pin(async move {
            if self.allowed.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(FileReadError::new("file_read_caller_invalid"))
            }
        })
    }
    fn open<'a>(
        &'a self,
        _: &'a FileReadBinding,
        _: &'a str,
        _: &'a str,
    ) -> runtime_core::runtime_backend::FileReadFuture<'a, FileReadSource> {
        Box::pin(async move {
            Ok(FileReadSource {
                metadata: extension_contracts::FileReadMetadata {
                    filename: "a.bin".into(),
                    content_type: None,
                    content_length: self.length,
                },
                reader: Box::pin(Observed {
                    inner: Cursor::new(vec![42; self.actual]),
                    drops: self.drops.clone(),
                    pending: self.pending,
                    started: self.started.clone(),
                }),
            })
        })
    }
}
fn fixture(length: u64, actual: usize, pending: bool) -> (FileReadCall, Arc<Port>) {
    let port = Arc::new(Port {
        allowed: AtomicBool::new(true),
        length,
        actual,
        pending,
        drops: Arc::new(AtomicUsize::new(0)),
        started: Arc::new(AtomicBool::new(false)),
    });
    let binding = FileReadBinding {
        subject: extension_contracts::ManagedContributionSubject::new(extension_contracts::ManagedInstallationId::new("installation").unwrap(), extension_contracts::ManagedWorkspaceId::new("system").unwrap(), extension_contracts::ContributionId::new("read").unwrap()),
        publisher_namespace:"acme".into(), plugin_code:"ssh".into(), plugin_version:"1.0.0".into(),
        caller: runtime_core::runtime_backend::RuntimeCallerContext { actor: serde_json::from_value(serde_json::json!({"user_id":"00000000-0000-0000-0000-000000000001","tenant_id":"00000000-0000-0000-0000-000000000000","current_workspace_id":"00000000-0000-0000-0000-000000000002","effective_display_role":"root","is_root":true,"permissions":[]})).unwrap(), session_version:7, session_identity:None, interface_id:"read".into(), selected_mcp_instance_ids:None }, deadline_unix_ms:i64::MAX,
    };
    (FileReadCall::new(binding, port.clone()), port)
}
async fn open(call: &FileReadCall) -> String {
    let response = call.execute(serde_json::json!({"operation":"open","file_table_id":"00000000-0000-0000-0000-000000000001","record_id":"00000000-0000-0000-0000-000000000002"})).await.unwrap();
    let FileReadResponse::Open { ticket, .. } = response else {
        panic!("open expected")
    };
    ticket
}
#[tokio::test]
async fn tickets_are_call_scoped_and_invalid_requests_or_currentness_failure_release_reader() {
    let (call, port) = fixture(5, 5, false);
    let ticket = open(&call).await;
    let (other, _) = fixture(5, 5, false);
    assert_eq!(
        other
            .execute(serde_json::json!({"operation":"chunk","ticket":ticket,"offset":0}))
            .await
            .unwrap_err()
            .code,
        "file_read_ticket_invalid"
    );
    port.allowed.store(false, Ordering::SeqCst);
    assert_eq!(
        call.execute(serde_json::json!({"operation":"chunk","ticket":ticket,"offset":0}))
            .await
            .unwrap_err()
            .code,
        "file_read_caller_invalid"
    );
    assert_eq!(port.drops.load(Ordering::SeqCst), 1);
    let (call, port) = fixture(5, 5, false);
    open(&call).await;
    assert!(call
        .execute(serde_json::json!({"operation":"open","actor":"forged"}))
        .await
        .is_err());
    assert_eq!(port.drops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn truncated_oversized_and_wrong_offset_reads_fail_closed() {
    for (length, actual, offset, code) in [
        (5, 4, 0, "file_read_truncated"),
        (5, 6, 0, "file_read_length_invalid"),
        (5, 5, 1, "file_read_ticket_invalid"),
    ] {
        let (call, port) = fixture(length, actual, false);
        let ticket = open(&call).await;
        assert_eq!(
            call.execute(serde_json::json!({"operation":"chunk","ticket":ticket,"offset":offset}))
                .await
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(port.drops.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn empty_file_has_verified_eof_and_close_releases_reader() {
    let (call, port) = fixture(0, 0, false);
    let ticket = open(&call).await;
    let response = call
        .execute(serde_json::json!({"operation":"chunk","ticket":ticket,"offset":0}))
        .await
        .unwrap();
    assert!(
        matches!(response, FileReadResponse::Chunk { eof:true, sha256:Some(hash), .. } if hash == format!("{:x}",base16ct::HexDisplay(&Sha256::digest([]))))
    );
    call.execute(serde_json::json!({"operation":"close","ticket":ticket}))
        .await
        .unwrap();
    assert_eq!(port.drops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn cancelled_chunk_and_active_call_drop_release_readers() {
    let (call, port) = fixture(5, 5, true);
    let ticket = open(&call).await;
    let child = call.clone();
    let task = tokio::spawn(async move {
        child
            .execute(serde_json::json!({"operation":"chunk","ticket":ticket,"offset":0}))
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !port.started.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    assert_eq!(port.drops.load(Ordering::SeqCst), 1);
    let (call, port) = fixture(5, 5, false);
    open(&call).await;
    drop(call);
    assert_eq!(port.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn deadline_drops_a_pending_read() {
    let (mut call, port) = fixture(5, 5, true);
    let ticket = open(&call).await;
    call.binding.deadline_unix_ms =
        i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).unwrap()
            + 20;
    assert_eq!(
        call.execute(serde_json::json!({"operation":"chunk","ticket":ticket,"offset":0}))
            .await
            .unwrap_err()
            .code,
        "file_read_deadline_exceeded"
    );
    assert_eq!(port.drops.load(Ordering::SeqCst), 1);
}
