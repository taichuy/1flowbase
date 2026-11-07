use super::*;
use crate::managed_services::{
    interface::{ManagedServiceInput, ManagedServiceOutput},
    stream::{self, StreamEvent, StreamInput},
};
use crate::routes::console_interface::{self, ConsoleInterfaceTargetError};
use interface_runtime::{
    InterfaceExtensionPoint, InterfaceInvocationError, InterfaceInvocationFailure,
    InterfaceInvocationStage, InterfaceInvocationTerminal, InterfaceObserverStatus,
    InterfaceStreamTerminalOutcome,
};
use runtime_core::runtime_backend::{RuntimeManagedDuplex, RuntimeManagedDuplexEvent};
use tokio::sync::{mpsc, watch};
struct Active<L> {
    stream_id: String,
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
async fn start(f: &Fixture, root: bool, record: &str) -> Active<stream::SocketLease> {
    let state = f.state();
    let credential = if root { &f.root } else { &f.member };
    let headers = credential.headers();
    let payload = json!({"path":{"record_id":record},"query":{},"body":{}});
    let operation = state
        .console_surface_registry
        .managed_services()
        .iter()
        .find(|service| service.installation_id == f.installation)
        .unwrap()
        .declaration
        .operations
        .iter()
        .find(|op| op.interface_id == "record_fixture.stream")
        .unwrap();
    let (lease, cancellation) =
        stream::register_test_stream(&state, &headers, f.installation, operation, &payload)
            .await
            .unwrap();
    let (input, input_receiver) = mpsc::channel(1);
    let (unused_events, _) = mpsc::channel::<RuntimeManagedDuplexEvent>(1);
    let server = console_interface::invoke_server_stream::<
        ManagedServiceInput,
        StreamEvent,
        ManagedServiceOutput,
    >(
        state.clone(),
        "http.record_fixture.stream.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        ManagedServiceInput {
            interface_id: "record_fixture.stream".into(),
            payload,
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
    let stream_id = ready.event["stream_id"].as_str().unwrap().to_owned();
    assert_eq!(
        ready.event,
        json!({"ready":true,"record_id":record,"stream_id":stream_id})
    );
    ready
        .consumed
        .send(())
        .expect("acknowledge actual worker credit");
    Active {
        stream_id,
        lease,
        cancellation,
        _input: input,
        _events: events,
        completion,
    }
}
fn assert_active<L>(stream: &Active<L>) {
    assert!(
        !*stream.cancellation.borrow(),
        "authorized stream must keep its cancellation lease"
    );
    assert!(
        !stream.completion.is_finished(),
        "authorized stream must continue executing"
    );
}
async fn completed<L>(active: Active<L>) {
    let Active {
        lease,
        mut cancellation,
        _input,
        _events,
        completion,
        ..
    } = active;
    if !*cancellation.borrow() {
        tokio::time::timeout(Duration::from_secs(5), cancellation.changed())
            .await
            .expect("policy save must actively cancel registered stream")
            .unwrap();
    }
    assert!(*cancellation.borrow());
    // The Kernel returns a target failure as Err only after Failure/Completion observers
    // finalize its receipt; a failed stream is not an Ok terminal outcome.
    let failure = tokio::time::timeout(Duration::from_secs(10), completion)
        .await
        .expect("canonical completion must remain owned after revocation")
        .expect("canonical completion task must not panic")
        .err()
        .expect("revoked ephemeral stream must fail canonical completion");
    let receipt = failure.receipt();
    assert_eq!(receipt.terminal(), InterfaceInvocationTerminal::Failed);
    assert_eq!(
        receipt.interface_id().unwrap().as_str(),
        "record_fixture.stream"
    );
    assert_eq!(
        receipt
            .stages()
            .filter(|stage| *stage == InterfaceInvocationStage::Executing)
            .count(),
        1
    );
    assert_eq!(
        receipt
            .resolved()
            .expect("must finalize the real compiled canonical plan")
            .binding_id()
            .as_str(),
        "http.record_fixture.stream.v1"
    );
    let observers = receipt.observer_records();
    assert_eq!(observers.len(), 2, "{observers:?}");
    for (observer, (point, plugin)) in observers.iter().zip([
        (
            InterfaceExtensionPoint::Failure,
            "interface-runtime.managed.failure",
        ),
        (
            InterfaceExtensionPoint::Completion,
            "interface-runtime.managed.completion",
        ),
    ]) {
        assert_eq!(observer.point(), point);
        assert_eq!(observer.plugin().as_str(), plugin);
        assert_eq!(observer.status(), InterfaceObserverStatus::Executed);
        assert_eq!(observer.reason(), None);
    }
    let target = match failure.into_error() {
        InterfaceInvocationError::TargetFailed(target) => target,
        error => panic!("revoked managed target must report its exact failure: {error:?}"),
    };
    assert_eq!(target.classification(), "managed_service_stream");
    let ConsoleInterfaceTargetError(api_error) = target
        .into_source::<ConsoleInterfaceTargetError>()
        .expect("managed stream must preserve its typed target error");
    // The current adapter maps its cancellation Conflict to this safe execution code.
    assert!(
        matches!(
            api_error
                .0
                .downcast_ref::<control_plane::errors::ControlPlaneError>(),
            Some(
                control_plane::errors::ControlPlaneError::UpstreamUnavailable(
                    "managed_service_stream_execution"
                )
            )
        ),
        "unexpected managed stream failure: {:?}",
        api_error
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
    f.policy(&f.beta, "restricted", &["open"]).await;
    let member = start(f, false, &f.alpha).await;
    let root = start(f, true, &f.alpha).await;
    let neighbor = start(f, false, &f.beta).await;
    let mut expected_streams = vec![
        member.stream_id.clone(),
        root.stream_id.clone(),
        neighbor.stream_id.clone(),
    ];
    let epoch = stream::epoch(f.installation);
    // Saving a still-allowing grant must not interrupt that user's existing terminal.
    f.policy(&f.alpha, "restricted", &["open"]).await;
    assert_active(&member);
    assert_active(&root);
    assert_active(&neighbor);
    assert_eq!(
        stream::epoch(f.installation),
        epoch,
        "a record save must not revoke installation tickets"
    );
    f.policy(&f.alpha, "restricted", &[]).await;
    assert_active(&root);
    assert_active(&neighbor);
    completed(member).await;
    assert!(!revalidate(f, false).await);
    assert!(revalidate(f, true).await);
    let reopened = start(f, true, &f.alpha).await;
    expected_streams.push(reopened.stream_id.clone());
    f.policy(&f.beta, "restricted", &["open"]).await;
    f.policy(&f.alpha, "restricted", &[]).await;
    for cancellation in [
        &root.cancellation,
        &reopened.cancellation,
        &neighbor.cancellation,
    ] {
        assert!(
            !*cancellation.borrow(),
            "root and still-authorized neighbors remain active"
        );
    }
    assert_active(&root);
    assert_active(&reopened);
    assert_active(&neighbor);
    // Generation/disable still uses the pre-existing installation-wide cancellation owner.
    stream::revoke_installation(f.installation);
    completed(root).await;
    completed(reopened).await;
    completed(neighbor).await;
    f.policy(&f.beta, "restricted", &[]).await;
    let expected: std::collections::BTreeSet<_> = expected_streams.into_iter().collect();
    assert_eq!(
        expected.len(),
        4,
        "each ready stream must have a unique identity"
    );
    let trace_path = f.worker.with_extension("trace");
    let bounded_trace = || {
        let trace = std::fs::read_to_string(&trace_path)
            .unwrap_or_else(|error| format!("trace read failed: {error}"));
        trace
            .chars()
            .rev()
            .take(4000)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>()
    };
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let traces = std::fs::read_to_string(&trace_path).unwrap_or_default();
            let starts = traces
                .lines()
                .filter_map(|line| line.strip_prefix("stream_start:"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let start_ids: std::collections::BTreeSet<_> = starts.iter().cloned().collect();
            let cleanups = traces
                .lines()
                .filter_map(|line| line.strip_prefix("stream_cleanup:"))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let cleanup_ids: std::collections::BTreeSet<_> = cleanups.iter().cloned().collect();
            assert_eq!(
                starts.len(), start_ids.len(),
                "duplicate stream start; trace_tail={}", bounded_trace()
            );
            assert_eq!(
                start_ids, expected,
                "ready identities must match actual worker starts; path={}; trace_tail={}", trace_path.display(), bounded_trace()
            );
            assert_eq!(
                cleanups.len(), cleanup_ids.len(),
                "duplicate stream cleanup; trace_tail={}", bounded_trace()
            );
            assert!(
                cleanup_ids.is_subset(&expected),
                "unexpected cleanup identity; expected={expected:?}; trace_tail={}", bounded_trace()
            );
            if cleanup_ids == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|error| panic!(
        "cancelled real SDK streams must each release their cleanup owner once: {error:?}; expected={expected:?}; path={}; trace_tail={}", trace_path.display(), bounded_trace()
    ));
}
