//! Task-local authority supplied by the communication execution owner.
use domain::CommunicationHistoryScope;
use std::future::Future;
tokio::task_local! { static COMMUNICATION_SCOPE: CommunicationHistoryScope; }
pub async fn with_scope<T>(scope: CommunicationHistoryScope, future: impl Future<Output = T>) -> T {
    COMMUNICATION_SCOPE.scope(scope, future).await
}

pub fn has_scope() -> bool {
    COMMUNICATION_SCOPE.try_with(|_| ()).is_ok()
}
pub fn current_scope() -> Option<CommunicationHistoryScope> {
    COMMUNICATION_SCOPE.try_with(Clone::clone).ok()
}
