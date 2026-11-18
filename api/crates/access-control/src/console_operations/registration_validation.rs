use super::*;

pub(super) fn validate_settings_feature_operation_coverage(
    settings_features: &SettingsFeatureRegistry,
    registrations: &[ConsoleOperationRegistration],
) -> Result<(), ConsoleOperationRegistryError> {
    for feature in &settings_features.inventory().features {
        let expected_routes = feature
            .api_routes
            .iter()
            .map(|route| (route.method.to_ascii_uppercase(), route_shape(&route.path)))
            .collect::<BTreeSet<_>>();
        let mut claims = BTreeSet::new();
        for registration in registrations {
            let owns_feature = match &registration.policy_group {
                ConsolePolicyGroup::SettingsFeature(id) => id == &feature.feature_id,
                ConsolePolicyGroup::Plugin(id) => {
                    feature.owner.kind == SettingsFeatureOwnerKind::ManagedService
                        && feature.owner == registration.owner
                        && id == &feature.owner.owner_id
                        && id == &registration.owner.owner_id
                }
                ConsolePolicyGroup::Other(_) => false,
            };
            if owns_feature {
                claims.extend(
                    registration
                        .routes
                        .iter()
                        .map(|route| (route.method.to_ascii_uppercase(), route_shape(&route.path)))
                        .filter(|route| expected_routes.contains(route)),
                );
            }
        }
        if let Some((method, path)) = expected_routes.difference(&claims).next() {
            return Err(ConsoleOperationRegistryError::new(format!(
                "settings feature {} route has no configurable operation: {method} {path}",
                feature.feature_id
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_operation(
    registration: &ConsoleOperationRegistration,
    settings_feature_ids: &BTreeSet<&str>,
    resources: &BTreeMap<String, ResourceAccessRegistration>,
) -> Result<(), ConsoleOperationRegistryError> {
    validate_non_empty(&registration.operation_id, "operation_id")?;
    validate_owner(&registration.owner, "operation owner")?;
    validate_host_extension_namespace(
        &registration.owner,
        &registration.operation_id,
        "operation_id",
    )?;
    if registration.lifecycle == SettingsFeatureLifecycle::Inactive {
        return Err(ConsoleOperationRegistryError::new(format!(
            "inactive operation {} cannot own console routes",
            registration.operation_id
        )));
    }
    if registration.routes.is_empty() {
        return Err(ConsoleOperationRegistryError::new(format!(
            "operation {} must own at least one console route",
            registration.operation_id
        )));
    }
    if !matches!(
        registration.authorization,
        ConsoleAuthorization::Authenticated
    ) && registration.routes.len() != 1
    {
        return Err(ConsoleOperationRegistryError::new(format!(
            "configurable operation {} must bind exactly one console route",
            registration.operation_id
        )));
    }
    for route in &registration.routes {
        validate_route(route)?;
    }
    if let ConsolePolicyGroup::SettingsFeature(feature_id) = &registration.policy_group {
        if !settings_feature_ids.contains(feature_id.as_str()) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "operation {} references unknown settings feature {feature_id}",
                registration.operation_id
            )));
        }
    }
    if let ConsolePolicyGroup::Plugin(group_id) = &registration.policy_group {
        validate_non_empty(group_id, "Plugin policy group_id")?;
        if registration.owner.kind != SettingsFeatureOwnerKind::ManagedService
            || group_id != &registration.owner.owner_id
        {
            return Err(ConsoleOperationRegistryError::new(
                "Plugin policy group must equal its ManagedService owner_id",
            ));
        }
    }
    if registration.owner.kind == SettingsFeatureOwnerKind::ManagedService
        && !matches!(&registration.policy_group, ConsolePolicyGroup::Plugin(_))
    {
        return Err(ConsoleOperationRegistryError::new(
            "ManagedService operations must use their Plugin policy group",
        ));
    }
    if let ConsolePolicyGroup::Other(group_id) = &registration.policy_group {
        validate_non_empty(group_id, "Other policy group_id")?;
    }
    if let ConsoleAuthorization::ResourceAction {
        resource_code,
        action_code,
    } = &registration.authorization
    {
        let known = resources.get(resource_code).is_some_and(|resource| {
            resource
                .actions
                .iter()
                .any(|action| action.action_code == *action_code)
        });
        if !known {
            return Err(ConsoleOperationRegistryError::new(format!(
                "operation {} references unknown resource action {resource_code}.{action_code}",
                registration.operation_id
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_resource(
    registration: &ResourceAccessRegistration,
) -> Result<(), ConsoleOperationRegistryError> {
    validate_non_empty(&registration.resource_code, "resource_code")?;
    validate_owner(&registration.owner, "resource owner")?;
    validate_host_extension_namespace(
        &registration.owner,
        &registration.resource_code,
        "resource_code",
    )?;
    validate_non_empty(&registration.identity_field, "resource identity_field")?;
    validate_optional_non_empty(registration.scope_field.as_deref(), "resource scope_field")?;
    validate_optional_non_empty(registration.owner_field.as_deref(), "resource owner_field")?;
    validate_non_empty(&registration.label_ref, "resource label_ref")?;
    validate_optional_non_empty(
        registration.description_ref.as_deref(),
        "resource description_ref",
    )?;
    if registration.lifecycle == SettingsFeatureLifecycle::Inactive {
        return Err(ConsoleOperationRegistryError::new(format!(
            "inactive resource {} cannot expose actions",
            registration.resource_code
        )));
    }
    if registration.actions.is_empty() {
        return Err(ConsoleOperationRegistryError::new(format!(
            "resource {} must register at least one action",
            registration.resource_code
        )));
    }
    let mut actions = BTreeSet::new();
    for action in &registration.actions {
        validate_non_empty(&action.action_code, "resource action_code")?;
        validate_non_empty(&action.label_ref, "resource action label_ref")?;
        validate_optional_non_empty(
            action.description_ref.as_deref(),
            "resource action description_ref",
        )?;
        if !actions.insert(action.action_code.as_str()) {
            return Err(ConsoleOperationRegistryError::new(format!(
                "duplicate resource action {}.{}",
                registration.resource_code, action.action_code
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_route(
    route: &ConsoleRouteBinding,
) -> Result<(), ConsoleOperationRegistryError> {
    let method = route.method.to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "HEAD" | "OPTIONS" | "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return Err(ConsoleOperationRegistryError::new(format!(
            "unsupported console route method {}",
            route.method
        )));
    }
    if !route.path.starts_with("/api/console/") && route.path != "/api/console" {
        return Err(ConsoleOperationRegistryError::new(format!(
            "console route path must start with /api/console: {}",
            route.path
        )));
    }
    Ok(())
}

pub(super) fn validate_owner(
    owner: &ConsoleOperationOwner,
    field: &str,
) -> Result<(), ConsoleOperationRegistryError> {
    validate_non_empty(&owner.owner_id, &format!("{field} owner_id"))?;
    validate_non_empty(&owner.version, &format!("{field} version"))
}

pub(super) fn validate_host_extension_namespace(
    owner: &ConsoleOperationOwner,
    identifier: &str,
    field: &str,
) -> Result<(), ConsoleOperationRegistryError> {
    if matches!(
        owner.kind,
        SettingsFeatureOwnerKind::HostExtension | SettingsFeatureOwnerKind::ManagedService
    ) && identifier != owner.owner_id
        && !identifier.starts_with(&format!("{}.", owner.owner_id))
    {
        return Err(ConsoleOperationRegistryError::new(format!(
            "HostExtension {field} must equal owner_id or start with <owner_id>."
        )));
    }
    Ok(())
}

pub(super) fn validate_non_empty(
    value: &str,
    field: &str,
) -> Result<(), ConsoleOperationRegistryError> {
    if value.trim().is_empty() {
        Err(ConsoleOperationRegistryError::new(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn validate_optional_non_empty(
    value: Option<&str>,
    field: &str,
) -> Result<(), ConsoleOperationRegistryError> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        Err(ConsoleOperationRegistryError::new(format!(
            "{field} must not be empty when present"
        )))
    } else {
        Ok(())
    }
}
