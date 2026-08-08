use std::{collections::HashMap, path::Path};

use anyhow::Result;
use plugin_framework::{
    provider_contract::{ModelDiscoveryMode, ProviderModelDescriptor, ProviderModelSource},
    provider_package::ProviderConfigField,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    plugin_management::ready_current_node_plugin_installation,
    ports::{AuthRepository, PluginRepository},
};

#[derive(Clone, Copy)]
pub(super) struct ModelProviderNodeArtifactContext<'a> {
    pub node_id: &'a str,
    pub install_root: &'a Path,
}

pub(super) async fn ready_model_provider_installation<R>(
    repository: &R,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    installation_id: Uuid,
) -> Result<domain::LocalPluginInstallationRecord>
where
    R: PluginRepository,
{
    match node_artifact_context {
        Some(context) => {
            ready_current_node_plugin_installation(
                repository,
                context.node_id,
                context.install_root,
                installation_id,
            )
            .await
        }
        None => Err(ControlPlaneError::Conflict("plugin_node_context_required").into()),
    }
}

pub(super) async fn model_provider_installation_from_current_snapshot<R>(
    repository: &R,
    node_artifact_context: Option<ModelProviderNodeArtifactContext<'_>>,
    installation: domain::PluginInstallationRecord,
) -> Result<Option<domain::LocalPluginInstallationRecord>>
where
    R: PluginRepository,
{
    let Some(context) = node_artifact_context else {
        return Err(ControlPlaneError::Conflict("plugin_node_context_required").into());
    };
    let Some(artifact) = repository
        .get_artifact_instance(context.node_id, installation.id)
        .await?
    else {
        return Ok(None);
    };
    if !artifact.artifact_status.is_ready() {
        return Ok(None);
    }
    if artifact.local_path.is_none() {
        return Ok(None);
    }
    Ok(Some(domain::LocalPluginInstallationRecord {
        installation,
        artifact,
    }))
}

pub(super) async fn load_actor_context_for_user<R>(
    repository: &R,
    actor_user_id: Uuid,
) -> Result<domain::ActorContext>
where
    R: AuthRepository,
{
    let scope = repository.default_scope_for_user(actor_user_id).await?;
    repository
        .load_actor_context(actor_user_id, scope.tenant_id, scope.workspace_id, None)
        .await
}

pub(super) async fn ensure_installation_assigned<R>(
    repository: &R,
    workspace_id: Uuid,
    installation_id: Uuid,
) -> Result<()>
where
    R: PluginRepository,
{
    let assigned = repository
        .list_assignments(workspace_id)
        .await?
        .into_iter()
        .any(|assignment| assignment.installation_id == installation_id);
    if assigned {
        Ok(())
    } else {
        Err(ControlPlaneError::Conflict("plugin_assignment_required").into())
    }
}

pub(super) async fn ensure_model_provider_permission(
    actor: &domain::ActorContext,
    action: &str,
    use_case: &super::ModelProviderUseCase,
) -> Result<(), ControlPlaneError> {
    match use_case {
        super::ModelProviderUseCase::BusinessActions
            if actor.is_root
                || actor.has_permission(&format!("state_model.{action}.all"))
                || actor.has_permission(&format!("state_model.{action}.own")) =>
        {
            Ok(())
        }
        super::ModelProviderUseCase::ConsoleOperation {
            policy_reader,
            group,
            operation_id,
        } if actor.is_root
            || domain::effective_console_simple_operation(
                &policy_reader
                    .load_role_console_policies_for_user(actor)
                    .await
                    .map_err(|_| ControlPlaneError::PermissionDenied("permission_denied"))?,
                group,
                operation_id,
            ) =>
        {
            Ok(())
        }
        _ => Err(ControlPlaneError::PermissionDenied("permission_denied")),
    }
}

pub(super) fn normalize_required_text(
    value: &str,
    field: &'static str,
) -> Result<String, anyhow::Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ControlPlaneError::InvalidInput(field).into())
    } else {
        Ok(trimmed.to_string())
    }
}

pub(super) fn split_provider_config(
    form_schema: &[ProviderConfigField],
    input: &Value,
) -> Result<(Value, Value)> {
    let object = input
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let mut public = Map::new();
    let mut secret = Map::new();
    let field_lookup = form_schema
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect::<HashMap<_, _>>();
    for (key, value) in object {
        let field = field_lookup
            .get(key.as_str())
            .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
        if is_secret_field(&field.field_type) {
            secret.insert(key.clone(), value.clone());
        } else {
            public.insert(key.clone(), value.clone());
        }
    }
    Ok((Value::Object(public), Value::Object(secret)))
}

pub(super) fn validate_required_fields(
    form_schema: &[ProviderConfigField],
    public_config: &Value,
    secret_config: &Value,
) -> Result<()> {
    let public_object = public_config
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let secret_object = secret_config
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    for field in form_schema {
        if !field.required {
            continue;
        }
        let value = if is_secret_field(&field.field_type) {
            secret_object.get(&field.key)
        } else {
            public_object.get(&field.key)
        };
        if value.is_none()
            || value == Some(&Value::Null)
            || value == Some(&Value::String(String::new()))
        {
            return Err(ControlPlaneError::InvalidInput("config_json").into());
        }
    }
    Ok(())
}

pub(super) fn merge_json_object(base: &Value, patch: &Value) -> Result<Value> {
    let mut merged = base
        .as_object()
        .cloned()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let patch_object = patch
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    for (key, value) in patch_object {
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
}

pub(super) fn mask_secret_config(
    base: &Value,
    secret_json: &Value,
    form_schema: &[ProviderConfigField],
) -> Result<Value> {
    let mut merged = base
        .as_object()
        .cloned()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    let secret_object = secret_json
        .as_object()
        .ok_or(ControlPlaneError::InvalidInput("config_json"))?;
    for field in form_schema {
        if !is_secret_field(&field.field_type) {
            continue;
        }

        let Some(value) = secret_object.get(&field.key) else {
            continue;
        };
        merged.insert(field.key.clone(), mask_secret_value(value));
    }

    Ok(Value::Object(merged))
}

fn mask_secret_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(mask_secret_preview(text)),
        Value::Null => Value::Null,
        _ => Value::String("****".to_string()),
    }
}

fn mask_secret_preview(value: &str) -> String {
    let char_count = value.chars().count();
    if char_count <= 8 {
        return "****".to_string();
    }

    let prefix = value.chars().take(4).collect::<String>();
    let suffix = value
        .chars()
        .skip(char_count.saturating_sub(4))
        .collect::<String>();
    format!("{prefix}****{suffix}")
}

pub(super) fn empty_object() -> Value {
    Value::Object(Map::new())
}

pub(super) fn is_empty_object(value: &Value) -> bool {
    value
        .as_object()
        .map(|object| object.is_empty())
        .unwrap_or(false)
}

pub(super) fn is_secret_field(field_type: &str) -> bool {
    field_type.trim().eq_ignore_ascii_case("secret")
}

pub(super) fn localized_model_descriptor(
    namespace: &str,
    model: ProviderModelDescriptor,
) -> crate::model_provider::LocalizedProviderModelDescriptor {
    let display_name_fallback = Some(model.display_name.clone());
    match model.source {
        ProviderModelSource::Static => {
            let model_key = model_i18n_key(&model.model_id);
            crate::model_provider::LocalizedProviderModelDescriptor {
                descriptor: model,
                namespace: Some(namespace.to_string()),
                label_key: Some(format!("models.{model_key}.label")),
                description_key: Some(format!("models.{model_key}.description")),
                display_name_fallback,
            }
        }
        ProviderModelSource::Dynamic => crate::model_provider::LocalizedProviderModelDescriptor {
            descriptor: model,
            namespace: None,
            label_key: None,
            description_key: None,
            display_name_fallback,
        },
    }
}

fn model_i18n_key(model_id: &str) -> String {
    model_id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn map_model_discovery_mode(
    mode: ModelDiscoveryMode,
) -> domain::ModelProviderDiscoveryMode {
    match mode {
        ModelDiscoveryMode::Static => domain::ModelProviderDiscoveryMode::Static,
        ModelDiscoveryMode::Dynamic => domain::ModelProviderDiscoveryMode::Dynamic,
        ModelDiscoveryMode::Hybrid => domain::ModelProviderDiscoveryMode::Hybrid,
    }
}

pub(super) fn map_catalog_source(mode: ModelDiscoveryMode) -> domain::ModelProviderCatalogSource {
    match mode {
        ModelDiscoveryMode::Static => domain::ModelProviderCatalogSource::Static,
        ModelDiscoveryMode::Dynamic => domain::ModelProviderCatalogSource::Dynamic,
        ModelDiscoveryMode::Hybrid => domain::ModelProviderCatalogSource::Hybrid,
    }
}
