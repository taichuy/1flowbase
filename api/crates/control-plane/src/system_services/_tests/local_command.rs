use super::*;

#[test]
fn local_command_authorization_is_system_root_only() {
    let mut actor = ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "Root");
    assert!(authorize(&actor, "printf hello | cat").is_ok());
    assert!(matches!(
        authorize(&actor, "  "),
        Err(ControlPlaneError::InvalidInput(_))
    ));
    assert!(matches!(
        authorize(&actor, "echo\0secret"),
        Err(ControlPlaneError::InvalidInput(_))
    ));
    actor.is_root = false;
    assert!(matches!(
        authorize(&actor, "touch should-not-exist"),
        Err(ControlPlaneError::PermissionDenied(
            "local_command_root_required"
        ))
    ));
}
