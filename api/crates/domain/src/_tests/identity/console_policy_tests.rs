use uuid::Uuid;

use crate::{
    effective_console_simple_operation, ConsoleOperationId, ConsoleOperationPolicy,
    ConsolePolicyGroup, ConsolePolicyStrategy, RoleConsoleGroupPolicy, RoleConsolePolicy,
};

// Issue #1485 AC-002: disabling a group suppresses grants without deleting its saved strategy.
#[test]
fn ac_002_disabled_group_retains_custom_operations_for_reactivation() {
    let group = ConsolePolicyGroup::settings_feature("system.applications")
        .expect("settings feature group must be valid");
    let operation_id = ConsoleOperationId::try_from("application_list")
        .expect("OpenAPI operation id must be valid");
    let disabled = RoleConsoleGroupPolicy::new(
        group.clone(),
        false,
        ConsolePolicyStrategy::Custom,
        vec![ConsoleOperationPolicy::simple(operation_id.clone(), true)],
    );
    let disabled_policy = RoleConsolePolicy::new(Uuid::now_v7(), vec![disabled.clone()]);

    assert!(!disabled.enabled());
    assert_eq!(disabled.strategy(), ConsolePolicyStrategy::Custom);
    assert_eq!(disabled.operations().len(), 1);
    assert!(!effective_console_simple_operation(
        &[disabled_policy],
        &group,
        &operation_id
    ));

    let enabled = disabled.with_enabled(true);
    let enabled_policy = RoleConsolePolicy::new(Uuid::now_v7(), vec![enabled]);
    assert!(effective_console_simple_operation(
        &[enabled_policy],
        &group,
        &operation_id
    ));
}
