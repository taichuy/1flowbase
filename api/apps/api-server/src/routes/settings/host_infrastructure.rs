pub use api_console_settings::routes::host_infrastructure::*;

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
mod tests {
    use domain::{
        ActorContext, ConsoleOperationId, ConsoleOperationPolicy, ConsolePolicyGroup,
        RoleConsoleGroupPolicy, RoleConsolePolicy,
    };
    use uuid::Uuid;

    use super::has_registered_simple_operations;

    fn policy(feature_id: &str, operation_ids: &[&str]) -> RoleConsolePolicy {
        let group = ConsolePolicyGroup::settings_feature(feature_id)
            .expect("compiled settings feature id must be valid");
        RoleConsolePolicy::new(
            Uuid::now_v7(),
            vec![RoleConsoleGroupPolicy::custom(
                group,
                operation_ids
                    .iter()
                    .map(|operation_id| {
                        ConsoleOperationPolicy::simple(
                            ConsoleOperationId::try_from(*operation_id)
                                .expect("compiled operation id must be valid"),
                            true,
                        )
                    })
                    .collect(),
            )],
        )
    }

    #[test]
    fn ac_007_ac_011_host_and_mcp_capabilities_ignore_legacy_grants() {
        let settings = crate::app_state::compile_core_settings_feature_registry()
            .expect("core settings feature registry must compile");
        let registry = crate::app_state::compile_core_console_operation_registry(&settings)
            .expect("core console operation registry must compile");
        let policy_only = ActorContext::scoped(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "member",
            Vec::<String>::new(),
        );
        let legacy_only = ActorContext::scoped(
            Uuid::now_v7(),
            Uuid::now_v7(),
            "member",
            [
                access_control::SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_PERMISSION.to_string(),
                access_control::SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION.to_string(),
            ],
        );
        let policies = vec![
            policy(
                "system.memory-observation",
                &["host_infrastructure.cache.reveal"],
            ),
            policy("system.mcp-management", &["mcp.instances.create"]),
        ];

        assert!(has_registered_simple_operations(
            &registry,
            &policy_only,
            &policies,
            &["host_infrastructure.cache.reveal"]
        ));
        assert!(has_registered_simple_operations(
            &registry,
            &policy_only,
            &policies,
            &["mcp.instances.create"]
        ));
        assert!(!has_registered_simple_operations(
            &registry,
            &legacy_only,
            &[],
            &["host_infrastructure.cache.reveal"]
        ));
        assert!(!has_registered_simple_operations(
            &registry,
            &legacy_only,
            &[],
            &["mcp.instances.create"]
        ));
    }
}
