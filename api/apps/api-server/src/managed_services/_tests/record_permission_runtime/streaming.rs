use super::*;
use crate::managed_services::{
    interface::{ManagedServiceInput, ManagedServiceOutput},
    stream::{self, StreamEvent, StreamInput},
};
use crate::routes::console_interface::{self, ConsoleInterfaceTargetError};
use interface_runtime::{
    InterfaceInvocationFailure, InterfaceInvocationStage, InterfaceInvocationTerminal,
    InterfaceStreamTerminal, InterfaceStreamTerminalOutcome,
};
use runtime_core::runtime_backend::{RuntimeManagedDuplex, RuntimeManagedDuplexEvent};
use tokio::sync::{mpsc, watch};
struct Active<L> {
    lease: L,
    cancellation: watch::Receiver<bool>,
    _input: mpsc::Sender<Value>,
    _events: mpsc::Receiver<StreamEvent>,
    completion: tokio::task::JoinHandle<
        Result<
            InterfaceStreamTerminalOutcome<ManagedServiceOutput, ConsoleInterfaceTargetError>,
            InterfaceInvocationFailure,
        >,
    >,
}
async fn start(f: &Fixture, root: bool) -> Active<impl Send> {
    let state = f.state();
    let credential = if root { &f.root } else { &f.member };
    let generation = crate::managed_services::stream_ticket::generation(&state).unwrap();
    let (lease, cancellation) = stream::register_test_stream(f.installation, &generation).unwrap();
    let (input, input_receiver) = mpsc::channel(1);
    let (unused_events, _) = mpsc::channel::<RuntimeManagedDuplexEvent>(1);
    let server = console_interface::invoke_server_stream::<
        ManagedServiceInput,
        StreamEvent,
        ManagedServiceOutput,
    >(
        state.clone(),
        "http.record_fixture.stream.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state,
            headers: credential.headers(),
        },
        ManagedServiceInput {
            interface_id: "record_fixture.stream".into(),
            payload: json!({"path":{"record_id":f.alpha},"query":{},"body":{}}),
            stream: Some(StreamInput::Run {
                duplex: RuntimeManagedDuplex {
                    input: Arc::new(tokio::sync::Mutex::new(input_receiver)),
                    events: unused_events,
                },
                cancel: cancellation.clone(),
            }),
        },
    )
    .await
    .expect("current record open action must admit this real stream");
    let (mut events, completion) = server.into_parts();
    // As in production upgrade: this is the sole completion owner, independent of subscriber IO.
    let completion = tokio::spawn(async move { completion.complete().await });
    let StreamEvent(ready) = tokio::time::timeout(Duration::from_secs(10), events.recv())
        .await
        .expect("finite worker ready event deadline")
        .expect("stream ended before the worker ready event");
    assert_eq!(ready.event, json!({"ready":true,"record_id":f.alpha}));
    ready
        .consumed
        .send(())
        .expect("acknowledge actual worker credit");
    Active {
        lease,
        cancellation,
        _input: input,
        _events: events,
        completion,
    }
}
async fn completed<L>(active: Active<L>) {
    let Active {
        lease,
        mut cancellation,
        _input,
        _events,
        completion,
    } = active;
    if !*cancellation.borrow() {
        tokio::time::timeout(Duration::from_secs(5), cancellation.changed())
            .await
            .expect("policy save must actively cancel registered stream")
            .unwrap();
    }
    assert!(*cancellation.borrow());
    let outcome = tokio::time::timeout(Duration::from_secs(10), completion)
        .await
        .expect("canonical completion must remain owned after revocation")
        .unwrap()
        .unwrap();
    let (terminal, receipt) = outcome.into_parts();
    assert!(
        matches!(terminal, InterfaceStreamTerminal::Failed(_)),
        "revoked ephemeral stream must report its real terminal"
    );
    assert_eq!(receipt.terminal(), InterfaceInvocationTerminal::Failed);
    assert_eq!(
        receipt.interface_id().unwrap().as_str(),
        "record_fixture.stream"
    );
    assert!(receipt
        .stages()
        .any(|stage| stage == InterfaceInvocationStage::Executing));
    assert!(
        receipt.resolved().is_some(),
        "must finalize the real compiled canonical plan"
    );
    drop((lease, _input, _events));
}
async fn revalidate(f: &Fixture, root: bool) -> bool {
    let state = f.state();
    let headers = if root {
        f.root.headers()
    } else {
        f.member.headers()
    };
    let context = crate::middleware::require_session::require_session(&state, &headers)
        .await
        .unwrap();
    let session = context.cookie_session().unwrap().session_id.clone();
    let registration = state
        .console_surface_registry
        .managed_services()
        .iter()
        .find(|s| s.installation_id == f.installation)
        .unwrap();
    let operation = registration
        .declaration
        .operations
        .iter()
        .find(|o| o.interface_id == "record_fixture.stream")
        .unwrap();
    stream::revalidate_test_stream(
        &state,
        &headers,
        &session,
        operation,
        f.installation,
        &json!({"path":{"record_id":f.alpha},"query":{},"body":{}}),
    )
    .await
    .is_ok()
}
pub(super) async fn run(f: &Fixture) {
    assert!(
        !revalidate(f, false).await,
        "periodic authority owner must reject current empty record grant"
    );
    assert!(
        revalidate(f, true).await,
        "root periodic row evaluation remains exempt"
    );
    let (status, denied_ticket) = f
        .request(
            false,
            "POST",
            &format!("/records/{}/stream", f.alpha),
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied_ticket}");
    let (status, root_ticket) = f
        .request(
            true,
            "POST",
            &format!("/records/{}/stream", f.alpha),
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{root_ticket}");
    assert!(root_ticket["data"]["ticket"].is_string());
    f.policy(&f.alpha, "restricted", &["open"]).await;
    assert!(revalidate(f, false).await);
    let (status, ticket) = f
        .request(
            false,
            "POST",
            &format!("/records/{}/stream", f.alpha),
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ticket}");
    assert!(ticket["data"]["ticket"].is_string());
    let member = start(f, false).await;
    f.policy(&f.alpha, "restricted", &[]).await;
    completed(member).await;
    assert!(!revalidate(f, false).await);
    assert!(revalidate(f, true).await);
    // The existing installation-level invalidation also ends root's active stream. It is not
    // a root row denial: root can immediately start another stream under the same empty policy.
    let root = start(f, true).await;
    f.policy(&f.beta, "restricted", &[]).await;
    completed(root).await;
    let reopened = start(f, true).await;
    assert!(
        !*reopened.cancellation.borrow(),
        "new root stream must not inherit the past revocation epoch"
    );
    f.policy(&f.alpha, "restricted", &[]).await;
    completed(reopened).await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let traces =
                std::fs::read_to_string(f.worker.with_extension("trace")).unwrap_or_default();
            if traces
                .lines()
                .filter(|line| *line == "stream_cleanup")
                .count()
                >= 3
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("cancelled real SDK streams must release their cleanup owners");
}
