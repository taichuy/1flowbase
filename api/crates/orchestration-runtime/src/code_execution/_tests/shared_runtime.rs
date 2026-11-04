use super::*;
use crate::compiled_plan::CodeIsolationProfile;
use serde_json::json;
use std::{sync::Arc, time::Instant};

fn runtime(source: &str) -> CompiledCodeRuntime {
    CompiledCodeRuntime {
        language: "javascript".into(),
        source: Some(source.into()),
        source_ref: None,
        entrypoint: "main".into(),
        imports: Vec::new(),
        dependencies: Vec::new(),
        isolation_profile: CodeIsolationProfile::quickjs_default(),
    }
}

async fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "execution lifecycle observation timed out"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

#[tokio::test]
async fn original_node_retains_nested_stringify_omission_and_conversion() {
    let runtime = runtime(
        r#"function main(){return {omit:undefined,nested:{omit:undefined,n:NaN,p:Infinity},
        array:[undefined,,NaN],date:new Date('2020-01-01T00:00:00Z'),
        custom:{toJSON(){return {n:1}}}}}"#,
    );
    let output = QuickJsCodeInvoker::default()
        .invoke_code_node(&runtime, json!({}), json!({}))
        .await
        .unwrap();
    assert_eq!(
        output.output_payload,
        json!({"nested":{"n":null,"p":null},"array":[null,null,null],
        "date":"2020-01-01T00:00:00.000Z","custom":{"n":1}})
    );
}

#[tokio::test]
async fn original_node_serialization_exceptions_and_logs_keep_error_shape() {
    for body in [
        "return {n:1n}",
        "const x={};x.x=x;return x",
        "return {toJSON(){throw Error('private')}}",
    ] {
        let runtime = runtime(&format!("function main(){{console.warn('before');{body}}}"));
        let error = QuickJsCodeInvoker::default()
            .invoke_code_node(&runtime, json!({}), json!({}))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "runtime_error: code execution failed");
        let logs = &error
            .downcast_ref::<sync_js_runtime::ExecutionError>()
            .unwrap()
            .console_logs;
        assert_eq!(logs[0].message, "before");
    }
}

#[tokio::test]
async fn dropped_started_invocation_stops_exits_and_joins_before_its_deadline() {
    let probe = Arc::new(ExecutionProbe::default());
    let invoker = QuickJsCodeInvoker {
        timeout_override: Some(Duration::from_secs(30)),
        lifecycle_probe: Some(probe.clone()),
    };
    let runtime = runtime("function main(){while(true){}}");
    let future = tokio::spawn(async move {
        invoker
            .invoke_code_node(&runtime, json!({}), json!({}))
            .await
    });
    wait_for(|| {
        probe
            .cancel
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(CancellationToken::has_started)
    })
    .await;
    let cancel = probe.cancel.lock().unwrap().clone().unwrap();
    assert!(!cancel.has_exited());
    future.abort();
    assert!(future.await.unwrap_err().is_cancelled());
    wait_for(|| probe.events.lock().unwrap().last() == Some(&"joined")).await;
    assert!(cancel.is_cancelled());
    assert!(cancel.has_exited());
    assert_eq!(
        *probe.events.lock().unwrap(),
        vec!["stop", "js_exited", "joined"]
    );
}

#[test]
fn dropping_queued_invocation_stops_before_blocking_start_and_still_joins() {
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    executor.block_on(async {
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            let _ = started_tx.send(());
            release_rx.recv().unwrap();
        });
        started_rx.await.unwrap();
        let probe = Arc::new(ExecutionProbe::default());
        let invoker = QuickJsCodeInvoker {
            timeout_override: Some(Duration::from_secs(30)),
            lifecycle_probe: Some(probe.clone()),
        };
        let compiled = runtime("function main(){while(true){}}");
        let future = tokio::spawn(async move {
            invoker
                .invoke_code_node(&compiled, json!({}), json!({}))
                .await
        });
        wait_for(|| probe.cancel.lock().unwrap().is_some()).await;
        let cancel = probe.cancel.lock().unwrap().clone().unwrap();
        assert!(!cancel.has_started());
        assert!(!cancel.has_exited());
        future.abort();
        assert!(future.await.unwrap_err().is_cancelled());
        assert!(cancel.is_cancelled());
        release_tx.send(()).unwrap();
        blocker.await.unwrap();
        wait_for(|| probe.events.lock().unwrap().last() == Some(&"joined")).await;
        assert!(!cancel.has_started());
        assert_eq!(
            *probe.events.lock().unwrap(),
            vec!["stop", "not_started", "joined"]
        );
    });
}

#[tokio::test]
async fn completed_timeout_has_a_join_receipt_and_next_call_recovers() {
    let probe = Arc::new(ExecutionProbe::default());
    let invoker = QuickJsCodeInvoker {
        timeout_override: Some(Duration::from_millis(200)),
        lifecycle_probe: Some(probe.clone()),
    };
    let failure = invoker
        .invoke_code_node(
            &runtime("function main(){while(true){}}"),
            json!({}),
            json!({}),
        )
        .await
        .unwrap_err();
    assert_eq!(failure.to_string(), "timeout: code execution timed out");
    assert_eq!(*probe.events.lock().unwrap(), vec!["js_exited", "joined"]);
    let output = QuickJsCodeInvoker::default()
        .invoke_code_node(
            &runtime("function main(){return {ok:true}}"),
            json!({}),
            json!({}),
        )
        .await
        .unwrap();
    assert_eq!(output.output_payload, json!({"ok":true}));
}
