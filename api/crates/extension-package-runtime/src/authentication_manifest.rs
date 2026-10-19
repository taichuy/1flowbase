//! Authentication descriptor, contribution and execution form one Host-governed declaration.
use std::collections::BTreeSet;

use crate::{
    FrameworkResult, ManagedContributionPayload, PluginExecutionMode, PluginFrameworkError,
    PluginManifestV1,
};

pub(crate) fn validate_authentication_manifest(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    if manifest.authentication_providers.is_empty() {
        return Ok(());
    }
    if manifest.execution_mode != PluginExecutionMode::ProcessPerCall
        || manifest.runtime.protocol != extension_contracts::STDIO_JSON_MULTIPLEX_V1
        || manifest.binding_targets != ["system"]
        || manifest.module_kind() == extension_contracts::ModuleKind::TrustedHost
    {
        return Err(invalid("authentication providers require managed process_per_call multiplex system-only packages"));
    }
    let managed = manifest
        .managed
        .as_ref()
        .ok_or_else(|| invalid("authentication providers require managed declarations"))?;
    let mut auth_types = BTreeSet::new();
    for descriptor in &manifest.authentication_providers {
        descriptor.validate().map_err(|error| invalid(&error))?;
        if !auth_types.insert(&descriptor.auth_type) {
            return Err(invalid("duplicate authentication auth_type"));
        }
        let contribution = managed
            .module
            .contributions
            .iter()
            .find(|item| item.contribution_id == descriptor.contribution_id)
            .ok_or_else(|| invalid("missing authentication contribution"))?;
        let binding = managed
            .execution_bindings
            .iter()
            .find(|item| item.contribution_id == descriptor.contribution_id)
            .ok_or_else(|| invalid("missing authentication execution binding"))?;
        if contribution.point_id.as_str() != extension_contracts::AUTHENTICATION_POINT
            || contribution.contract_version.as_str()
                != extension_contracts::AUTHENTICATION_CONTRACT_V1
            || contribution.mode != extension_contracts::ContributionMode::Append
            || !contribution.required_permissions.iter().any(|permission| {
                permission.as_str() == extension_contracts::AUTHENTICATION_PERMISSION
            })
            || binding.execution_mode != PluginExecutionMode::ProcessPerCall
            || binding.runtime.protocol != extension_contracts::STDIO_JSON_MULTIPLEX_V1
            || binding.payload
                != Some(ManagedContributionPayload::AuthenticationProvider {
                    auth_type: descriptor.auth_type.clone(),
                })
        {
            return Err(invalid("authentication descriptor requires its exact authentication.verify process_per_call multiplex contribution and payload"));
        }
    }
    Ok(())
}

fn invalid(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}
