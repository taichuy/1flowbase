use super::*;
#[test]
fn local_command_validation_does_not_require_a_root_actor() {
    assert!(validate_command("printf hello | cat").is_ok());
    assert!(matches!(
        validate_command("  "),
        Err(ControlPlaneError::InvalidInput(_))
    ));
    assert!(matches!(
        validate_command("echo\0secret"),
        Err(ControlPlaneError::InvalidInput(_))
    ));
}
