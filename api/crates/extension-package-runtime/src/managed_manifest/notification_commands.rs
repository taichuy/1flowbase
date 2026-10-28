use super::*;

pub(super) fn validate(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    let managed = manifest
        .managed
        .as_ref()
        .expect("validated managed manifest");
    let mut command_ids = BTreeSet::new();
    let mut command_names = BTreeSet::new();
    for command in &manifest.communication_commands {
        command
            .validate_plugin_namespace(&manifest.publisher_namespace, manifest.plugin_code()?)
            .map_err(|error| invalid(&error.to_string()))?;
        if !command_ids.insert(&command.command_id) || !command_names.insert(&command.name) {
            return Err(invalid(
                "communication commands require unique identities and names",
            ));
        }
        let contribution = managed
            .module
            .contributions
            .iter()
            .find(|item| item.contribution_id == command.contribution_id)
            .ok_or_else(|| invalid("communication command contribution is not declared"))?;
        let binding = managed
            .execution_bindings
            .iter()
            .find(|item| item.contribution_id == command.contribution_id)
            .ok_or_else(|| invalid("communication command execution binding is missing"))?;
        if contribution.point_id.as_str() != extension_contracts::COMMUNICATION_COMMAND_POINT
            || contribution.contract_version.as_str()
                != extension_contracts::COMMUNICATION_COMMAND_CONTRACT_VERSION
            || contribution.mode != extension_contracts::ContributionMode::Append
            || !contribution
                .required_permissions
                .iter()
                .any(|permission| permission.as_str() == "event.subscribe")
            || binding.execution_mode != PluginExecutionMode::ProcessPerCall
            || binding.payload.is_some()
            || binding.interface_protocol.is_some()
            || !manifest
                .binding_targets
                .iter()
                .any(|target| target == "workspace")
        {
            return Err(invalid("communication command requires an explicit workspace command event subscriber binding"));
        }
    }
    Ok(())
}
