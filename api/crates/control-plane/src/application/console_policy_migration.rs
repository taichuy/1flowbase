use access_control::{
    APPLICATIONS_CREATE_OPERATION_ID, APPLICATIONS_DELETE_OPERATION_ID,
    APPLICATIONS_UPDATE_OPERATION_ID, APPLICATIONS_VIEW_OPERATION_ID,
    SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID, SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION,
};
use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
};

use crate::role::console_policy_migration::{
    CompiledConsolePolicyCatalog, CompiledConsolePolicyGroup, LegacyConsoleGrantMapping,
};

fn operation_id(value: &str) -> ConsoleOperationId {
    ConsoleOperationId::try_from(value)
        .expect("compiled applications console operation id must be valid")
}

fn simple(value: &str) -> ConsoleOperationPolicy {
    ConsoleOperationPolicy::simple(operation_id(value), true)
}

fn row(value: &str, scope: ConsoleOperationRowScope) -> ConsoleOperationPolicy {
    ConsoleOperationPolicy::row(operation_id(value), scope)
}

pub fn applications_console_policy_catalog() -> CompiledConsolePolicyCatalog {
    CompiledConsolePolicyCatalog {
        complete: true,
        groups: vec![CompiledConsolePolicyGroup {
            group: ConsolePolicyGroup::settings_feature(SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID)
                .expect("compiled applications settings feature id must be valid"),
            full_operations: vec![
                simple(SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION),
                simple(APPLICATIONS_CREATE_OPERATION_ID),
                row(
                    APPLICATIONS_VIEW_OPERATION_ID,
                    ConsoleOperationRowScope::ScopeAll,
                ),
                row(
                    APPLICATIONS_UPDATE_OPERATION_ID,
                    ConsoleOperationRowScope::ScopeAll,
                ),
                row(
                    APPLICATIONS_DELETE_OPERATION_ID,
                    ConsoleOperationRowScope::ScopeAll,
                ),
            ],
        }],
    }
}

pub fn applications_legacy_console_grant_mappings() -> Vec<LegacyConsoleGrantMapping> {
    let mut mappings = vec![LegacyConsoleGrantMapping {
        legacy_grant: SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION.to_string(),
        operations: vec![simple(SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION)],
    }];
    for legacy_grant in ["application.create.own", "application.create.all"] {
        mappings.push(LegacyConsoleGrantMapping {
            legacy_grant: legacy_grant.to_string(),
            operations: vec![simple(APPLICATIONS_CREATE_OPERATION_ID)],
        });
    }
    for (legacy_action, operation_id) in [
        ("view", APPLICATIONS_VIEW_OPERATION_ID),
        ("edit", APPLICATIONS_UPDATE_OPERATION_ID),
        ("delete", APPLICATIONS_DELETE_OPERATION_ID),
    ] {
        for (legacy_scope, scope) in [
            ("own", ConsoleOperationRowScope::Own),
            ("all", ConsoleOperationRowScope::ScopeAll),
        ] {
            mappings.push(LegacyConsoleGrantMapping {
                legacy_grant: format!("application.{legacy_action}.{legacy_scope}"),
                operations: vec![row(operation_id, scope)],
            });
        }
    }
    mappings
}
