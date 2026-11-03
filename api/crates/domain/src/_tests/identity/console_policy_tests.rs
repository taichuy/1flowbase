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

#[test]
fn canonical_plugin_group_preserves_full_custom_and_disabled_authorization() {
    let group = ConsolePolicyGroup::plugin("acme.calendar").unwrap();
    assert_eq!(group.kind().as_str(), "plugin");
    assert_eq!(
        crate::ConsolePolicyGroupKind::parse("plugin"),
        Some(group.kind())
    );
    let allowed = ConsoleOperationId::try_from("acme.calendar.list").unwrap();
    let denied = ConsoleOperationId::try_from("acme.calendar.delete").unwrap();
    let custom = RoleConsoleGroupPolicy::custom(
        group.clone(),
        vec![
            ConsoleOperationPolicy::simple(allowed.clone(), true),
            ConsoleOperationPolicy::simple(denied.clone(), false),
        ],
    );
    let check = |selection, op: &ConsoleOperationId| {
        effective_console_simple_operation(
            &[RoleConsolePolicy::new(Uuid::nil(), vec![selection])],
            &group,
            op,
        )
    };
    assert!(check(RoleConsoleGroupPolicy::full(group.clone()), &denied));
    assert!(check(custom.clone(), &allowed));
    assert!(!check(custom.clone(), &denied));
    assert!(!check(custom.with_enabled(false), &allowed));
    let foreign =
        RoleConsoleGroupPolicy::full(ConsolePolicyGroup::plugin("other.calendar").unwrap());
    assert!(!check(foreign, &allowed));
    let legacy = RoleConsoleGroupPolicy::full(
        ConsolePolicyGroup::settings_feature("acme.calendar").unwrap(),
    );
    assert!(!check(legacy, &allowed));
}
