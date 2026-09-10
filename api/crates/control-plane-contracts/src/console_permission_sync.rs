use std::collections::BTreeSet;

use domain::{
    ConsolePolicyGroup, ConsolePolicyStrategy, RoleConsoleGroupPolicy, RoleConsolePolicy,
};

use crate::CompiledConsolePolicyGroup;

/// Pure projection of first-seen catalog additions onto an opted-in role. Existing omissions,
/// explicit denials and full strategies are not rewritten. Only genuinely new groups may open.
/// Returns additive grant rows, never a replacement for the stored role policy.
pub fn project_console_permission_additions(
    policy: &RoleConsolePolicy,
    additions: &[CompiledConsolePolicyGroup],
    new_groups: &BTreeSet<ConsolePolicyGroup>,
) -> Vec<RoleConsoleGroupPolicy> {
    let existing_ids = policy
        .groups()
        .iter()
        .flat_map(|group| {
            group
                .operations()
                .iter()
                .map(|operation| operation.operation_id())
        })
        .collect::<BTreeSet<_>>();
    additions
        .iter()
        .filter_map(|addition| {
            let existing = policy
                .groups()
                .iter()
                .find(|group| group.group() == &addition.group);
            match existing {
                Some(group)
                    if !group.enabled() || group.strategy() == ConsolePolicyStrategy::Full =>
                {
                    return None
                }
                None if !new_groups.contains(&addition.group) => return None,
                _ => {}
            }
            let new_operations = addition
                .full_operations
                .iter()
                .filter(|operation| !existing_ids.contains(operation.operation_id()))
                .cloned()
                .collect::<Vec<_>>();
            if new_operations.is_empty() {
                return None;
            }
            Some(RoleConsoleGroupPolicy::custom(
                addition.group.clone(),
                new_operations,
            ))
        })
        .collect()
}
