use super::*;

pub(super) fn to_console_policy_group_kind_body(
    kind: domain::ConsolePolicyGroupKind,
) -> ConsolePolicyGroupKindBody {
    match kind {
        domain::ConsolePolicyGroupKind::SettingsFeature => {
            ConsolePolicyGroupKindBody::SettingsFeature
        }
        domain::ConsolePolicyGroupKind::Other => ConsolePolicyGroupKindBody::Other,
    }
}

pub(super) fn to_console_policy_strategy_body(
    strategy: domain::ConsolePolicyStrategy,
) -> ConsolePolicyStrategyBody {
    match strategy {
        domain::ConsolePolicyStrategy::Full => ConsolePolicyStrategyBody::Full,
        domain::ConsolePolicyStrategy::Custom => ConsolePolicyStrategyBody::Custom,
    }
}

pub(super) fn to_console_policy_row_scope_body(
    scope: domain::ConsoleOperationRowScope,
) -> ConsolePolicyRowScopeBody {
    match scope {
        domain::ConsoleOperationRowScope::Disabled => ConsolePolicyRowScopeBody::Disabled,
        domain::ConsoleOperationRowScope::Own => ConsolePolicyRowScopeBody::Own,
        domain::ConsoleOperationRowScope::ScopeAll => ConsolePolicyRowScopeBody::ScopeAll,
    }
}

pub(crate) fn to_console_policy_catalog_response(
    catalog: control_plane::role::ConsolePolicyCatalog,
) -> ConsolePolicyCatalogResponse {
    ConsolePolicyCatalogResponse {
        schema_version: catalog.schema_version,
        locale: catalog.locale,
        settings_order_revision: catalog.settings_order_revision,
        group_strategy_options: catalog
            .group_strategy_options
            .into_iter()
            .map(|option| ConsolePolicyCatalogOptionResponse {
                value: option.value,
                label: option.label,
                description: option.description,
            })
            .collect(),
        groups: catalog
            .groups
            .into_iter()
            .map(|group| ConsolePolicyCatalogGroupResponse {
                kind: to_console_policy_group_kind_body(group.kind),
                group_id: group.group_id,
                label: group.label,
                description: group.description,
                operations: group
                    .operations
                    .into_iter()
                    .map(|operation| ConsolePolicyCatalogOperationResponse {
                        operation_id: operation.operation_id,
                        summary: operation.summary,
                        description: operation.description,
                        order: operation.order,
                        route: ConsolePolicyCatalogRouteResponse {
                            method: operation.route.method,
                            path: operation.route.path,
                        },
                        full_profile: match operation.full_profile {
                            control_plane::role::ConsolePolicyCatalogFullProfile::Simple {
                                enabled,
                            } => {
                                ConsolePolicyCatalogOperationFullProfileResponse::Simple { enabled }
                            }
                            control_plane::role::ConsolePolicyCatalogFullProfile::Row { scope } => {
                                ConsolePolicyCatalogOperationFullProfileResponse::Row {
                                    scope: to_console_policy_row_scope_body(scope),
                                }
                            }
                        },
                        allowed_row_scopes: operation
                            .allowed_row_scopes
                            .into_iter()
                            .map(|option| ConsolePolicyCatalogOptionResponse {
                                value: option.value,
                                label: option.label,
                                description: option.description,
                            })
                            .collect(),
                        authorization: match operation.authorization {
                            control_plane::role::ConsolePolicyAuthorization::Simple => {
                                ConsolePolicyOperationAuthorizationResponse::Simple
                            }
                            control_plane::role::ConsolePolicyAuthorization::ResourceAction {
                                resource_code,
                                action_code,
                            } => ConsolePolicyOperationAuthorizationResponse::ResourceAction {
                                resource_code,
                                action_code,
                            },
                        },
                    })
                    .collect(),
            })
            .collect(),
        resources: catalog
            .resources
            .into_iter()
            .map(|resource| ConsolePolicyCatalogResourceResponse {
                resource_code: resource.resource_code,
                label: resource.label,
                description: resource.description,
                actions: resource
                    .actions
                    .into_iter()
                    .map(|action| ConsolePolicyCatalogResourceActionResponse {
                        action_code: action.action_code,
                        label: action.label,
                        description: action.description,
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub(crate) async fn resolve_console_policy_catalog_display_with(
    store: &storage_durable_postgres::MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
    catalog: &mut control_plane::role::ConsolePolicyCatalog,
) -> Result<(), ApiError> {
    catalog.locale = locale.as_str().to_string();
    for option in &mut catalog.group_strategy_options {
        option.label = super::super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &option.label,
        )
        .await?;
        option.description =
            super::super::super::core_console_i18n::resolve_core_console_display_with(
                store,
                bootstrap_workspace_id,
                locale,
                &option.description,
            )
            .await?;
    }
    for group in &mut catalog.groups {
        group.label = super::super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &group.label,
        )
        .await?;
        group.description =
            super::super::super::core_console_i18n::resolve_core_console_display_with(
                store,
                bootstrap_workspace_id,
                locale,
                &group.description,
            )
            .await?;
        for operation in &mut group.operations {
            operation.summary = crate::app_state::resolve_request_text_with(
                store,
                bootstrap_workspace_id,
                locale,
                &operation.summary,
            )
            .await?;
            operation.description = crate::app_state::resolve_request_text_with(
                store,
                bootstrap_workspace_id,
                locale,
                &operation.description,
            )
            .await?;
            for option in &mut operation.allowed_row_scopes {
                option.label =
                    super::super::super::core_console_i18n::resolve_core_console_display_with(
                        store,
                        bootstrap_workspace_id,
                        locale,
                        &option.label,
                    )
                    .await?;
                option.description =
                    super::super::super::core_console_i18n::resolve_core_console_display_with(
                        store,
                        bootstrap_workspace_id,
                        locale,
                        &option.description,
                    )
                    .await?;
            }
        }
    }
    for resource in &mut catalog.resources {
        resource.label = super::super::super::core_console_i18n::resolve_core_console_display_with(
            store,
            bootstrap_workspace_id,
            locale,
            &resource.label,
        )
        .await?;
        resource.description =
            super::super::super::core_console_i18n::resolve_core_console_display_with(
                store,
                bootstrap_workspace_id,
                locale,
                &resource.description,
            )
            .await?;
        for action in &mut resource.actions {
            action.label =
                super::super::super::core_console_i18n::resolve_core_console_display_with(
                    store,
                    bootstrap_workspace_id,
                    locale,
                    &action.label,
                )
                .await?;
            action.description =
                super::super::super::core_console_i18n::resolve_core_console_display_with(
                    store,
                    bootstrap_workspace_id,
                    locale,
                    &action.description,
                )
                .await?;
        }
    }
    Ok(())
}

pub(crate) fn to_role_console_policy_response(
    role_code: String,
    policy: domain::RoleConsolePolicy,
) -> RoleConsolePolicyResponse {
    RoleConsolePolicyResponse {
        role_code,
        groups: policy
            .groups()
            .iter()
            .map(|group| RoleConsolePolicyGroupResponse {
                kind: to_console_policy_group_kind_body(group.group().kind()),
                group_id: group.group().group_id().as_str().to_string(),
                enabled: group.enabled(),
                strategy: to_console_policy_strategy_body(group.strategy()),
                operations: group
                    .operations()
                    .iter()
                    .map(|operation| match operation {
                        domain::ConsoleOperationPolicy::Simple {
                            operation_id,
                            enabled,
                        } => RoleConsolePolicyOperationResponse::Simple {
                            operation_id: operation_id.as_str().to_string(),
                            enabled: *enabled,
                        },
                        domain::ConsoleOperationPolicy::Row {
                            operation_id,
                            scope,
                        } => RoleConsolePolicyOperationResponse::Row {
                            operation_id: operation_id.as_str().to_string(),
                            scope: to_console_policy_row_scope_body(*scope),
                        },
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub(crate) fn to_console_policy_group_input(
    group: ConsoleRoleConsolePolicyGroupBody,
) -> ConsolePolicyGroupInput {
    ConsolePolicyGroupInput {
        kind: group.kind.as_str().to_string(),
        group_id: group.group_id,
        enabled: group.enabled,
        strategy: group.strategy.as_str().to_string(),
        operations: group
            .operations
            .into_iter()
            .map(|operation| match operation {
                ConsoleRoleConsolePolicyOperationBody::Simple {
                    operation_id,
                    enabled,
                } => ConsolePolicyOperationInput::Simple {
                    operation_id,
                    enabled,
                },
                ConsoleRoleConsolePolicyOperationBody::Row {
                    operation_id,
                    scope,
                } => ConsolePolicyOperationInput::Row {
                    operation_id,
                    scope: scope.as_str().to_string(),
                },
            })
            .collect(),
    }
}
