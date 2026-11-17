//! Real canonical plans, sessions, row ACL and SDK processes around the shared broker.
use super::*;
use crate::managed_services::{
    interface::{ManagedServiceInput, ManagedServiceOutput},
    stream::{self, StreamEvent, StreamInput},
};
use crate::routes::console_interface::{self, ConsoleInterfaceTargetError};
use interface_runtime::{
    InterfaceInvocationFailure, InterfaceStreamTerminal, InterfaceStreamTerminalOutcome,
};
use runtime_core::runtime_backend::RuntimeManagedDuplex;
use tokio::sync::mpsc;

struct Viewer {
    lease: stream::SocketLease,
    input: mpsc::Sender<Value>,
    events: mpsc::Receiver<StreamEvent>,
    completion: tokio::task::JoinHandle<
        Result<
            InterfaceStreamTerminalOutcome<ManagedServiceOutput, ConsoleInterfaceTargetError>,
            InterfaceInvocationFailure,
        >,
    >,
}
impl Viewer {
    async fn ready(&mut self) -> Value {
        let StreamEvent(event) = tokio::time::timeout(Duration::from_secs(10), self.events.recv())
            .await
            .expect("finite shared ready deadline")
            .expect("shared source ended without ready");
        assert_eq!(event.event["ready"], true);
        let _ = event.consumed.send(());
        event.event
    }
    async fn close(self) {
        self.input.send(json!({"type":"close"})).await.unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(10), self.completion)
            .await
            .expect("subscriber close must join its last producer")
            .unwrap()
            .unwrap();
        let InterfaceStreamTerminal::Completed(output) = outcome.into_parts().0 else {
            panic!("declared close_output must complete the subscriber")
        };
        assert_eq!(output.0, json!({"closed":true}));
        drop(self.lease);
    }
}
async fn start(f: &Fixture, root: bool) -> Viewer {
    let state = f.state();
    let headers = if root {
        f.root.headers()
    } else {
        f.member.headers()
    };
    let operation = state
        .console_surface_registry
        .managed_services()
        .iter()
        .find(|r| r.installation_id == f.installation)
        .unwrap()
        .declaration
        .operations
        .iter()
        .find(|op| op.interface_id == "record_fixture.shared_source")
        .unwrap()
        .clone();
    let payload = json!({"path":{"record_id":f.alpha},"query":{},"body":{"interval_ms":5000}});
    let (lease, cancel) =
        stream::register_test_stream(&state, &headers, f.installation, &operation, &payload)
            .await
            .unwrap();
    let (input, receiver) = mpsc::channel(1);
    let (unused, _) = mpsc::channel(1);
    let event_policy =
        Arc::new(crate::managed_services::event_projection::OperationPolicy::new(&operation));
    let invocation = console_interface::invoke_server_stream::<
        ManagedServiceInput,
        StreamEvent,
        ManagedServiceOutput,
    >(
        state.clone(),
        "http.record_fixture.shared_source.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        ManagedServiceInput::test_new(
            event_policy,
            operation.interface_id,
            payload,
            Some(StreamInput::Run {
                duplex: RuntimeManagedDuplex {
                    input: Arc::new(tokio::sync::Mutex::new(receiver)),
                    events: unused,
                },
                cancel,
            }),
        ),
    )
    .await
    .expect("real source canonical admission");
    let (events, completion) = invocation.into_parts();
    Viewer {
        lease,
        input,
        events,
        completion: tokio::spawn(async move { completion.complete().await }),
    }
}
fn trace(f: &Fixture) -> String {
    std::fs::read_to_string(f.worker.with_extension("trace")).unwrap_or_default()
}
fn count(f: &Fixture, prefix: &str) -> usize {
    trace(f)
        .lines()
        .filter(|line| line.starts_with(prefix))
        .count()
}
async fn run(f: &Fixture) {
    f.policy(&f.alpha, "restricted", &[]).await;
    let (status, denied) = f
        .request(
            false,
            "POST",
            &format!("/records/{}/shared_source", f.alpha),
            Some(json!({"interval_ms":5000})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(
        count(f, "shared_source_start:"),
        0,
        "denied admission must not open a producer"
    );
    f.policy(&f.alpha, "restricted", &["open"]).await;
    let mut member = start(f, false).await;
    let member_ready = member.ready().await;
    let mut root = start(f, true).await;
    assert_eq!(
        root.ready().await,
        member_ready,
        "two actual principals must share one raw source identity"
    );
    assert_eq!(count(f, "shared_source_start:"), 1);
    // MCP uses a real User API key, not a synthesized cookie or service principal.
    eprintln!("[shared-fixture] member cached snapshot first");
    let first = f
        .mcp(
            false,
            "shared_snapshot",
            json!({"record_id":f.alpha,"interval_ms":5000}),
        )
        .await;
    assert_eq!(mcp_success(&first), &member_ready);
    eprintln!("[shared-fixture] member cached snapshot second");
    let second = f
        .mcp(
            false,
            "shared_snapshot",
            json!({"record_id":f.alpha,"interval_ms":5000}),
        )
        .await;
    assert_eq!(mcp_success(&second), &member_ready);
    assert_eq!(
        count(f, "shared_snapshot:"),
        0,
        "active source cache must not execute unary SSH equivalent"
    );
    f.policy(&f.alpha, "restricted", &[]).await;
    eprintln!("[shared-fixture] member revoked; awaiting owner rotation");
    let revoked = tokio::time::timeout(Duration::from_secs(10), member.completion)
        .await
        .expect("revoked subscriber completion stays owned")
        .unwrap();
    assert!(revoked.is_err());
    drop((member.lease, member.input, member.events));
    let rotated = root.ready().await;
    assert_ne!(rotated["stream_id"], member_ready["stream_id"]);
    assert_eq!(count(f, "shared_source_start:"), 2);
    let denied = f
        .mcp(
            false,
            "shared_snapshot",
            json!({"record_id":f.alpha,"interval_ms":5000}),
        )
        .await;
    mcp_denied(&denied);
    assert_eq!(
        count(f, "shared_snapshot:"),
        0,
        "fresh cache must still enforce current row grants"
    );
    assert_eq!(
        mcp_success(
            &f.mcp(
                true,
                "shared_snapshot",
                json!({"record_id":f.alpha,"interval_ms":5000})
            )
            .await
        ),
        &rotated
    );
    root.close().await;
    eprintln!("[shared-fixture] last source closed; entering concurrent unary fallback");
    assert_eq!(count(f, "shared_source_cleanup:"), 2);
    let traces = trace(f);
    let cleanup = traces
        .find(&format!(
            "shared_source_cleanup:{}:{}",
            f.alpha,
            member_ready["stream_id"].as_str().unwrap()
        ))
        .unwrap();
    let restart = traces
        .find(&format!(
            "shared_source_start:{}:{}",
            f.alpha,
            rotated["stream_id"].as_str().unwrap()
        ))
        .unwrap();
    assert!(
        cleanup < restart,
        "old actor's raw producer must clean up before the new actor starts"
    );
    // A never-subscribed record exercises the original unary fallback and its cache.
    let (first, second) = tokio::join!(
        f.mcp(
            true,
            "shared_snapshot",
            json!({"record_id":f.beta,"interval_ms":5000})
        ),
        f.mcp(
            true,
            "shared_snapshot",
            json!({"record_id":f.beta,"interval_ms":5000})
        )
    );
    assert_eq!(mcp_success(&first), mcp_success(&second));
    assert_eq!(
        count(f, "shared_snapshot:"),
        1,
        "consecutive unary-only MCP calls must reuse the projected latest value"
    );
}

#[tokio::test]
#[ignore = "requires RECORD_PERMISSION_WORKER_FIXTURE built by record-permissions CI"]
async fn real_kernel_row_acl_two_principals_share_source_cache_and_rotate_owner() {
    use futures_util::FutureExt;
    let fixture = Fixture::new().await;
    let result = std::panic::AssertUnwindSafe(async {
        tokio::time::timeout(Duration::from_secs(120), run(&fixture))
            .await
            .expect("finite shared source acceptance deadline");
    })
    .catch_unwind()
    .await;
    if result.is_err() {
        let trace = trace(&fixture);
        let bounded = trace
            .chars()
            .rev()
            .take(8000)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        eprintln!("[shared-fixture] retained worker trace:\n{bounded}");
    }
    stream::revoke_installation(fixture.installation);
    fixture
        .host
        .drain()
        .await
        .expect("shared fixture Runtime Host drain");
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
