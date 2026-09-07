//! AC-006 controlled Kernel/Registry contract, not production plugin activation.
//! Unlike a pre-captured Arc test, the old invocation is polled into a real Before
//! barrier before publication. No plugin hot-unloading guarantee is claimed.
use super::*;
use tokio::sync::Notify;

struct BarrierHook {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}
impl InterfaceBeforeHook<Input> for BarrierHook {
    fn before<'a>(
        &'a self,
        _: InterfaceHookContext,
        _: &'a mut Input,
    ) -> InterfaceBeforeHookFuture<'a> {
        Box::pin(async move {
            self.entered.notify_one();
            self.release.notified().await;
            Ok(())
        })
    }
}
struct Completion(Arc<Mutex<Vec<(String, String, InterfaceInvocationTerminal)>>>);
impl InterfaceCompletionHook for Completion {
    fn completed(
        &self,
        context: InterfaceHookContext,
        terminal: InterfaceInvocationTerminal,
    ) -> InterfaceCompletionHookFuture<'_> {
        Box::pin(async move {
            self.0.lock().unwrap().push((
                context.graph_fingerprint().as_str().into(),
                context.registry_fingerprint().as_str().into(),
                terminal,
            ));
        })
    }
}

#[tokio::test]
async fn root_1998_ac_006_in_flight_snapshot_keeps_plan_handler_and_completion_identity() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let old_hooks = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:old-in-flight").unwrap(),
    )
    .bind_before(
        PluginIdentity::new("fixture.barrier").unwrap(),
        Arc::new(BarrierHook {
            entered: entered.clone(),
            release: release.clone(),
        }),
    )
    .bind_completion(
        PluginIdentity::new("fixture.completion").unwrap(),
        Arc::new(Completion(seen.clone())),
    );
    let old = compile_snapshot_with_hooks(
        "graph:old-in-flight",
        1,
        false,
        Arc::new(old_hooks),
        vec![
            (
                1,
                hook_registration("fixture.barrier", InterfaceExtensionPoint::Before),
            ),
            (
                2,
                hook_registration("fixture.completion", InterfaceExtensionPoint::Completion),
            ),
        ],
    );
    let new_hooks = TypedInterfaceHookPlan::<Input, Output>::new(
        GraphFingerprint::new("graph:new-in-flight").unwrap(),
    )
    .bind_completion(
        PluginIdentity::new("fixture.completion").unwrap(),
        Arc::new(Completion(seen.clone())),
    );
    let new = compile_snapshot_with_hooks(
        "graph:new-in-flight",
        8,
        false,
        Arc::new(new_hooks),
        vec![(
            2,
            hook_registration("fixture.completion", InterfaceExtensionPoint::Completion),
        )],
    );
    let registry = DynamicInterfaceRegistry::new(old.clone());
    let kernel = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization { reject: false }),
        Arc::new(Admission { reject: false }),
    );
    let active = registry.snapshot();
    let (old_outcome, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            kernel.invoke::<Input, Output, TargetError>(active, envelope(actor(true))),
            async {
                entered.notified().await;
                assert!(
                    seen.lock().unwrap().is_empty(),
                    "old call is still before dispatch/completion"
                );
                registry.publish(new.clone());
                release.notify_one();
            }
        )
    })
    .await
    .expect("controlled barrier must finish");
    let old_outcome = old_outcome.unwrap();
    let new_outcome = kernel
        .invoke::<Input, Output, TargetError>(registry.snapshot(), envelope(actor(true)))
        .await
        .unwrap();
    assert_eq!(old_outcome.value(), &Output(3));
    assert_eq!(new_outcome.value(), &Output(10));
    for (outcome, snapshot) in [(&old_outcome, &old), (&new_outcome, &new)] {
        let receipt = outcome.receipt();
        let plan = snapshot.plan_for_interface(&interface_id()).unwrap();
        assert_eq!(receipt.interface_id(), Some(&interface_id()));
        assert_eq!(receipt.graph_fingerprint(), snapshot.graph_fingerprint());
        assert_eq!(receipt.registry_fingerprint(), snapshot.fingerprint());
        assert_eq!(
            receipt.resolved().unwrap().plan_fingerprint(),
            plan.fingerprint()
        );
        assert_eq!(receipt.terminal(), InterfaceInvocationTerminal::Completed);
        assert_eq!(
            receipt.attempt().unwrap().target(),
            &ExecutionTargetPin::BuiltIn {
                handler: plan.effective_handler().handler().clone(),
                target: plan.effective_handler().target().clone(),
            }
        );
    }
    assert_ne!(
        old_outcome.receipt().resolved().unwrap().plan_fingerprint(),
        new_outcome.receipt().resolved().unwrap().plan_fingerprint()
    );
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            (
                old.graph_fingerprint().as_str().into(),
                old.fingerprint().as_str().into(),
                InterfaceInvocationTerminal::Completed
            ),
            (
                new.graph_fingerprint().as_str().into(),
                new.fingerprint().as_str().into(),
                InterfaceInvocationTerminal::Completed
            )
        ]
    );
}
