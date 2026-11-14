use super::*;

#[tokio::test]
async fn communication_cancellation_joins_leases_after_worker_unwind() {
    let registry = Arc::new(CommunicationActivity::default());
    let id = Uuid::now_v7();
    let session = registry.session(id, 1).unwrap();
    let lease = session.enter().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _state = session.state.lock().unwrap();
        panic!("fixture worker unwind");
    }));
    assert!(result.is_err());
    drop(lease);
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        registry.cancel_session(id),
    )
    .await
    .expect("poisoned state must not prevent /new cancellation from joining")
    .unwrap();
    assert!(session.enter().is_err());
}

#[tokio::test]
async fn communication_new_joins_execution_and_rejects_late_lease() {
    let registry = Arc::new(CommunicationActivity::default());
    let id = Uuid::now_v7();
    let session = registry.session(id, 1).unwrap();
    let lease = session.enter().unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (dropped, observed) = tokio::sync::oneshot::channel();
    struct ExecutionDrop(Option<tokio::sync::oneshot::Sender<()>>);
    impl Drop for ExecutionDrop {
        fn drop(&mut self) {
            let _ = self.0.take().unwrap().send(());
        }
    }
    let active = session.clone();
    let task = tokio::spawn(async move {
        let work = async {
            let _drop = ExecutionDrop(Some(dropped));
            let _ = started.send(());
            std::future::pending::<()>().await;
        };
        tokio::select! { biased; _ = active.cancelled() => {}, _ = work => {} }
        drop(lease);
    });
    // Ensure the provider future has been polled before requesting /new.
    ready.await.unwrap();
    registry.cancel_session(id).await.unwrap();
    observed.await.unwrap();
    task.await.unwrap();
    assert!(session.enter().is_err());
    assert!(registry.session(id, 1).is_err());
    let other = registry.session(Uuid::now_v7(), 1).unwrap();
    assert!(other.enter().is_ok());
}

#[derive(Clone)]
pub(crate) struct OwnedProviderFixture {
    entered: watch::Sender<bool>,
    cancel_entered: watch::Sender<bool>,
    release: watch::Sender<bool>,
    fail_join: bool,
}
impl OwnedProviderFixture {
    pub(crate) async fn wait_started(&self) {
        signalled(&self.entered).await;
    }
    pub(crate) fn new(fail_join: bool) -> Self {
        Self {
            entered: watch::channel(false).0,
            cancel_entered: watch::channel(false).0,
            release: watch::channel(false).0,
            fail_join,
        }
    }
}
async fn signalled(sender: &watch::Sender<bool>) {
    let mut receiver = sender.subscribe();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !*receiver.borrow_and_update() {
            receiver.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[async_trait::async_trait]
impl runtime_core::runtime_backend::RuntimeExecutionPort for OwnedProviderFixture {
    async fn activate_provider_distribution_rule(
        &self,
        _: runtime_core::runtime_backend::RuntimePackageActivation,
    ) -> std::result::Result<(), runtime_core::runtime_backend::RuntimeBackendError> {
        unreachable!()
    }
    async fn deactivate_provider_distribution_rule(
        &self,
        _: &str,
    ) -> std::result::Result<(), runtime_core::runtime_backend::RuntimeBackendError> {
        unreachable!()
    }
    async fn select_provider_distribution(
        &self,
        _: runtime_core::runtime_backend::RuntimeProviderDistributionRequest,
    ) -> std::result::Result<
        extension_contracts::ProviderDistributionSelectionReceipt,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        unreachable!()
    }
    async fn execute(
        &self,
        request: runtime_core::runtime_backend::RuntimeExecutionRequest,
    ) -> std::result::Result<
        runtime_core::runtime_backend::RuntimeExecutionOutcome,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        self.execute_stream(request, Default::default()).await
    }
    async fn execute_stream(
        &self,
        _: runtime_core::runtime_backend::RuntimeExecutionRequest,
        _: runtime_core::runtime_backend::RuntimeStreamSinks,
    ) -> std::result::Result<
        runtime_core::runtime_backend::RuntimeExecutionOutcome,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        self.entered.send_replace(true);
        std::future::pending().await
    }
    async fn cancel(
        &self,
        request: &runtime_core::runtime_backend::RuntimeRequestId,
    ) -> std::result::Result<
        runtime_core::runtime_backend::RuntimeCancelOutcome,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        self.cancel_entered.send_replace(true);
        if self.fail_join {
            return Err(
                runtime_core::runtime_backend::RuntimeBackendError::Execution {
                    target_id: request.to_string(),
                    message: "fixture physical join failed".into(),
                },
            );
        }
        let mut receiver = self.release.subscribe();
        while !*receiver.borrow_and_update() {
            receiver.changed().await.unwrap();
        }
        Ok(runtime_core::runtime_backend::RuntimeCancelOutcome::Cancelled)
    }
}

pub(crate) fn provider_request() -> runtime_core::runtime_backend::RuntimeExecutionRequest {
    runtime_core::runtime_backend::RuntimeExecutionRequest {
        request_id: runtime_core::runtime_backend::RuntimeRequestId::new(
            Uuid::now_v7().to_string(),
        )
        .unwrap(),
        target: runtime_core::runtime_backend::RuntimeTargetId::new("fixture").unwrap(),
        input: Default::default(),
        principal: None,
    }
}

#[tokio::test]
async fn communication_new_waits_for_runtime_request_join_after_outer_future_drop_including_legacy_owner(
) {
    for epoch in [0, 1] {
        let registry = Arc::new(CommunicationActivity::default());
        let id = Uuid::now_v7();
        let run_id = Uuid::now_v7();
        let session = registry.session(id, epoch).unwrap();
        let registration = registry
            .register_provider_run(run_id, session.clone())
            .unwrap();
        let provider = Arc::new(OwnedProviderFixture::new(false));
        let backend = orchestration_runtime::runtime_backend::OrchestrationRuntimeBackend::new(
            provider.clone(),
        );
        let caller_registry = registry.clone();
        let caller_backend = backend.clone();
        let caller = tokio::spawn(async move {
            caller_registry
                .execute_provider(
                    run_id,
                    &caller_backend,
                    provider_request(),
                    Default::default(),
                )
                .await
        });
        signalled(&provider.entered).await;
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        let canceller = registry.clone();
        let joined = tokio::spawn(async move { canceller.cancel_session(id).await });
        signalled(&provider.cancel_entered).await;
        assert!(
            !joined.is_finished(),
            "outer drop is not a provider join receipt"
        );
        assert!(session.enter().is_err());
        assert!(matches!(
            registry
                .execute_provider(run_id, &backend, provider_request(), Default::default())
                .await,
            Err(runtime_core::runtime_backend::RuntimeBackendError::Cancelled(_))
        ));
        provider.release.send_replace(true);
        joined.await.unwrap().unwrap();
        drop(registration);
    }
}

#[tokio::test]
async fn communication_new_propagates_unconfirmed_runtime_join_and_keeps_the_session_fenced() {
    let registry = Arc::new(CommunicationActivity::default());
    let id = Uuid::now_v7();
    let run_id = Uuid::now_v7();
    let session = registry.session(id, 1).unwrap();
    let registration = registry
        .register_provider_run(run_id, session.clone())
        .unwrap();
    let provider = Arc::new(OwnedProviderFixture::new(true));
    let backend =
        orchestration_runtime::runtime_backend::OrchestrationRuntimeBackend::new(provider.clone());
    let caller_registry = registry.clone();
    let caller = tokio::spawn(async move {
        caller_registry
            .execute_provider(run_id, &backend, provider_request(), Default::default())
            .await
    });
    signalled(&provider.entered).await;
    let error = registry.cancel_session(id).await.unwrap_err();
    assert!(error.to_string().contains("fixture physical join failed"));
    assert!(caller.await.unwrap().is_err());
    drop(registration);
    drop(session);
    assert!(
        registry.cancel_session(id).await.is_err(),
        "dropping the observer cannot erase join failure"
    );
    assert!(registry.session(id, 1).is_err());
}
