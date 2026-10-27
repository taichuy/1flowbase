//! A local PostgreSQL wire fault, scoped to this disposable database. No product
//! hook or fake commit result: the server finishes COMMIT and the TCP proxy drops
//! its CommandComplete frame before SQLx receives the acknowledgement.
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub(super) struct CommitAckProxy {
    task: tokio::task::JoinHandle<()>,
    dropped: Arc<AtomicUsize>,
}
impl CommitAckProxy {
    pub async fn start(url: &str) -> (Self, PgConnectOptions) {
        let options = PgConnectOptions::from_str(url).unwrap();
        assert!(options.get_database().unwrap().starts_with("test_backup_"));
        let upstream = (options.get_host().to_owned(), options.get_port());
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let dropped = Arc::new(AtomicUsize::new(0));
        let task = tokio::spawn({
            let dropped = dropped.clone();
            async move {
                let mut sessions = tokio::task::JoinSet::new();
                loop {
                    tokio::select! {
                        incoming = listener.accept() => {
                            let Ok((client, _)) = incoming else { break; };
                            let upstream = upstream.clone(); let dropped = dropped.clone();
                            sessions.spawn(async move {
                                if let Ok(server) = TcpStream::connect(upstream).await {
                                    let _ = forward(client, server, dropped).await;
                                }
                            });
                        }
                        _ = sessions.join_next(), if !sessions.is_empty() => {}
                    }
                }
            }
        });
        (
            Self { task, dropped },
            options
                .host("127.0.0.1")
                .port(port)
                .ssl_mode(PgSslMode::Disable),
        )
    }
    pub fn dropped_commits(&self) -> usize {
        self.dropped.load(Ordering::SeqCst)
    }
}
impl Drop for CommitAckProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn read_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> std::io::Result<(u8, Vec<u8>)> {
    let tag = reader.read_u8().await?;
    let length = reader.read_u32().await?;
    let mut body = vec![
        0;
        length
            .checked_sub(4)
            .ok_or(std::io::ErrorKind::InvalidData)? as usize
    ];
    reader.read_exact(&mut body).await?;
    Ok((tag, body))
}
async fn write_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    tag: u8,
    body: &[u8],
) -> std::io::Result<()> {
    writer.write_u8(tag).await?;
    writer
        .write_u32((body.len() + 4).try_into().unwrap())
        .await?;
    writer.write_all(body).await
}
async fn forward(
    mut client: TcpStream,
    mut server: TcpStream,
    dropped: Arc<AtomicUsize>,
) -> std::io::Result<()> {
    client.set_nodelay(true)?;
    server.set_nodelay(true)?;
    // SSL is disabled for this loopback test; the first frontend message is the
    // untagged startup packet. Authentication frames then use ordinary framing.
    let size = client.read_u32().await?;
    let mut startup = vec![0; size.checked_sub(4).ok_or(std::io::ErrorKind::InvalidData)? as usize];
    client.read_exact(&mut startup).await?;
    server.write_u32(size).await?;
    server.write_all(&startup).await?;
    let (mut cr, mut cw) = client.split();
    let (mut sr, mut sw) = server.split();
    let waiting_transaction = Arc::new(AtomicBool::new(false));
    let to_server = {
        let waiting_transaction = waiting_transaction.clone();
        async move {
            loop {
                let (tag, body) = read_frame(&mut cr).await?;
                if tag == b'P' {
                    // Target the actual waiting UPDATE, not an earlier node
                    // transaction or a result payload mentioning waiting_callback.
                    let text = String::from_utf8_lossy(&body);
                    if text.contains("update flow_runs")
                        && text.contains("scope_id = $5 and status = $6")
                        && text.contains("finished_at = null")
                    {
                        waiting_transaction.store(true, Ordering::SeqCst);
                    }
                }
                write_frame(&mut sw, tag, &body).await?;
            }
            #[allow(unreachable_code)]
            Ok::<(), std::io::Error>(())
        }
    };
    let to_client = async move {
        loop {
            let (tag, body) = read_frame(&mut sr).await?;
            if tag == b'C'
                && body == b"COMMIT\0"
                && waiting_transaction.load(Ordering::SeqCst)
                && dropped
                    .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return Ok::<(), std::io::Error>(());
            }
            write_frame(&mut cw, tag, &body).await?;
        }
    };
    tokio::select! { result = to_server => result, result = to_client => result }
}
