use super::*;
use crate::routes::console_interface::ConsoleInterfaceTargetError;
use control_plane::errors::ControlPlaneError;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

enum Command {
    Read,
    Commit,
    Denied,
}
struct Owner {
    started: Arc<Notify>,
    finish: Arc<Notify>,
    expected_user: Uuid,
}
impl ConsoleInterfacePort<Command, &'static str> for Owner {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: Command,
    ) -> ConsoleInterfaceFuture<'a, &'static str> {
        Box::pin(async move {
            assert_eq!(principal.actor().user_id, self.expected_user);
            match input {
                Command::Read => Ok("original read output"),
                Command::Denied => Err(ConsoleInterfaceTargetError(
                    ControlPlaneError::PermissionDenied("original_denial").into(),
                )),
                Command::Commit => {
                    self.started.notify_one();
                    self.finish.notified().await;
                    Ok("original committed output")
                }
            }
        })
    }
}
fn relevant(command: &Command) -> bool {
    matches!(command, Command::Commit | Command::Denied)
}
#[tokio::test]
async fn observer_only_refreshes_after_the_real_owner_commits_and_preserves_output() {
    let started = Arc::new(Notify::new());
    let finish = Arc::new(Notify::new());
    let changes = Arc::new(Notify::new());
    let user = Uuid::now_v7();
    let principal =
        UserPrincipal::server_delegation(domain::ActorContext::root(user, Uuid::now_v7(), "root"));
    let port = after_success(
        Arc::new(Owner {
            started: started.clone(),
            finish: finish.clone(),
            expected_user: user,
        }),
        changes.clone(),
        relevant,
    );
    let invocation = tokio::spawn(async move { port.execute(&principal, Command::Commit).await });
    timeout(Duration::from_secs(1), started.notified())
        .await
        .unwrap();
    assert!(timeout(Duration::from_millis(5), changes.notified())
        .await
        .is_err());
    finish.notify_one();
    assert_eq!(
        invocation
            .await
            .unwrap()
            .unwrap_or_else(|_| panic!("real owner succeeded")),
        "original committed output"
    );
    timeout(Duration::from_secs(1), changes.notified())
        .await
        .unwrap();
}
#[tokio::test]
async fn reads_and_rejected_mutations_never_refresh_and_denial_is_unchanged() {
    let changes = Arc::new(Notify::new());
    let user = Uuid::now_v7();
    let principal =
        UserPrincipal::server_delegation(domain::ActorContext::root(user, Uuid::now_v7(), "root"));
    let port = after_success(
        Arc::new(Owner {
            started: Arc::new(Notify::new()),
            finish: Arc::new(Notify::new()),
            expected_user: user,
        }),
        changes.clone(),
        relevant,
    );
    assert_eq!(
        port.execute(&principal, Command::Read)
            .await
            .unwrap_or_else(|_| panic!("read must succeed")),
        "original read output"
    );
    assert!(timeout(Duration::from_millis(5), changes.notified())
        .await
        .is_err());
    let Err(error) = port.execute(&principal, Command::Denied).await else {
        panic!("owner must reject unauthorized mutation");
    };
    assert!(matches!(
        error.0 .0.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("original_denial"))
    ));
    assert!(timeout(Duration::from_millis(5), changes.notified())
        .await
        .is_err());
}
