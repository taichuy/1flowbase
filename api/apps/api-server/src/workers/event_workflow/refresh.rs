//! Post-success invalidation of event subscription eligibility at the typed business boundary.
use crate::routes::console_interface::{ConsoleInterfaceFuture, ConsoleInterfacePort};
use interface_runtime::UserPrincipal;
use std::sync::Arc;
use tokio::sync::Notify;

/// The owning enum classifies relevant mutations before it is moved into the real port.
/// Authentication, authorization, output and errors remain entirely owned by that port.
pub(crate) fn after_success<I: Send + 'static, O: Send + 'static>(
    port: Arc<dyn ConsoleInterfacePort<I, O>>,
    changes: Arc<Notify>,
    affects_subscriptions: fn(&I) -> bool,
) -> Arc<dyn ConsoleInterfacePort<I, O>> {
    Arc::new(SubscriptionRefresh {
        port,
        changes,
        affects_subscriptions,
    })
}
struct SubscriptionRefresh<I, O> {
    port: Arc<dyn ConsoleInterfacePort<I, O>>,
    changes: Arc<Notify>,
    affects_subscriptions: fn(&I) -> bool,
}
impl<I: Send + 'static, O: Send + 'static> ConsoleInterfacePort<I, O>
    for SubscriptionRefresh<I, O>
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: I,
    ) -> ConsoleInterfaceFuture<'a, O> {
        let refresh = (self.affects_subscriptions)(&input);
        Box::pin(async move {
            let output = self.port.execute(principal, input).await?;
            if refresh {
                self.changes.notify_one();
            }
            Ok(output)
        })
    }
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/refresh.rs"]
mod tests;
