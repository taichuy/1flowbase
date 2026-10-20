use super::*;

pub(super) fn validate_console_contributions(
    manifest: &HostExtensionContributionManifest,
) -> FrameworkResult<()> {
    let declared_i18n_refs = manifest
        .console_locale_catalog
        .texts
        .iter()
        .map(|text| text.reference.clone())
        .collect::<BTreeSet<_>>();
    if declared_i18n_refs.len() != manifest.console_locale_catalog.texts.len() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_locale_catalog.texts[].reference must be unique",
        ));
    }
    for text in &manifest.console_locale_catalog.texts {
        validate_i18n_ref(
            &manifest.extension_id,
            &text.reference,
            "console_locale_catalog.texts[].reference",
        )?;
        validate_non_empty(&text.en_us, "console_locale_catalog.texts[].en_us")?;
        validate_non_empty(&text.zh_hans, "console_locale_catalog.texts[].zh_hans")?;
    }

    let mut operation_ids = BTreeSet::<String>::new();
    let mut resource_codes = BTreeSet::<String>::new();
    let mut route_owners = BTreeMap::<(String, String), String>::new();
    let mut referenced_i18n_refs = BTreeSet::<String>::new();

    for feature in &manifest.settings_features {
        validate_i18n_reference(
            manifest,
            &feature.console_surface.label_key,
            "settings_features[].console_surface.label_key",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
        validate_i18n_reference(
            manifest,
            &feature.console_surface.description_key,
            "settings_features[].console_surface.description_key",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
    }

    for operation in &manifest.console_operations {
        validate_console_operation(manifest, operation, &mut operation_ids, &mut route_owners)?;
    }

    for resource in &manifest.console_resources {
        validate_console_resource(
            manifest,
            resource,
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
            &mut resource_codes,
        )?;
    }

    let resources = manifest
        .console_resources
        .iter()
        .map(|resource| (resource.resource_code.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    for operation in &manifest.console_operations {
        if let ConsoleAuthorization::ResourceAction {
            resource_code,
            action_code,
        } = &operation.authorization
        {
            let Some(resource) = resources.get(resource_code.as_str()) else {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "console_operations[].authorization references unknown resource",
                ));
            };
            if !resource
                .actions
                .iter()
                .any(|action| action.action_code == *action_code)
            {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "console_operations[].authorization references unknown resource action",
                ));
            }
        }
    }

    let used_other_groups = manifest
        .console_operations
        .iter()
        .filter_map(|operation| match &operation.policy_group {
            ConsolePolicyGroup::Other(group_id) => Some(group_id.as_str()),
            ConsolePolicyGroup::SettingsFeature(_) => None,
        })
        .collect::<BTreeSet<_>>();
    let mut declared_other_groups = BTreeSet::new();
    for group in &manifest.console_locale_catalog.policy_groups {
        validate_non_empty(
            &group.group_id,
            "console_locale_catalog.policy_groups[].group_id",
        )?;
        if !is_extension_owned_group_id(&manifest.extension_id, &group.group_id) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_locale_catalog.policy_groups[].group_id must stay in the extension namespace",
            ));
        }
        if !declared_other_groups.insert(group.group_id.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_locale_catalog.policy_groups[].group_id must be unique",
            ));
        }
        if !used_other_groups.contains(group.group_id.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_locale_catalog.policy_groups[] contains an unreferenced group",
            ));
        }
        validate_i18n_reference(
            manifest,
            &group.label_ref,
            "console_locale_catalog.policy_groups[].label_ref",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
        validate_i18n_reference(
            manifest,
            &group.description_ref,
            "console_locale_catalog.policy_groups[].description_ref",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
    }
    if used_other_groups
        .iter()
        .any(|group_id| !declared_other_groups.contains(group_id))
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_operations[].policy_group is missing a locale catalog display",
        ));
    }

    if referenced_i18n_refs.len() != declared_i18n_refs.len()
        || declared_i18n_refs
            .iter()
            .any(|reference| !referenced_i18n_refs.contains(reference))
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_locale_catalog.texts[] contains an unused reference",
        ));
    }

    Ok(())
}

pub(super) fn validate_console_operation(
    manifest: &HostExtensionContributionManifest,
    operation: &ConsoleOperationRegistration,
    operation_ids: &mut BTreeSet<String>,
    route_owners: &mut BTreeMap<(String, String), String>,
) -> FrameworkResult<()> {
    let field = "console_operations[]";
    validate_non_empty(&operation.operation_id, &format!("{field}.operation_id"))?;
    if !operation_ids.insert(operation.operation_id.clone()) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_operations[].operation_id must be unique",
        ));
    }
    validate_console_owner(manifest, &operation.owner, &format!("{field}.owner"))?;
    validate_extension_owned_id(
        &manifest.extension_id,
        &operation.operation_id,
        &format!("{field}.operation_id"),
    )?;
    validate_active_lifecycle(operation.lifecycle, &format!("{field}.lifecycle"))?;
    validate_console_policy_group(manifest, &operation.policy_group, field)?;
    if operation.routes.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_operations[].routes must not be empty",
        ));
    }

    for route in &operation.routes {
        validate_console_route(route, field, route_owners, &operation.operation_id)?;
    }

    Ok(())
}

pub(super) fn validate_console_resource(
    manifest: &HostExtensionContributionManifest,
    resource: &ResourceAccessRegistration,
    declared_i18n_refs: &BTreeSet<String>,
    referenced_i18n_refs: &mut BTreeSet<String>,
    resource_codes: &mut BTreeSet<String>,
) -> FrameworkResult<()> {
    let field = "console_resources[]";
    validate_non_empty(&resource.resource_code, &format!("{field}.resource_code"))?;
    if !resource_codes.insert(resource.resource_code.clone()) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_resources[].resource_code must be unique",
        ));
    }
    validate_console_owner(manifest, &resource.owner, &format!("{field}.owner"))?;
    validate_extension_owned_id(
        &manifest.extension_id,
        &resource.resource_code,
        &format!("{field}.resource_code"),
    )?;
    validate_active_lifecycle(resource.lifecycle, &format!("{field}.lifecycle"))?;
    validate_non_empty(&resource.identity_field, &format!("{field}.identity_field"))?;
    validate_optional_non_empty(
        resource.scope_field.as_deref(),
        &format!("{field}.scope_field"),
    )?;
    validate_optional_non_empty(
        resource.owner_field.as_deref(),
        &format!("{field}.owner_field"),
    )?;
    validate_i18n_reference(
        manifest,
        &resource.label_ref,
        &format!("{field}.label_ref"),
        declared_i18n_refs,
        referenced_i18n_refs,
    )?;
    if let Some(description_ref) = resource.description_ref.as_deref() {
        validate_i18n_reference(
            manifest,
            description_ref,
            &format!("{field}.description_ref"),
            declared_i18n_refs,
            referenced_i18n_refs,
        )?;
    }
    if resource.actions.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_resources[].actions must not be empty",
        ));
    }

    let mut action_codes = BTreeSet::new();
    for action in &resource.actions {
        validate_console_resource_action(
            manifest,
            action,
            declared_i18n_refs,
            referenced_i18n_refs,
            &mut action_codes,
        )?;
    }

    Ok(())
}

pub(super) fn validate_console_resource_action(
    manifest: &HostExtensionContributionManifest,
    action: &ResourceAccessAction,
    declared_i18n_refs: &BTreeSet<String>,
    referenced_i18n_refs: &mut BTreeSet<String>,
    action_codes: &mut BTreeSet<String>,
) -> FrameworkResult<()> {
    let field = "console_resources[].actions[]";
    validate_non_empty(&action.action_code, &format!("{field}.action_code"))?;
    if !action_codes.insert(action.action_code.clone()) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_resources[].actions[].action_code must be unique",
        ));
    }
    validate_i18n_reference(
        manifest,
        &action.label_ref,
        &format!("{field}.label_ref"),
        declared_i18n_refs,
        referenced_i18n_refs,
    )?;
    if let Some(description_ref) = action.description_ref.as_deref() {
        validate_i18n_reference(
            manifest,
            description_ref,
            &format!("{field}.description_ref"),
            declared_i18n_refs,
            referenced_i18n_refs,
        )?;
    }
    Ok(())
}

pub(super) fn validate_console_owner(
    manifest: &HostExtensionContributionManifest,
    owner: &access_control::ConsoleOperationOwner,
    field: &str,
) -> FrameworkResult<()> {
    if owner.kind != SettingsFeatureOwnerKind::HostExtension {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.kind must be host_extension"
        )));
    }
    if owner.owner_id != manifest.extension_id {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.owner_id must equal extension_id"
        )));
    }
    if owner.version != manifest.version {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.version must equal extension version"
        )));
    }
    validate_non_empty(&owner.owner_id, &format!("{field}.owner_id"))?;
    validate_non_empty(&owner.version, &format!("{field}.version"))?;
    Ok(())
}

pub(super) fn validate_active_lifecycle(
    lifecycle: SettingsFeatureLifecycle,
    field: &str,
) -> FrameworkResult<()> {
    if lifecycle == SettingsFeatureLifecycle::Inactive {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must be active"
        )));
    }
    Ok(())
}

pub(super) fn validate_console_policy_group(
    manifest: &HostExtensionContributionManifest,
    group: &ConsolePolicyGroup,
    field: &str,
) -> FrameworkResult<()> {
    match group {
        ConsolePolicyGroup::SettingsFeature(feature_id) => {
            if !manifest
                .settings_features
                .iter()
                .any(|feature| feature.feature_id == *feature_id)
            {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "{field}.policy_group references unknown settings feature"
                )));
            }
        }
        ConsolePolicyGroup::Other(group_id) => {
            validate_non_empty(group_id, &format!("{field}.policy_group"))?;
            if !is_extension_owned_group_id(&manifest.extension_id, group_id) {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "{field}.policy_group must stay in the extension namespace"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_console_route(
    route: &access_control::ConsoleRouteBinding,
    field: &str,
    route_owners: &mut BTreeMap<(String, String), String>,
    operation_id: &str,
) -> FrameworkResult<()> {
    let method = route.method.to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "HEAD" | "OPTIONS" | "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.routes[].method is unsupported"
        )));
    }
    if !route.path.starts_with("/api/console/") && route.path != "/api/console" {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.routes[].path must start with /api/console/"
        )));
    }

    let shape = console_route_shape(&route.path);
    let key = (method.clone(), shape.clone());
    if route_owners.contains_key(&key)
        || route_owners
            .iter()
            .any(|((existing_method, existing_shape), _)| {
                existing_method == &method
                    && console_route_templates_are_ambiguous(existing_shape, &shape)
            })
    {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.routes contains duplicate or ambiguous ownership"
        )));
    }
    route_owners.insert(key, operation_id.to_string());
    Ok(())
}
