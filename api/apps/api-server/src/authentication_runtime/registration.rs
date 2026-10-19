use crate::error_response::ApiError;
use control_plane::{
    auth::AuthenticatorRegistry, errors::ControlPlaneError as Error, ports::PluginRepository,
};
use extension_contracts::AuthenticationProviderDescriptor;
use std::{path::Path, sync::Arc};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

pub(super) async fn active_declarations(
    store: &MainDurableStore,
    node_id: &str,
    install_root: &str,
) -> Result<Vec<(Uuid, AuthenticationProviderDescriptor)>, ApiError> {
    let mut declarations = Vec::new();
    let mut auth_types = std::collections::BTreeSet::new();
    for installation in store.list_installations().await? {
        if installation.desired_state != domain::PluginDesiredState::ActiveRequested
            || domain::managed_installation_scope(&installation, domain::DEFAULT_SCOPE_ID)
                != domain::SYSTEM_SCOPE_ID
            || installation
                .metadata_json
                .get("authentication_providers")
                .and_then(serde_json::Value::as_array)
                .is_none_or(Vec::is_empty)
        {
            continue;
        }
        let manifest = control_plane::plugin_management::verified_installed_managed_manifest(
            store,
            node_id,
            Path::new(install_root),
            installation.id,
        )
        .await?
        .ok_or(Error::PluginUnavailable)?;
        for declaration in manifest.authentication_providers {
            if !auth_types.insert(declaration.auth_type.clone()) {
                return Err(Error::Conflict("auth_provider").into());
            }
            declarations.push((installation.id, declaration));
        }
    }
    Ok(declarations)
}

pub(crate) async fn registry_for(
    store: &MainDurableStore,
    node_id: &str,
    install_root: &str,
    base: &AuthenticatorRegistry,
) -> Result<Arc<AuthenticatorRegistry>, ApiError> {
    let providers = active_declarations(store, node_id, install_root)
        .await?
        .into_iter()
        .map(|(_, descriptor)| descriptor)
        .collect::<Vec<_>>();
    Ok(Arc::new(base.with_managed_definitions(&providers)?))
}
