//! Root #2007 AC-003/004: frozen host context survives cancellation and isolated observer reports.
use super::*;
use crate::InterfaceObserverStatus;

struct Frozen {
    value: u64,
}
struct Observer {
    reports: bool,
    seen: Arc<Mutex<Vec<u64>>>,
}
impl InterfaceCompletionHook for Observer {
    fn completed(
        &self,
        context: InterfaceHookContext,
        _: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            self.seen
                .lock()
                .unwrap()
                .push(context.extension_context::<Frozen>().unwrap().value);
            if self.reports {
                context.report_observer_failure();
            }
        })
    }
}

#[tokio::test]
async fn root_2007_ac_003_004_managed_context_completion_and_reporter_are_isolated() {
    for cancelled in [false, true] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut hooks = TypedInterfaceHookPlan::<Input, Output>::new(
            GraphFingerprint::new("graph:managed-context").unwrap(),
        );
        let mut registrations = Vec::new();
        for (order, name, reports) in [(10, "observer.quick", false), (20, "observer.failed", true)]
        {
            hooks = hooks.bind_completion(
                PluginIdentity::new(name).unwrap(),
                Arc::new(Observer {
                    reports,
                    seen: seen.clone(),
                }),
            );
            registrations.push((
                order,
                hook_registration(name, InterfaceExtensionPoint::Completion),
            ));
        }
        let snapshot = compile_snapshot_with_hooks(
            "graph:managed-context",
            3,
            false,
            Arc::new(hooks),
            registrations,
        );
        let cancellation = InvocationCancellation::new();
        if cancelled {
            cancellation.cancel();
        }
        let envelope = InvocationEnvelope::with_principal_and_controls(
            InvocationLineage::root(InvocationId::now_v7()),
            binding().binding_id().clone(),
            InterfaceProtocol::Http,
            AuthenticationAdapterReference::new("test.authn").unwrap(),
            AuthenticationActivationIdentity::new("test.authn.activation.v1").unwrap(),
            UserPrincipal::server_delegation(actor(true)),
            InvocationControls::new(None, cancellation, None),
            Input(2),
        )
        .freeze_extension_context(Arc::new(Frozen { value: 7 }))
        .unwrap();
        let kernel = InterfaceInvocationKernel::with_target_admission(
            Arc::new(Authorization { reject: false }),
            Arc::new(Admission { reject: false }),
        );
        let result = kernel
            .invoke::<Input, Output, TargetError>(snapshot, envelope)
            .await;
        let receipt = if cancelled {
            result.unwrap_err().receipt().clone()
        } else {
            let result = result.unwrap();
            assert_eq!(result.value(), &Output(5));
            result.receipt().clone()
        };
        assert_eq!(*seen.lock().unwrap(), [7, 7]);
        let records = receipt.observer_records();
        assert_eq!(
            records
                .iter()
                .find(|r| r.plugin().as_str() == "observer.failed")
                .unwrap()
                .status(),
            InterfaceObserverStatus::Failed
        );
        assert_eq!(
            records
                .iter()
                .find(|r| r.plugin().as_str() == "observer.quick")
                .unwrap()
                .status(),
            InterfaceObserverStatus::Executed
        );
    }
    assert!(envelope(actor(true))
        .freeze_extension_context(Arc::new(Frozen { value: 1 }))
        .unwrap()
        .freeze_extension_context(Arc::new(Frozen { value: 2 }))
        .is_err());
}

struct LateReport {
    report: Arc<tokio::sync::Notify>,
    done: Arc<tokio::sync::Notify>,
}
impl InterfaceCompletionHook for LateReport {
    fn completed(
        &self,
        context: InterfaceHookContext,
        _: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        let report = self.report.clone();
        let done = self.done.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                report.notified().await;
                context.report_observer_failure();
                done.notify_one();
            });
        })
    }
}
struct FollowingObserver {
    report: Arc<tokio::sync::Notify>,
    done: Arc<tokio::sync::Notify>,
}
impl InterfaceCompletionHook for FollowingObserver {
    fn completed(
        &self,
        _: InterfaceHookContext,
        _: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            self.report.notify_one();
            self.done.notified().await;
        })
    }
}
#[tokio::test]
async fn root_2007_ac_003_004_managed_late_report_cannot_mark_another_observer_failed() {
    let report = Arc::new(tokio::sync::Notify::new());
    let done = Arc::new(tokio::sync::Notify::new());
    let hooks = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:late-report").unwrap(),
    )
    .bind_completion(
        PluginIdentity::new("observer.following").unwrap(),
        Arc::new(FollowingObserver {
            report: report.clone(),
            done: done.clone(),
        }),
    )
    .bind_completion(
        PluginIdentity::new("observer.late").unwrap(),
        Arc::new(LateReport { report, done }),
    );
    let snapshot = compile_snapshot_with_hooks(
        "graph:late-report",
        3,
        false,
        Arc::new(hooks),
        vec![
            (
                10,
                hook_registration("observer.following", InterfaceExtensionPoint::Completion),
            ),
            (
                20,
                hook_registration("observer.late", InterfaceExtensionPoint::Completion),
            ),
        ],
    );
    let outcome = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(Admission { reject: false }),
    )
    .invoke::<Input, Output, TargetError>(snapshot, envelope(actor(true)))
    .await
    .unwrap();
    assert_eq!(outcome.value(), &Output(5));
    assert_eq!(outcome.receipt().observer_records().len(), 2);
    assert!(outcome
        .receipt()
        .observer_records()
        .iter()
        .all(|record| record.status() == InterfaceObserverStatus::Executed));
}
