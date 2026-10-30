//! API-host adapter for the native one-shot execution module.
use control_plane::local_command::{LocalCommandExecutor, LocalCommandFuture, LocalCommandOutcome};
use std::sync::Arc;
pub(crate) mod process;
pub(crate) struct HostCommandExecutor;

impl LocalCommandOutcome for process::CompletedCommand {
    fn audit_payload(&self) -> serde_json::Value {
        serde_json::to_value(&self.metadata).expect("command metadata has no fallible serializer")
    }
}
impl LocalCommandExecutor for HostCommandExecutor {
    type Output = process::CompletedCommand;
    fn execute(&self, command: String) -> LocalCommandFuture<'_, Self::Output> {
        Box::pin(async move {
            struct CancelOnDrop(process::Cancellation);
            impl Drop for CancelOnDrop {
                fn drop(&mut self) {
                    self.0.cancel();
                }
            }
            let cancellation = process::Cancellation::default();
            let guard = CancelOnDrop(cancellation.clone());
            let result =
                tokio::task::spawn_blocking(move || process::execute(&command, &cancellation))
                    .await?;
            drop(guard);
            Ok(result?)
        })
    }
}
pub(crate) fn executor() -> Arc<HostCommandExecutor> {
    Arc::new(HostCommandExecutor)
}
