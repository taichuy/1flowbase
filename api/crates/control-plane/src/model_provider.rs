use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
};

use anyhow::Result;
use plugin_framework::{
    provider_contract::{PluginFormSchema, ProviderBalanceResult, ProviderModelDescriptor},
    provider_package::{ProviderConfigField, ProviderPackage},
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    i18n::{I18nCatalog, RequestedLocales},
    plugin_management::mark_current_node_plugin_runtime_status,
    ports::{
        AuthRepository, CreateModelProviderInstanceInput, CreateModelProviderPreviewSessionInput,
        ModelProviderRepository, PluginRepository, ProviderRuntimePort,
        UpdateModelProviderInstanceInput, UpsertModelProviderCatalogCacheInput,
        UpsertModelProviderSecretInput,
    },
    state_transition::ensure_model_provider_instance_transition,
};

mod balance;
mod catalog;
pub mod catalog_source;
pub mod failover_queue;
mod instances;
mod main_instance;
mod options;
mod request_log_maintenance;
pub(crate) mod routing;
mod shared;

use self::{
    instances::{build_provider_runtime_config, hydrate_instance_view},
    shared::{
        empty_object, ensure_installation_assigned, ensure_model_provider_permission,
        is_empty_object, load_actor_context_for_user, load_provider_package, map_catalog_source,
        map_model_discovery_mode, merge_json_object, normalize_required_text,
        ready_model_provider_installation, split_provider_config, validate_required_fields,
        ModelProviderNodeArtifactContext,
    },
};

pub use request_log_maintenance::{
    ClearModelProviderRequestLogsBatchCommand, ClearModelProviderRequestLogsBatchView,
    ClearModelProviderRequestLogsContinuation, DeleteSelectedModelProviderRequestLogsCommand,
    ListModelProviderRequestLogsCommand,
};

pub struct CreateModelProviderInstanceCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
    pub display_name: String,
    pub config_json: Value,
    pub configured_models: Vec<domain::ModelProviderConfiguredModel>,
    pub enabled_model_ids: Vec<String>,
    pub included_in_main: Option<bool>,
    pub preview_token: Option<Uuid>,
}

pub struct UpdateModelProviderInstanceCommand {
    pub actor_user_id: Uuid,
    pub instance_id: Uuid,
    pub display_name: String,
    pub config_json: Value,
    pub configured_models: Vec<domain::ModelProviderConfiguredModel>,
    pub enabled_model_ids: Vec<String>,
    pub included_in_main: bool,
    pub preview_token: Option<Uuid>,
}

pub struct UpdateModelProviderMainInstanceCommand {
    pub actor_user_id: Uuid,
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub model_distribution_rules: Option<Vec<domain::ModelProviderMainModelDistributionRule>>,
}

pub type ModelProviderConfiguredModelInput = domain::ModelProviderConfiguredModel;
pub type ModelProviderBalanceResult = ProviderBalanceResult;

pub struct DeleteModelProviderInstanceCommand {
    pub actor_user_id: Uuid,
    pub instance_id: Uuid,
}

pub struct PreviewModelProviderModelsCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub instance_id: Option<Uuid>,
    pub config_json: Value,
}

#[derive(Debug, Clone)]
pub struct ModelProviderCatalogEntry {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub display_name: String,
    pub protocol: String,
    pub help_url: Option<String>,
    pub default_base_url: Option<String>,
    pub model_discovery_mode: String,
    pub supports_model_fetch_without_credentials: bool,
    pub desired_state: String,
    pub availability_status: String,
    pub form_schema: Vec<ProviderConfigField>,
    pub predefined_models: Vec<LocalizedProviderModelDescriptor>,
    pub catalog_refresh_status: String,
    pub catalog_last_error_message: Option<String>,
    pub catalog_refreshed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct ModelProviderCatalogView {
    pub entries: Vec<ModelProviderCatalogEntry>,
    pub i18n_catalog: I18nCatalog,
}

#[derive(Debug, Clone)]
pub struct ModelProviderIconSource {
    pub installed_path: String,
    pub metadata_json: Value,
}

#[derive(Debug, Clone)]
pub struct LocalizedProviderModelDescriptor {
    pub descriptor: ProviderModelDescriptor,
    pub namespace: Option<String>,
    pub label_key: Option<String>,
    pub description_key: Option<String>,
    pub display_name_fallback: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ModelProviderInstanceView {
    pub instance: domain::ModelProviderInstanceRecord,
    pub cache: Option<domain::ModelProviderCatalogCacheRecord>,
}

#[derive(Debug, Clone)]
pub struct ModelProviderMainInstanceView {
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub model_distribution_rules: Vec<domain::ModelProviderMainModelDistributionRule>,
}

#[derive(Debug, Clone)]
pub struct ValidateModelProviderResult {
    pub instance: domain::ModelProviderInstanceRecord,
    pub cache: domain::ModelProviderCatalogCacheRecord,
    pub output: Value,
}

#[derive(Debug, Clone)]
pub struct ModelProviderModelCatalog {
    pub provider_instance_id: Uuid,
    pub refresh_status: domain::ModelProviderCatalogRefreshStatus,
    pub source: domain::ModelProviderCatalogSource,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
    pub models: Vec<ProviderModelDescriptor>,
}

#[derive(Debug, Clone)]
pub struct PreviewModelProviderModelsResult {
    pub models: Vec<ProviderModelDescriptor>,
    pub preview_token: Uuid,
    pub expires_at: OffsetDateTime,
}

struct PreviewStateRequest<'a> {
    installation_id: Option<Uuid>,
    instance_id: Option<Uuid>,
    provider_config: &'a Value,
    preview_token: Option<Uuid>,
}

struct ResolvedPreviewState {
    models_json: Option<Value>,
    refreshed_at: Option<OffsetDateTime>,
    preview_token: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ModelProviderOptionEntry {
    pub provider_code: String,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub protocol: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub parameter_form: Option<PluginFormSchema>,
    pub main_instance: ModelProviderMainInstanceSummary,
    pub model_groups: Vec<ModelProviderOptionGroup>,
}

#[derive(Debug, Clone)]
pub struct ModelProviderMainInstanceSummary {
    pub provider_code: String,
    pub auto_include_new_instances: bool,
    pub group_count: usize,
    pub model_count: usize,
}

#[derive(Debug, Clone)]
pub struct ModelProviderOptionGroup {
    pub model_id: String,
    pub distribution_rule: domain::ModelProviderDistributionRule,
    pub model: LocalizedProviderModelDescriptor,
    pub targets: Vec<ModelProviderOptionTarget>,
}

#[derive(Debug, Clone)]
pub struct ModelProviderOptionTarget {
    pub source_instance_id: Uuid,
    pub source_instance_display_name: String,
    pub model: LocalizedProviderModelDescriptor,
}

#[derive(Debug, Clone)]
pub struct ModelProviderOptionsView {
    pub providers: Vec<ModelProviderOptionEntry>,
    pub i18n_catalog: I18nCatalog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelProviderUseCase {
    BusinessActions,
    ModelProviderSettings,
}

pub struct ModelProviderService<R, H> {
    repository: R,
    runtime: H,
    provider_secret_master_key: String,
    node_id: Option<String>,
    install_root: Option<PathBuf>,
    use_case: ModelProviderUseCase,
}

impl<R, H> ModelProviderService<R, H>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
    H: ProviderRuntimePort,
{
    pub fn new(repository: R, runtime: H, provider_secret_master_key: impl Into<String>) -> Self {
        Self {
            repository,
            runtime,
            provider_secret_master_key: provider_secret_master_key.into(),
            node_id: None,
            install_root: None,
            use_case: ModelProviderUseCase::BusinessActions,
        }
    }

    pub fn for_model_provider_settings(
        repository: R,
        runtime: H,
        provider_secret_master_key: impl Into<String>,
    ) -> Self {
        Self {
            repository,
            runtime,
            provider_secret_master_key: provider_secret_master_key.into(),
            node_id: None,
            install_root: None,
            use_case: ModelProviderUseCase::ModelProviderSettings,
        }
    }

    pub fn with_node_artifact_context(
        mut self,
        node_id: impl Into<String>,
        install_root: impl Into<PathBuf>,
    ) -> Self {
        self.node_id = Some(node_id.into());
        self.install_root = Some(install_root.into());
        self
    }

    fn node_artifact_context(&self) -> Option<ModelProviderNodeArtifactContext<'_>> {
        Some(ModelProviderNodeArtifactContext {
            node_id: self.node_id.as_deref()?,
            install_root: self.install_root.as_deref()?,
        })
    }

    async fn ready_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<domain::PluginInstallationRecord> {
        ready_model_provider_installation(
            &self.repository,
            self.node_artifact_context(),
            installation_id,
        )
        .await
    }

    pub async fn list_catalog(
        &self,
        actor_user_id: Uuid,
        locales: RequestedLocales,
    ) -> Result<ModelProviderCatalogView> {
        catalog::list_catalog(&self.repository, actor_user_id, locales, self.use_case).await
    }

    pub async fn list_instances(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<ModelProviderInstanceView>> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_model_provider_permission(&actor, "view", self.use_case)?;

        let instances = self
            .repository
            .list_instances(actor.current_workspace_id)
            .await?;
        let mut form_schemas: HashMap<Uuid, Vec<ProviderConfigField>> = HashMap::new();
        let mut output = Vec::with_capacity(instances.len());
        for instance in instances {
            let cache = self.repository.get_catalog_cache(instance.id).await?;
            let form_schema = match form_schemas.get(&instance.installation_id) {
                Some(form_schema) => form_schema.clone(),
                None => {
                    let form_schema = self
                        .instance_list_form_schema(instance.installation_id)
                        .await?;
                    form_schemas.insert(instance.installation_id, form_schema.clone());
                    form_schema
                }
            };
            output.push(
                self.hydrate_instance_view(instance, cache, &form_schema)
                    .await?,
            );
        }
        Ok(output)
    }

    async fn instance_list_form_schema(
        &self,
        installation_id: Uuid,
    ) -> Result<Vec<ProviderConfigField>> {
        if let Some(node_id) = self.node_id.as_deref() {
            let Some(artifact) = self
                .repository
                .get_artifact_instance(node_id, installation_id)
                .await?
            else {
                return Ok(Vec::new());
            };
            if !artifact.artifact_status.is_ready() {
                return Ok(Vec::new());
            }
            let Some(installed_path) = artifact.installed_path.as_deref() else {
                return Ok(Vec::new());
            };
            return Ok(load_provider_package(installed_path)?.provider.form_schema);
        }

        let installation = self
            .repository
            .get_installation(installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        Ok(load_provider_package(&installation.installed_path)?
            .provider
            .form_schema)
    }

    pub async fn get_main_instance(
        &self,
        actor_user_id: Uuid,
        provider_code: &str,
    ) -> Result<ModelProviderMainInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_model_provider_permission(&actor, "view", self.use_case)?;
        main_instance::get_main_instance(
            &self.repository,
            actor.current_workspace_id,
            provider_code,
        )
        .await
    }

    pub async fn provider_icon_source(
        &self,
        actor_user_id: Uuid,
        provider_code: &str,
    ) -> Result<ModelProviderIconSource> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_model_provider_permission(&actor, "view", self.use_case)?;
        let assignment = self
            .repository
            .list_assignments(actor.current_workspace_id)
            .await?
            .into_iter()
            .find(|assignment| assignment.provider_code == provider_code)
            .ok_or(ControlPlaneError::NotFound("plugin_assignment"))?;
        let installation = self
            .repository
            .get_installation(assignment.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        Ok(ModelProviderIconSource {
            installed_path: installation.installed_path,
            metadata_json: installation.metadata_json,
        })
    }

    pub async fn create_instance(
        &self,
        command: CreateModelProviderInstanceCommand,
    ) -> Result<ModelProviderInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", self.use_case)?;
        let installation = self.ready_installation(command.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            actor.current_workspace_id,
            command.installation_id,
        )
        .await?;
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) {
            return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
        }

        let package = load_provider_package(&installation.installed_path)?;
        let (public_config, secret_config) =
            split_provider_config(&package.provider.form_schema, &command.config_json)?;
        validate_required_fields(
            &package.provider.form_schema,
            &public_config,
            &secret_config,
        )?;
        let provider_config = merge_json_object(&public_config, &secret_config)?;
        let configured_models =
            normalize_configured_models(command.configured_models, command.enabled_model_ids);
        let enabled_model_ids = configured_models_to_enabled_model_ids(&configured_models);
        let preview_state = self
            .resolve_preview_state(
                &actor,
                PreviewStateRequest {
                    installation_id: Some(installation.id),
                    instance_id: None,
                    provider_config: &provider_config,
                    preview_token: command.preview_token,
                },
            )
            .await?;
        let instance = self
            .repository
            .create_instance(&CreateModelProviderInstanceInput {
                instance_id: Uuid::now_v7(),
                workspace_id: actor.current_workspace_id,
                installation_id: installation.id,
                provider_code: installation.provider_code.clone(),
                protocol: installation.protocol.clone(),
                display_name: normalize_required_text(&command.display_name, "display_name")?,
                status: derive_instance_status(false, &enabled_model_ids),
                config_json: public_config.clone(),
                configured_models: configured_models.clone(),
                enabled_model_ids,
                included_in_main: command.included_in_main,
                created_by: command.actor_user_id,
            })
            .await?;
        if !is_empty_object(&secret_config) {
            self.repository
                .upsert_secret(&UpsertModelProviderSecretInput {
                    provider_instance_id: instance.id,
                    plaintext_secret_json: secret_config,
                    secret_version: 1,
                    master_key: self.provider_secret_master_key.clone(),
                })
                .await?;
        }
        if let Some(models_json) = preview_state.models_json.as_ref() {
            self.repository
                .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                    provider_instance_id: instance.id,
                    model_discovery_mode: map_model_discovery_mode(
                        package.provider.model_discovery_mode,
                    ),
                    refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                    source: map_catalog_source(package.provider.model_discovery_mode),
                    models_json: models_json.clone(),
                    last_error_message: None,
                    refreshed_at: preview_state.refreshed_at,
                })
                .await?;
        }
        if let Some(preview_token) = preview_state.preview_token {
            self.repository
                .delete_preview_session(actor.current_workspace_id, preview_token)
                .await?;
        }
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider_instance",
                Some(instance.id),
                "model_provider.created",
                json!({
                    "provider_code": instance.provider_code,
                    "display_name": instance.display_name,
                }),
            ))
            .await?;

        self.hydrate_instance_view(
            instance.clone(),
            self.repository.get_catalog_cache(instance.id).await?,
            &package.provider.form_schema,
        )
        .await
    }

    pub async fn update_instance(
        &self,
        command: UpdateModelProviderInstanceCommand,
    ) -> Result<ModelProviderInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", self.use_case)?;
        let existing = self
            .repository
            .get_instance(actor.current_workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        let installation = self.ready_installation(existing.installation_id).await?;
        let package = load_provider_package(&installation.installed_path)?;

        let (patch_public_config, patch_secret_config) =
            split_provider_config(&package.provider.form_schema, &command.config_json)?;
        let current_secret_json = self
            .repository
            .get_secret_json(existing.id, &self.provider_secret_master_key)
            .await?
            .unwrap_or_else(empty_object);
        let merged_public_config = merge_json_object(&existing.config_json, &patch_public_config)?;
        let merged_secret_config = merge_json_object(&current_secret_json, &patch_secret_config)?;
        validate_required_fields(
            &package.provider.form_schema,
            &merged_public_config,
            &merged_secret_config,
        )?;
        let provider_config = merge_json_object(&merged_public_config, &merged_secret_config)?;
        let configured_models =
            normalize_configured_models(command.configured_models, command.enabled_model_ids);
        let enabled_model_ids = configured_models_to_enabled_model_ids(&configured_models);

        if !is_empty_object(&patch_secret_config) {
            let version = self
                .repository
                .get_secret_record(existing.id)
                .await?
                .map(|record| record.secret_version + 1)
                .unwrap_or(1);
            self.repository
                .upsert_secret(&UpsertModelProviderSecretInput {
                    provider_instance_id: existing.id,
                    plaintext_secret_json: merged_secret_config.clone(),
                    secret_version: version,
                    master_key: self.provider_secret_master_key.clone(),
                })
                .await?;
        }

        let preview_state = self
            .resolve_preview_state(
                &actor,
                PreviewStateRequest {
                    installation_id: Some(installation.id),
                    instance_id: Some(existing.id),
                    provider_config: &provider_config,
                    preview_token: command.preview_token,
                },
            )
            .await?;
        let next_status = derive_instance_status(
            matches!(
                existing.status,
                domain::ModelProviderInstanceStatus::Disabled
            ),
            &enabled_model_ids,
        );
        ensure_model_provider_instance_transition(existing.status, next_status, "update_instance")?;

        let updated = self
            .repository
            .update_instance(&UpdateModelProviderInstanceInput {
                instance_id: existing.id,
                workspace_id: actor.current_workspace_id,
                display_name: normalize_required_text(&command.display_name, "display_name")?,
                status: next_status,
                config_json: merged_public_config,
                configured_models: configured_models.clone(),
                enabled_model_ids,
                included_in_main: command.included_in_main,
                updated_by: command.actor_user_id,
            })
            .await?;
        if let Some(models_json) = preview_state.models_json.as_ref() {
            self.repository
                .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                    provider_instance_id: existing.id,
                    model_discovery_mode: map_model_discovery_mode(
                        package.provider.model_discovery_mode,
                    ),
                    refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                    source: map_catalog_source(package.provider.model_discovery_mode),
                    models_json: models_json.clone(),
                    last_error_message: None,
                    refreshed_at: preview_state.refreshed_at,
                })
                .await?;
        }
        if let Some(preview_token) = preview_state.preview_token {
            self.repository
                .delete_preview_session(actor.current_workspace_id, preview_token)
                .await?;
        }
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider_instance",
                Some(updated.id),
                "model_provider.updated",
                json!({
                    "provider_code": updated.provider_code,
                    "display_name": updated.display_name,
                }),
            ))
            .await?;

        self.hydrate_instance_view(
            updated,
            self.repository.get_catalog_cache(existing.id).await?,
            &package.provider.form_schema,
        )
        .await
    }

    async fn resolve_preview_state(
        &self,
        actor: &domain::ActorContext,
        request: PreviewStateRequest<'_>,
    ) -> Result<ResolvedPreviewState> {
        let Some(preview_token) = request.preview_token else {
            return Ok(ResolvedPreviewState {
                models_json: None,
                refreshed_at: None,
                preview_token: None,
            });
        };

        let session = self
            .repository
            .get_preview_session(actor.current_workspace_id, preview_token)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("preview_token"))?;
        if session.actor_user_id != actor.user_id || session.expires_at < OffsetDateTime::now_utc()
        {
            return Err(ControlPlaneError::InvalidInput("preview_token").into());
        }
        if session.installation_id != request.installation_id
            || session.instance_id != request.instance_id
        {
            return Err(ControlPlaneError::InvalidInput("preview_token").into());
        }
        if session.config_fingerprint != fingerprint_provider_config(request.provider_config)? {
            return Err(ControlPlaneError::InvalidInput("preview_token").into());
        }

        let models_json = session.models_json;
        deserialize_models_json(&models_json)?;

        Ok(ResolvedPreviewState {
            models_json: Some(models_json),
            refreshed_at: Some(OffsetDateTime::now_utc()),
            preview_token: Some(preview_token),
        })
    }

    pub async fn validate_instance(
        &self,
        actor_user_id: Uuid,
        instance_id: Uuid,
    ) -> Result<ValidateModelProviderResult> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", self.use_case)?;
        let instance = self
            .repository
            .get_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        let installation = self.ready_installation(instance.installation_id).await?;
        if installation.availability_status != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::Conflict("plugin_installation_unavailable").into());
        }
        let package = load_provider_package(&installation.installed_path)?;
        let provider_config = self
            .build_provider_runtime_config(&package, &instance)
            .await?;
        ensure_model_provider_instance_transition(
            instance.status,
            domain::ModelProviderInstanceStatus::Ready,
            "validate_instance_success",
        )?;

        let validation_result = async {
            self.ensure_runtime_loaded(&installation).await?;
            let output = self
                .runtime
                .validate_provider(&installation, provider_config.clone())
                .await?;
            let models = self
                .runtime
                .list_models(&installation, provider_config)
                .await?;
            let next_status = derive_instance_status(false, &instance.enabled_model_ids);
            let now = OffsetDateTime::now_utc();
            let cache = self
                .repository
                .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                    provider_instance_id: instance.id,
                    model_discovery_mode: map_model_discovery_mode(
                        package.provider.model_discovery_mode,
                    ),
                    refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                    source: map_catalog_source(package.provider.model_discovery_mode),
                    models_json: serde_json::to_value(&models)?,
                    last_error_message: None,
                    refreshed_at: Some(now),
                })
                .await?;
            ensure_model_provider_instance_transition(
                instance.status,
                next_status,
                "validate_instance_success",
            )?;
            let updated_instance = self
                .repository
                .update_instance(&UpdateModelProviderInstanceInput {
                    instance_id: instance.id,
                    workspace_id: actor.current_workspace_id,
                    display_name: instance.display_name.clone(),
                    status: next_status,
                    config_json: instance.config_json.clone(),
                    configured_models: instance.configured_models.clone(),
                    enabled_model_ids: instance.enabled_model_ids.clone(),
                    included_in_main: instance.included_in_main,
                    updated_by: actor_user_id,
                })
                .await?;
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor_user_id),
                    "model_provider_instance",
                    Some(instance.id),
                    "model_provider.validated",
                    json!({
                        "provider_code": instance.provider_code,
                        "model_count": models.len(),
                    }),
                ))
                .await?;
            let masked_view = self
                .hydrate_instance_view(
                    updated_instance,
                    Some(cache.clone()),
                    &package.provider.form_schema,
                )
                .await?;
            Ok::<ValidateModelProviderResult, anyhow::Error>(ValidateModelProviderResult {
                instance: masked_view.instance,
                cache,
                output,
            })
        }
        .await;

        match validation_result {
            Ok(result) => Ok(result),
            Err(error) => {
                let existing_cache = self.repository.get_catalog_cache(instance.id).await?;
                let invalid_transition_allowed = ensure_model_provider_instance_transition(
                    instance.status,
                    domain::ModelProviderInstanceStatus::Invalid,
                    "validate_instance_failure",
                )
                .is_ok();
                let _ = self
                    .repository
                    .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                        provider_instance_id: instance.id,
                        model_discovery_mode: map_model_discovery_mode(
                            package.provider.model_discovery_mode,
                        ),
                        refresh_status: domain::ModelProviderCatalogRefreshStatus::Failed,
                        source: map_catalog_source(package.provider.model_discovery_mode),
                        models_json: existing_cache
                            .map(|cache| cache.models_json)
                            .unwrap_or_else(|| json!([])),
                        last_error_message: Some(error.to_string()),
                        refreshed_at: None,
                    })
                    .await;
                if invalid_transition_allowed {
                    let _ = self
                        .repository
                        .update_instance(&UpdateModelProviderInstanceInput {
                            instance_id: instance.id,
                            workspace_id: actor.current_workspace_id,
                            display_name: instance.display_name.clone(),
                            status: domain::ModelProviderInstanceStatus::Invalid,
                            config_json: instance.config_json.clone(),
                            configured_models: instance.configured_models.clone(),
                            enabled_model_ids: instance.enabled_model_ids.clone(),
                            included_in_main: instance.included_in_main,
                            updated_by: actor_user_id,
                        })
                        .await;
                }
                let _ = self
                    .repository
                    .append_audit_log(&audit_log(
                        Some(actor.current_workspace_id),
                        Some(actor_user_id),
                        "model_provider_instance",
                        Some(instance.id),
                        "model_provider.validate_failed",
                        json!({
                            "provider_code": instance.provider_code,
                            "message": error.to_string(),
                        }),
                    ))
                    .await;
                Err(error)
            }
        }
    }

    pub async fn preview_models(
        &self,
        command: PreviewModelProviderModelsCommand,
    ) -> Result<PreviewModelProviderModelsResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", self.use_case)?;

        let (installation, _package, provider_config, provider_code, audit_resource_id) =
            match command.instance_id {
                Some(instance_id) => {
                    let instance = self
                        .repository
                        .get_instance(actor.current_workspace_id, instance_id)
                        .await?
                        .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
                    let installation = self.ready_installation(instance.installation_id).await?;
                    if installation.availability_status
                        != domain::PluginAvailabilityStatus::Available
                    {
                        return Err(
                            ControlPlaneError::Conflict("plugin_installation_unavailable").into(),
                        );
                    }
                    let package = load_provider_package(&installation.installed_path)?;
                    let (patch_public_config, patch_secret_config) =
                        split_provider_config(&package.provider.form_schema, &command.config_json)?;
                    let current_secret_json = self
                        .repository
                        .get_secret_json(instance.id, &self.provider_secret_master_key)
                        .await?
                        .unwrap_or_else(empty_object);
                    let merged_public_config =
                        merge_json_object(&instance.config_json, &patch_public_config)?;
                    let merged_secret_config =
                        merge_json_object(&current_secret_json, &patch_secret_config)?;
                    validate_required_fields(
                        &package.provider.form_schema,
                        &merged_public_config,
                        &merged_secret_config,
                    )?;
                    let provider_config =
                        merge_json_object(&merged_public_config, &merged_secret_config)?;
                    (
                        installation,
                        package,
                        provider_config,
                        instance.provider_code,
                        Some(instance.id),
                    )
                }
                None => {
                    let installation_id = command
                        .installation_id
                        .ok_or(ControlPlaneError::InvalidInput("installation_id"))?;
                    let installation = self.ready_installation(installation_id).await?;
                    ensure_installation_assigned(
                        &self.repository,
                        actor.current_workspace_id,
                        installation_id,
                    )
                    .await?;
                    if matches!(
                        installation.desired_state,
                        domain::PluginDesiredState::Disabled
                    ) || installation.availability_status
                        != domain::PluginAvailabilityStatus::Available
                    {
                        return Err(
                            ControlPlaneError::Conflict("plugin_installation_unavailable").into(),
                        );
                    }
                    let package = load_provider_package(&installation.installed_path)?;
                    let (public_config, secret_config) =
                        split_provider_config(&package.provider.form_schema, &command.config_json)?;
                    validate_required_fields(
                        &package.provider.form_schema,
                        &public_config,
                        &secret_config,
                    )?;
                    let provider_config = merge_json_object(&public_config, &secret_config)?;
                    (
                        installation.clone(),
                        package,
                        provider_config,
                        installation.provider_code,
                        None,
                    )
                }
            };

        self.ensure_runtime_loaded(&installation).await?;
        let models = self
            .runtime
            .list_models(&installation, provider_config.clone())
            .await?;
        let expires_at = OffsetDateTime::now_utc() + time::Duration::minutes(10);
        let preview_token = Uuid::now_v7();
        self.repository
            .create_preview_session(&CreateModelProviderPreviewSessionInput {
                session_id: preview_token,
                workspace_id: actor.current_workspace_id,
                actor_user_id: command.actor_user_id,
                installation_id: Some(installation.id),
                instance_id: audit_resource_id,
                config_fingerprint: fingerprint_provider_config(&provider_config)?,
                models_json: serde_json::to_value(&models)?,
                expires_at,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider_instance",
                audit_resource_id,
                "model_provider.models_previewed",
                json!({
                    "provider_code": provider_code,
                    "model_count": models.len(),
                }),
            ))
            .await?;
        Ok(PreviewModelProviderModelsResult {
            models,
            preview_token,
            expires_at,
        })
    }

    pub async fn list_models(
        &self,
        actor_user_id: Uuid,
        instance_id: Uuid,
    ) -> Result<ModelProviderModelCatalog> {
        options::list_models(
            &self.repository,
            actor_user_id,
            instance_id,
            self.node_artifact_context(),
            self.use_case,
        )
        .await
    }

    pub async fn refresh_models(
        &self,
        actor_user_id: Uuid,
        instance_id: Uuid,
    ) -> Result<ModelProviderModelCatalog> {
        options::refresh_models(
            &self.repository,
            &self.runtime,
            &self.provider_secret_master_key,
            actor_user_id,
            instance_id,
            self.node_artifact_context(),
            self.use_case,
        )
        .await
    }

    pub async fn get_balance(
        &self,
        actor_user_id: Uuid,
        instance_id: Uuid,
    ) -> Result<ModelProviderBalanceResult> {
        balance::get_balance(
            &self.repository,
            &self.runtime,
            &self.provider_secret_master_key,
            actor_user_id,
            instance_id,
            self.node_artifact_context(),
            self.use_case,
        )
        .await
    }

    pub async fn delete_instance(&self, command: DeleteModelProviderInstanceCommand) -> Result<()> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", self.use_case)?;
        let instance = self
            .repository
            .get_instance(actor.current_workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        let reference_count = self
            .repository
            .count_instance_references(actor.current_workspace_id, command.instance_id)
            .await?;
        if reference_count > 0 {
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "model_provider_instance",
                    Some(instance.id),
                    "model_provider.delete_conflict",
                    json!({
                        "provider_code": instance.provider_code,
                        "reference_count": reference_count,
                    }),
                ))
                .await?;
            return Err(ControlPlaneError::Conflict("model_provider_in_use").into());
        }

        self.repository
            .delete_instance(actor.current_workspace_id, command.instance_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider_instance",
                Some(instance.id),
                "model_provider.deleted",
                json!({
                    "provider_code": instance.provider_code,
                }),
            ))
            .await?;
        Ok(())
    }

    pub async fn update_main_instance(
        &self,
        command: UpdateModelProviderMainInstanceCommand,
    ) -> Result<ModelProviderMainInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", self.use_case)?;
        let updated = main_instance::update_main_instance(
            &self.repository,
            actor.current_workspace_id,
            &command,
        )
        .await?;

        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider",
                None,
                "model_provider.main_instance_updated",
                json!({
                    "provider_code": updated.provider_code,
                    "auto_include_new_instances": updated.auto_include_new_instances,
                }),
            ))
            .await?;

        Ok(updated)
    }

    pub async fn options(
        &self,
        actor_user_id: Uuid,
        locales: RequestedLocales,
    ) -> Result<ModelProviderOptionsView> {
        catalog::options(
            &self.repository,
            actor_user_id,
            locales,
            self.node_artifact_context(),
            self.use_case,
        )
        .await
    }

    pub async fn reveal_secret(
        &self,
        actor_user_id: Uuid,
        instance_id: Uuid,
        key: &str,
    ) -> Result<String> {
        options::reveal_secret(
            &self.repository,
            &self.provider_secret_master_key,
            actor_user_id,
            instance_id,
            key,
            self.node_artifact_context(),
            self.use_case,
        )
        .await
    }

    async fn build_provider_runtime_config(
        &self,
        package: &ProviderPackage,
        instance: &domain::ModelProviderInstanceRecord,
    ) -> Result<Value> {
        build_provider_runtime_config(
            &self.repository,
            &self.provider_secret_master_key,
            package,
            instance,
        )
        .await
    }

    async fn ensure_runtime_loaded(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<()> {
        match self.runtime.ensure_loaded(installation).await {
            Ok(()) => {
                self.mark_current_node_runtime_status(
                    installation,
                    domain::PluginRuntimeStatus::Active,
                    None,
                )
                .await?;
                Ok(())
            }
            Err(error) => {
                let _ = self
                    .mark_current_node_runtime_status(
                        installation,
                        domain::PluginRuntimeStatus::LoadFailed,
                        Some(error.to_string()),
                    )
                    .await;
                Err(error)
            }
        }
    }

    async fn mark_current_node_runtime_status(
        &self,
        installation: &domain::PluginInstallationRecord,
        runtime_status: domain::PluginRuntimeStatus,
        last_error: Option<String>,
    ) -> Result<()> {
        let Some(context) = self.node_artifact_context() else {
            return Ok(());
        };
        mark_current_node_plugin_runtime_status(
            &self.repository,
            context.node_id,
            installation,
            runtime_status,
            last_error,
        )
        .await?;
        Ok(())
    }

    async fn hydrate_instance_view(
        &self,
        instance: domain::ModelProviderInstanceRecord,
        cache: Option<domain::ModelProviderCatalogCacheRecord>,
        form_schema: &[ProviderConfigField],
    ) -> Result<ModelProviderInstanceView> {
        hydrate_instance_view(
            &self.repository,
            &self.provider_secret_master_key,
            instance,
            cache,
            form_schema,
        )
        .await
    }
}

fn normalize_enabled_model_ids(enabled_model_ids: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for model_id in enabled_model_ids {
        let trimmed = model_id.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

fn normalize_configured_models(
    configured_models: Vec<domain::ModelProviderConfiguredModel>,
    enabled_model_ids: Vec<String>,
) -> Vec<domain::ModelProviderConfiguredModel> {
    if configured_models.is_empty() {
        return normalize_enabled_model_ids(enabled_model_ids)
            .into_iter()
            .map(|model_id| domain::ModelProviderConfiguredModel {
                model_id,
                enabled: true,
                context_window_override_tokens: None,
                supports_multimodal: None,
            })
            .collect();
    }

    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for configured_model in configured_models {
        let trimmed = configured_model.model_id.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        normalized.push(domain::ModelProviderConfiguredModel {
            model_id: trimmed.to_string(),
            enabled: configured_model.enabled,
            context_window_override_tokens: configured_model.context_window_override_tokens,
            supports_multimodal: configured_model.supports_multimodal,
        });
    }
    normalized
}

fn configured_models_to_enabled_model_ids(
    configured_models: &[domain::ModelProviderConfiguredModel],
) -> Vec<String> {
    configured_models
        .iter()
        .filter(|configured_model| configured_model.enabled)
        .map(|configured_model| configured_model.model_id.clone())
        .collect()
}

fn derive_instance_status(
    disabled_instance: bool,
    enabled_model_ids: &[String],
) -> domain::ModelProviderInstanceStatus {
    if disabled_instance {
        domain::ModelProviderInstanceStatus::Disabled
    } else if enabled_model_ids.is_empty() {
        domain::ModelProviderInstanceStatus::Draft
    } else {
        domain::ModelProviderInstanceStatus::Ready
    }
}

fn fingerprint_provider_config(config: &Value) -> Result<String> {
    Ok(serde_json::to_string(&canonicalize_json(config))?)
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, entry)| (key.clone(), canonicalize_json(entry)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        _ => value.clone(),
    }
}

fn deserialize_models_json(models_json: &Value) -> Result<Vec<ProviderModelDescriptor>> {
    Ok(serde_json::from_value(models_json.clone())?)
}
