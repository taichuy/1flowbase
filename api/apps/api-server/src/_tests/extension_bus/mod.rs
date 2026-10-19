mod boot_snapshot_tests;
mod frontend_contribution_tests;
mod input_assembly_tests;
mod interface_operation_tests;

#[path = "managed/managed_activation_tests.rs"]
mod managed_activation_tests;
#[path = "managed/managed_event_authority_tests.rs"]
mod managed_event_authority_tests;

#[path = "managed/managed_event_effect_tests.rs"]
mod managed_event_effect_tests;

#[path = "managed/managed_snapshot_tests.rs"]
mod managed_snapshot_tests;

#[path = "managed/managed_candidate_tests.rs"]
mod managed_candidate_tests;

#[path = "managed/managed_governance_tests.rs"]
mod managed_governance_tests;

#[path = "managed/managed_resource_tests.rs"]
mod managed_resource_tests;

#[path = "managed/managed_reinstall_tests.rs"]
mod managed_reinstall_tests;

mod plugin_settings_page_tests;

#[path = "events/generic_event_history_tests.rs"]
mod generic_event_history_tests;
#[path = "events/generic_event_tests.rs"]
mod generic_event_tests;
mod notifications;
