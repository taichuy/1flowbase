use super::*;

pub(super) fn validate_settings_feature_operation_coverage(
    settings_features: &SettingsFeatureRegistry,
    registrations: &[ConsoleOperationRegistration],
) -> Result<(), ConsoleOperationRegistryError> {
    let feature_routes = settings_features
        .inventory()
        .features
        .iter()
        .map(|feature| {
            let routes = feature
                .api_routes
                .iter()
                .map(|route| (route.method.to_ascii_uppercase(), route_shape(&route.path)))
                .collect::<BTreeSet<_>>();
            (feature.feature_id.as_str(), routes)
        })
        .collect::<BTreeMap<_, _>>();
    let mut claimed_routes = BTreeMap::<&str, BTreeSet<(String, String)>>::new();

    for registration in registrations {
        let ConsolePolicyGroup::SettingsFeature(feature_id) = &registration.policy_group else {
            continue;
        };
        let expected_routes = feature_routes.get(feature_id.as_str()).ok_or_else(|| {
            ConsoleOperationRegistryError::new(format!(
                "operation {} references unknown settings feature {feature_id}",
                registration.operation_id
            ))
        })?;
        let claims = claimed_routes.entry(feature_id.as_str()).or_default();
        for route in &registration.routes {
            let route_key = (route.method.to_ascii_uppercase(), route_shape(&route.path));
            if expected_routes.contains(&route_key) {
                claims.insert(route_key);
            }
        }
    }

    for (feature_id, expected_routes) in feature_routes {
        let claims = claimed_routes.get(feature_id).cloned().unwrap_or_default();
        if let Some((method, path)) = expected_routes.difference(&claims).next() {
            return Err(ConsoleOperationRegistryError::new(format!(
                "settings feature {feature_id} route has no configurable operation: {method} {path}"
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
