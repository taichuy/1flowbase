//! R7 uses the real HTTP ticket + WS route and process-owned production shared broker.
//! No openStream shim, fabricated events, runtime, HostCall, or composition root.
use super::{
    browser::BrowserConfig,
    fixture::{Fixture, Preserved},
    loopback_ssh::LoopbackSsh,
};
use anyhow::{Context, Result, ensure};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, client::IntoClientRequest},
};

const PATH: &str = "/api/console/managed-services/compute-monitor/monitor/stream";
type Socket = WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;
fn now_ms() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}
struct Server {
    base: String,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<std::io::Result<()>>,
}
impl Server {
    async fn start(f: &Fixture) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (stop, stopped) = oneshot::channel();
        let ingress = f.ingress.clone(); // SAME production publisher ingress, not app reconstruction.
        let task = tokio::spawn(async move {
            axum::serve(listener, ingress)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
        });
        Ok(Self {
            base: format!("http://{address}"),
            stop: Some(stop),
            task,
        })
    }
    async fn shutdown(&mut self) -> Result<()> {
        // Release production socket leases/producers on error as well as success.
        crate::managed_services::stream::revoke_all();
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let result = tokio::time::timeout(Duration::from_secs(10), &mut self.task).await;
        if result.is_err() {
            self.task.abort();
            let _ = (&mut self.task).await;
        }
        result
            .context("R7 ephemeral ingress shutdown timed out")??
            .context("R7 ingress failed")
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        crate::managed_services::stream::revoke_all();
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}
async fn open(f: &Fixture, server: &Server, records: Value, member: bool) -> Result<Socket> {
    let (cookie, csrf) = f.stream_auth(member);
    let response = reqwest::Client::new()
        .post(format!("{}{PATH}", server.base))
        .timeout(Duration::from_secs(10))
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .header("origin", &server.base)
        .json(&json!({"records":records}))
        .send()
        .await?;
    let status = response.status();
    // Bounded HTTP response body; never echo a session/ticket in diagnostics.
    ensure!(
        response.content_length().unwrap_or(0) <= 64 * 1024,
        "oversize aggregate admission response"
    );
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 64 * 1024,
            "oversize aggregate admission response"
        );
        bytes.extend_from_slice(&chunk);
    }
    let wire: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        status == reqwest::StatusCode::OK,
        "authenticated aggregate admission failed: {status}"
    );
    let ticket = wire["data"]["ticket"]
        .as_str()
        .context("actual aggregate ticket missing")?;
    let mut request =
        format!("{}{PATH}?ticket={ticket}", server.base.replacen("http://", "ws://", 1)).into_client_request()?;
    request.headers_mut().insert("cookie", cookie.parse()?);
    request.headers_mut().insert("origin", server.base.parse()?);
    let (socket, response) = tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::connect_async(request))
        .await
        .context("aggregate WS upgrade timed out")??;
    ensure!(response.status().as_u16() == 101, "actual aggregate WS did not upgrade");
    Ok(socket)
}
async fn event(socket: &mut Socket) -> Result<Value> {
    loop {
        let message = socket
            .next()
            .await
            .context("aggregate WS closed before expected event")??;
        match message {
            Message::Text(text) => {
                ensure!(text.len() <= 64 * 1024, "oversize production aggregate frame");
                let frame: Value = serde_json::from_str(&text)?;
                ensure!(
                    frame["type"] == "event",
                    "aggregate error/terminal instead of sample: {}",
                    frame["type"]
                );
                let id = frame["id"].as_u64().context("aggregate event credit ID missing")?;
                socket
                    .send(Message::Text(json!({"type":"ack","id":id}).to_string().into()))
                    .await?;
                return Ok(frame["event"].clone());
            }
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
            Message::Pong(_) => {}
            _ => anyhow::bail!("aggregate WS ended before sample/denial evidence"),
        }
    }
}
async fn set(socket: &mut Socket, records: Value) -> Result<()> {
    socket
        .send(Message::Text(
            json!({"type":"input","input":{"type":"set","records":records}})
                .to_string()
                .into(),
        ))
        .await?;
    Ok(())
}
// This helper checks observations, not broker internals. Multiple non-warming, distinct
// samples are exactly what ui/streams.ts -> appendSample -> trend histories consume.
async fn samples(
    socket: &mut Socket,
    p: &Preserved,
    ssh: &LoopbackSsh,
    wanted: &[(&str, u64)],
    since: i64,
    strict_after: i64,
) -> Result<BTreeMap<String, Vec<i64>>> {
    tokio::time::timeout(Duration::from_secs(35), async {
        let mut times: BTreeMap<String, Vec<i64>> = wanted.iter().map(|(id, _)| ((*id).into(), Vec::new())).collect();
        loop {
            let value = event(socket).await?;
            p.no_secrets(&value)?;
            ensure!(
                value["type"] == "source",
                "authorized source emitted a subscription error: {value}"
            );
            let id = value["record_id"]
                .as_str()
                .context("aggregate record identity missing")?;
            let (visible, hidden) = p.host_ids();
            ensure!(
                id == visible || id == hidden,
                "aggregate emitted an unrelated provider record"
            );
            if value["event"]["type"] == "status" {
                // Normal production handshake states consumed by ui/streams.ts; they never
                // count as chart samples, and retry/error states cannot pass this fixture.
                ensure!(
                    value["event"]["host_id"] == id
                        && matches!(value["event"]["state"].as_str(), Some("queued" | "connecting"))
                        && value["event"]["error_code"].is_null(),
                    "unexpected or cross-host source status: {value}"
                );
                continue;
            }
            ensure!(
                value["event"]["type"] == "sample",
                "real SSH source emitted an error instead of a sample: {value}"
            );
            let sample = &value["event"]["snapshot"];
            ensure!(
                sample["host_id"] == id,
                "aggregate envelope/sample cross-host contamination"
            );
            let at = sample["captured_at_unix_ms"].as_i64().context("sample time missing")?;
            if !times.contains_key(id) {
                // Only in-flight pre-selection samples may drain after set; a detached
                // deselected producer emitting new samples is never accepted.
                ensure!(at <= strict_after, "deselected host continued emitting fresh samples");
                continue;
            }
            if at < since || sample["warming_up"] == true {
                continue;
            }
            ssh.assert_sample(sample, id, since)?;
            let observed = times.get_mut(id).unwrap();
            ensure!(
                observed.last().is_none_or(|previous| at > *previous),
                "aggregate repeated/out-of-order sample cannot build real trend history"
            );
            observed.push(at);
            if times.values().all(|values| values.len() >= 4) {
                break;
            }
        }
        for (id, interval) in wanted {
            let values = &times[*id];
            let mut deltas: Vec<_> = values.windows(2).map(|pair| pair[1] - pair[0]).collect();
            deltas.sort_unstable();
            let median = deltas[deltas.len() / 2];
            ensure!(
                median >= (*interval * 7 / 10) as i64 && median <= (*interval * 5 / 2 + 200) as i64,
                "host {id} cadence {median}ms does not match requested {interval}ms"
            );
        }
        Ok(times)
    })
    .await
    .context("real aggregate multi-host/cadence/selection samples timed out")?
}
async fn connections(ssh: &LoopbackSsh, expected: usize) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if ssh.established_connections()? == expected {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .context("real aggregate producer SSH socket ownership did not converge")?
}
async fn idle(ssh: &LoopbackSsh) -> Result<()> {
    connections(ssh, 0).await?;
    let probes = ssh.sessions()?;
    // More than two of the slowest requested 1500ms cadences, while runtime stays alive.
    tokio::time::sleep(Duration::from_millis(3300)).await;
    ensure!(
        ssh.established_connections()? == 0 && ssh.sessions()? == probes,
        "cancelled aggregate left SSH sockets or detached sampling probes alive"
    );
    Ok(())
}
async fn close(mut socket: Socket) -> Result<()> {
    // Match HostAPIclient.close(): request transport close, then release the client socket.
    // The strong cancellation evidence is actual producer/socket/probe quiescence in idle(),
    // not a fabricated business terminal or an assumption about WS close-handshake timing.
    tokio::time::timeout(Duration::from_secs(5), socket.close(None))
        .await
        .context("aggregate transport close timed out")??;
    drop(socket);
    Ok(())
}
pub async fn accept(f: &Fixture, p: &Preserved, ssh: &LoopbackSsh, browser: &BrowserConfig) -> Result<()> {
    let mut server = Server::start(f).await?;
    let result = exercise(f, p, ssh, browser, &server).await;
    let cleanup = server.shutdown().await;
    // Preserve business assertion error while always attempting server cleanup first.
    result?;
    cleanup
}
async fn exercise(
    f: &Fixture,
    p: &Preserved,
    ssh: &LoopbackSsh,
    browser: &BrowserConfig,
    server: &Server,
) -> Result<()> {
    let (visible, hidden) = p.host_ids();
    // Admission is the genuine authenticated route, not an unauthenticated stream shim.
    let anonymous = reqwest::Client::new()
        .post(format!("{}{PATH}", server.base))
        .timeout(Duration::from_secs(10))
        .header("origin", &server.base)
        .json(&json!({"records":[]}))
        .send()
        .await?;
    ensure!(
        anonymous.status() == reqwest::StatusCode::UNAUTHORIZED,
        "aggregate ticket endpoint allowed anonymous admission"
    );
    drop(anonymous);
    let before = ssh.sessions()?;
    connections(ssh, 0).await?;
    let started = now_ms();
    let records = json!([{"record_id":visible,"interval_ms":500},{"record_id":hidden,"interval_ms":1500}]);
    let mut socket = open(f, server, records, false).await?;
    let observed = samples(&mut socket, p, ssh, &[(visible, 500), (hidden, 1500)], started, started).await?;
    ensure!(
        ssh.sessions()? >= before + 8,
        "aggregate did not perform genuine multi-host SSH probes"
    );
    connections(ssh, 2).await?;
    eprintln!(
        "R7_AGGREGATE_MULTI_HOST: actual authenticated ticket/WS + event ACK -> production shared broker -> genuine archive runtime/credential HostCall -> pinned SSH; trend timestamps={observed:?}"
    );

    // A real same-socket selection update removes one source and changes the other's cadence.
    let changed = now_ms();
    set(&mut socket, json!([{"record_id":visible,"interval_ms":1250}])).await?;
    samples(&mut socket, p, ssh, &[(visible, 1250)], changed + 1500, changed + 1500).await?;
    connections(ssh, 1).await?;
    let changed = now_ms();
    set(
        &mut socket,
        json!([{"record_id":visible,"interval_ms":1250},{"record_id":hidden,"interval_ms":500}]),
    )
    .await?;
    samples(
        &mut socket,
        p,
        ssh,
        &[(visible, 1250), (hidden, 500)],
        changed + 1500,
        changed + 1500,
    )
    .await?;
    connections(ssh, 2).await?;

    // Authorize real member operations, not record open. Both per-record admissions must
    // deny even while Root's authorized producers/cache are live (no sharing ACL bypass).
    f.allow_member_aggregate().await?;
    let mut denied = open(
        f,
        server,
        json!([{"record_id":visible,"interval_ms":500},{"record_id":hidden,"interval_ms":500}]),
        true,
    )
    .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut ids = BTreeSet::new();
        while ids.len() < 2 {
            let value = event(&mut denied).await?;
            p.no_secrets(&value)?;
            ensure!(
                value["type"] == "subscription_error" && value["error_code"] == "managed_service_subscription_denied",
                "member view-only/hidden record obtained an aggregate source through Root sharing/cache"
            );
            let id = value["record_id"].as_str().context("denial record missing")?;
            ensure!(id == visible || id == hidden, "denial returned an unrelated host");
            ids.insert(id.to_owned());
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("actual member aggregate open denial timed out")??;
    close(denied).await?;
    connections(ssh, 2).await?;

    // The same live runtime must quiesce with empty selection; no shutdown-as-cleanup proof.
    set(&mut socket, json!([])).await?;
    idle(ssh).await?;
    // Reopen on the same subscription, rejecting stale/deselected-host frames in samples(),
    // and prove transport cancellation also joins its source.
    let restarted = now_ms();
    set(&mut socket, json!([{"record_id":visible,"interval_ms":500}])).await?;
    samples(&mut socket, p, ssh, &[(visible, 500)], restarted, restarted).await?;
    connections(ssh, 1).await?;
    close(socket).await?;
    idle(ssh).await?;
    eprintln!(
        "R7_AGGREGATE_SELECTION_CANCEL: same-socket cadence/remove/readd/empty-selection; member open denied; WS close/cancel; zero owned SSH sockets and no probes for >2 slow cadences before runtime shutdown"
    );

    // Use the actual SSH create contract: authenticated connection verification and a
    // pinned ephemeral key are required even for a disabled disposable management host.
    // The browser receives only its public ID, never a private key or credential value.
    let (status, created) = f
        .post(
            "/api/console/managed-services/ssh/hosts",
            json!({
                "name":"R7 disposable management host", "address":"127.0.0.1",
                "port":ssh.port, "username":ssh.username, "auth_type":"private_key",
                "credential":{"private_key":ssh.private_key()?},
                "expected_fingerprint":ssh.fingerprint, "enabled":false,
                "timeout_ms":30000, "max_output_bytes":1048576, "jump_hosts":[]
            }),
            false,
        )
        .await?;
    ensure!(
        status == axum::http::StatusCode::OK,
        "create disposable management host through real SSH API failed: {status}"
    );
    let management_host_id = created.get("data").unwrap_or(&created)["host_id"]
        .as_str()
        .context("created management host ID missing")?;
    let browser_result = async {
        let rename = json!({
            "name":"R7 authority-verified management host", "address":"127.0.0.1",
            "port":ssh.port, "username":ssh.username, "auth_type":"private_key", "enabled":false,
            "timeout_ms":30000, "max_output_bytes":1048576, "jump_hosts":[]
        });
        super::management_authority::accept(f, &server.base, management_host_id, rename, &browser.output_dir)
            .await?;
        browser.run(json!({
            "api_base_url":server.base,
            "visible_host_id":visible,
            "hidden_host_id":hidden,
            "management_host_id":management_host_id,
            "expected_hostname":std::fs::read_to_string("/proc/sys/kernel/hostname")?.trim(),
            "output_dir":browser.output_dir,
        })).await
    }
    .await;
    // Always attempt deletion after the subprocess, including browser startup/assertion
    // failures. Retain the original failure; normal deletion uses the same real SSH API.
    let deletion = f
        .delete(&format!("/api/console/managed-services/ssh/hosts/{management_host_id}"))
        .await;
    browser_result?;
    let (status, _) = deletion?;
    ensure!(
        status == axum::http::StatusCode::OK,
        "delete disposable management host/credential failed"
    );
    // Browser also has to release its subscriptions/SSH producers before revocation tail.
    idle(ssh).await?;
    Ok(())
}
