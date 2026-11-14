use tokio::sync::watch;
use uuid::Uuid;
#[derive(Clone)]
pub struct OwnedProviderFixture {
    pub(super) entered: watch::Sender<bool>,
    pub(super) cancel_entered: watch::Sender<bool>,
    pub(super) release: watch::Sender<bool>,
    fail_join: bool,
}
impl OwnedProviderFixture {
    pub async fn wait_started(&self) {
        signalled(&self.entered).await;
    }
    pub fn new(fail_join: bool) -> Self {
        Self {
            entered: watch::channel(false).0,
            cancel_entered: watch::channel(false).0,
            release: watch::channel(false).0,
            fail_join,
        }
    }
}
pub(super) async fn signalled(sender: &watch::Sender<bool>) {
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

pub fn provider_request() -> runtime_core::runtime_backend::RuntimeExecutionRequest {
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
