use std::{collections::HashSet, path::PathBuf};

use anyhow::Result;
use plugin_framework::data_source_contract::{
    DataSourceCatalogEntry, DataSourceConfigInput, DataSourceDescribeResourceInput,
    DataSourcePreviewReadInput, DataSourcePreviewReadOutput, DataSourceResourceDescriptor,
};
use plugin_framework::provider_contract::PluginFormFieldSchema;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    plugin_management::{
        mark_current_node_plugin_runtime_status, ready_current_node_plugin_installation,
    },
    ports::{
        AddModelFieldInput, AuthRepository, CreateDataSourceInstanceInput,
        CreateDataSourcePreviewSessionInput, CreateModelDefinitionInput,
        CreateScopeDataModelGrantInput, DataSourceInstanceVisibility, DataSourceRepository,
        DataSourceRuntimePort, ModelDefinitionRepository, PluginRepository,
        RotateDataSourceSecretInput, UpdateDataSourceDefaultsInput,
        UpdateDataSourceInstanceStatusInput, UpdateMainSourceDefaultsInput,
        UpsertDataSourceCatalogCacheInput, UpsertDataSourceSecretInput,
    },
};

mod instance_config;

use instance_config::{
    classify_data_source_config, load_data_source_config_schema,
    validate_data_source_secret_rotation,
};

#[derive(Debug, Clone)]
pub struct CreateDataSourceInstanceCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub installation_id: Uuid,
    pub source_code: String,
    pub display_name: String,
    pub config_json: Value,
    pub secret_json: Value,
}

#[derive(Debug, Clone)]
pub struct ValidateDataSourceInstanceCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct DiscoverDataSourceResourcesCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct UpdateDataSourceDefaultsCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub defaults: domain::DataSourceDefaults,
}

#[derive(Debug, Clone)]
pub struct UpdateMainDataSourceDefaultsCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub defaults: domain::DataSourceDefaults,
}

#[derive(Debug, Clone)]
pub struct RotateDataSourceSecretCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub secret_json: Value,
}

#[derive(Debug, Clone)]
pub struct PreviewDataSourceReadCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub resource_key: String,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub options_json: Value,
}

#[derive(Debug, Clone)]
pub struct MapDataSourceResourceToModelCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub resource_key: String,
}

#[derive(Debug, Clone)]
pub struct DataSourceInstanceView {
    pub instance: domain::DataSourceInstanceRecord,
    pub catalog: Option<domain::DataSourceCatalogCacheRecord>,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum DataSourceBackendView {
    Core {
        defaults: domain::DataSourceDefaults,
    },
    RuntimeExtension(DataSourceInstanceView),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataSourceCapabilitiesView {
    pub can_update_defaults: bool,
    pub can_create_data_model: bool,
    pub can_validate: bool,
    pub can_discover_resources: bool,
    pub can_preview_resources: bool,
    pub can_map_resources: bool,
}

#[derive(Debug, Clone)]
pub struct DataSourceView {
    pub backend: DataSourceBackendView,
}

impl DataSourceView {
    pub fn capabilities(&self) -> DataSourceCapabilitiesView {
        match &self.backend {
            DataSourceBackendView::Core { .. } => DataSourceCapabilitiesView {
                can_update_defaults: true,
                can_create_data_model: true,
                can_validate: false,
                can_discover_resources: false,
                can_preview_resources: false,
                can_map_resources: false,
            },
            DataSourceBackendView::RuntimeExtension(view) => {
                let ready = view.instance.status == domain::DataSourceInstanceStatus::Ready;
                DataSourceCapabilitiesView {
                    can_update_defaults: true,
                    can_create_data_model: false,
                    can_validate: true,
                    can_discover_resources: ready,
                    can_preview_resources: ready,
                    can_map_resources: ready,
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DataSourceCatalogEntryView {
    pub installation_id: Uuid,
    pub source_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub display_name: String,
    pub protocol: String,
    pub config_schema: Vec<PluginFormFieldSchema>,
}

#[derive(Debug, Clone)]
pub struct ValidateDataSourceInstanceResult {
    pub instance: domain::DataSourceInstanceRecord,
    pub output: Value,
}

#[derive(Debug, Clone)]
pub struct DataSourceResourcesView {
    pub entries: Vec<DataSourceCatalogEntry>,
    pub refresh_status: domain::DataSourceCatalogRefreshStatus,
    pub last_error_message: Option<String>,
    pub refreshed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct PreviewDataSourceReadResult {
    pub preview_session: domain::DataSourcePreviewSessionRecord,
    pub output: DataSourcePreviewReadOutput,
}

#[derive(Debug, Clone)]
pub struct MapDataSourceResourceToModelResult {
    pub model: domain::ModelDefinitionRecord,
    pub fields: Vec<domain::ModelFieldRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlDataSourceOption {
    pub data_source_instance_id: String,
    pub display_name: String,
    pub capability: String,
}

pub struct DataSourceService<R, H> {
    repository: R,
    runtime: H,
    secret_master_key: String,
    node_id: Option<String>,
    install_root: Option<PathBuf>,
}

impl<R, H> DataSourceService<R, H>
where
    R: AuthRepository + PluginRepository + DataSourceRepository + ModelDefinitionRepository,
    H: DataSourceRuntimePort,
{
    pub fn new(repository: R, runtime: H, secret_master_key: impl Into<String>) -> Self {
        Self {
            repository,
            runtime,
            secret_master_key: secret_master_key.into(),
            node_id: None,
            install_root: None,
        }
    }

    pub fn for_data_model_settings(
        repository: R,
        runtime: H,
        secret_master_key: impl Into<String>,
    ) -> Self {
        Self::new(repository, runtime, secret_master_key)
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

    async fn ready_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<domain::LocalPluginInstallationRecord> {
        match (self.node_id.as_deref(), self.install_root.as_deref()) {
            (Some(node_id), Some(install_root)) => {
                ready_current_node_plugin_installation(
                    &self.repository,
                    node_id,
                    install_root,
                    installation_id,
                )
                .await
            }
            _ => Err(ControlPlaneError::Conflict("plugin_node_context_required").into()),
        }
    }

    async fn ensure_console_simple_operation(
        &self,
        actor: &domain::ActorContext,
        group: &domain::ConsolePolicyGroup,
        operation_id: &str,
    ) -> Result<()> {
        if actor.is_root {
            return Ok(());
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
            .await?;
        ensure_console_simple_operation(&policies, group, operation_id).map_err(Into::into)
    }

    async fn data_source_view_visibility(
        &self,
        actor: &domain::ActorContext,
    ) -> Result<DataSourceInstanceVisibility> {
        if actor.is_root {
            return Ok(DataSourceInstanceVisibility::ScopeAll);
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
            .await?;
        resolve_data_source_instance_visibility(&policies).map_err(Into::into)
    }

    async fn data_source_list_visibility(
        &self,
        actor: &domain::ActorContext,
    ) -> Result<DataSourceInstanceVisibility> {
        if actor.is_root {
            return Ok(DataSourceInstanceVisibility::ScopeAll);
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
            .await?;
        ensure_console_simple_operation(
            &policies,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_LIST_OPERATION_ID,
        )?;
        resolve_data_source_instance_visibility(&policies).map_err(Into::into)
    }

    pub async fn list_catalog(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<DataSourceCatalogEntryView>> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_workspace_matches(&actor, workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION,
        )
        .await?;

        let assigned_installation_ids = self
            .repository
            .list_assignments(workspace_id)
            .await?
            .into_iter()
            .map(|assignment| assignment.installation_id)
            .collect::<HashSet<_>>();

        let installations = self
            .repository
            .list_installations()
            .await?
            .into_iter()
            .filter(|installation| installation.contract_version == "1flowbase.data_source/v1")
            .filter(|installation| assigned_installation_ids.contains(&installation.id))
            .collect::<Vec<_>>();
        let mut entries = Vec::with_capacity(installations.len());
        for installation in installations {
            let installation = self.ready_installation(installation.id).await?;
            let installed_path = installation
                .local_path()
                .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?
                .to_string();
            let package = tokio::task::spawn_blocking(move || {
                plugin_framework::DataSourcePackage::load_from_dir(installed_path)
            })
            .await??;
            if package.definition.source_code != installation.provider_code {
                return Err(ControlPlaneError::InvalidInput("source_code").into());
            }
            entries.push(DataSourceCatalogEntryView {
                installation_id: installation.id,
                source_code: installation.installation.provider_code,
                plugin_id: installation.installation.plugin_id,
                plugin_version: installation.installation.plugin_version,
                display_name: installation.installation.display_name,
                protocol: installation.installation.protocol,
                config_schema: package.definition.config_schema,
            });
        }
        entries.sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Ok(entries)
    }

    pub async fn list_instances(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<DataSourceInstanceView>> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_workspace_matches(&actor, workspace_id)?;
        let visibility = self.data_source_view_visibility(&actor).await?;

        Ok(self
            .repository
            .list_instances(workspace_id, actor.user_id, visibility)
            .await?
            .into_iter()
            .map(|instance| DataSourceInstanceView {
                instance,
                catalog: None,
            })
            .collect())
    }

    pub async fn list_data_sources(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<DataSourceView>> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_workspace_matches(&actor, workspace_id)?;
        // `data_sources.list` opens this endpoint, while the registered `data_sources.view`
        // resource action controls which persisted instance rows are visible.
        let visibility = self.data_source_list_visibility(&actor).await?;

        let defaults =
            DataSourceRepository::get_main_source_defaults(&self.repository, workspace_id).await?;
        let instances = self
            .repository
            .list_instances(workspace_id, actor.user_id, visibility)
            .await?;
        let mut data_sources = Vec::with_capacity(instances.len() + 1);
        data_sources.push(DataSourceView {
            backend: DataSourceBackendView::Core { defaults },
        });
        data_sources.extend(instances.into_iter().map(|instance| DataSourceView {
            backend: DataSourceBackendView::RuntimeExtension(DataSourceInstanceView {
                instance,
                catalog: None,
            }),
        }));
        Ok(data_sources)
    }

    pub async fn list_native_sql_options(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<NativeSqlDataSourceOption>> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_workspace_matches(&actor, workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &domain::ConsolePolicyGroup::other("other.agent-flow")
                .expect("compiled agent-flow policy group must be valid"),
            access_control::AGENT_FLOW_DATA_SOURCE_OPTIONS_LIST_OPERATION_ID,
        )
        .await?;
        let visibility = self.data_source_view_visibility(&actor).await?;
        let assigned_installation_ids = self
            .repository
            .list_assignments(workspace_id)
            .await?
            .into_iter()
            .map(|assignment| assignment.installation_id)
            .collect::<HashSet<_>>();
        let instances = self
            .repository
            .list_instances(workspace_id, actor.user_id, visibility)
            .await?;

        let mut options = vec![NativeSqlDataSourceOption {
            data_source_instance_id: "main".to_string(),
            display_name: "主数据源".to_string(),
            capability: plugin_framework::DATA_SOURCE_NATIVE_SQL_CAPABILITY.to_string(),
        }];
        for instance in instances {
            if instance.status != domain::DataSourceInstanceStatus::Ready
                || !assigned_installation_ids.contains(&instance.installation_id)
            {
                continue;
            }
            let installation = self.ready_installation(instance.installation_id).await?;
            if installation.desired_state == domain::PluginDesiredState::Disabled
                || installation.availability_status() != domain::PluginAvailabilityStatus::Available
                || installation.contract_version != "1flowbase.data_source/v1"
                || installation.provider_code != instance.source_code
            {
                continue;
            }
            let installed_path = installation
                .local_path()
                .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?
                .to_string();
            let package = tokio::task::spawn_blocking(move || {
                plugin_framework::DataSourcePackage::load_from_dir(installed_path)
            })
            .await??;
            if !package.supports_native_sql() {
                continue;
            }
            options.push(NativeSqlDataSourceOption {
                data_source_instance_id: instance.id.to_string(),
                display_name: instance.display_name,
                capability: plugin_framework::DATA_SOURCE_NATIVE_SQL_CAPABILITY.to_string(),
            });
        }
        options[1..].sort_by(|left, right| left.display_name.cmp(&right.display_name));
        Ok(options)
    }

    pub async fn create_instance(
        &self,
        command: CreateDataSourceInstanceCommand,
    ) -> Result<DataSourceInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_CREATE_OPERATION_ID,
        )
        .await?;

        let installation = self.ready_installation(command.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            command.workspace_id,
            command.installation_id,
        )
        .await?;
        ensure_data_source_installation(&installation, &command.source_code)?;

        let config_schema = load_data_source_config_schema(&installation).await?;

        let instance_id = Uuid::now_v7();
        let secret_ref = domain::data_source_secret_ref(instance_id);
        let (config_json, secret_json) = classify_data_source_config(
            &config_schema,
            &command.config_json,
            &command.secret_json,
            &secret_ref,
            1,
        )?;

        let instance = self
            .repository
            .create_instance(&CreateDataSourceInstanceInput {
                instance_id,
                workspace_id: command.workspace_id,
                installation_id: command.installation_id,
                source_code: normalize_required_text(&command.source_code, "source_code")?,
                display_name: normalize_required_text(&command.display_name, "display_name")?,
                status: domain::DataSourceInstanceStatus::Draft,
                config_json,
                metadata_json: json!({}),
                defaults: domain::DataSourceDefaults::default(),
                created_by: actor.user_id,
            })
            .await?;

        let instance = if secret_json
            .as_object()
            .is_some_and(|secrets| !secrets.is_empty())
        {
            self.repository
                .upsert_secret(&UpsertDataSourceSecretInput {
                    data_source_instance_id: instance.id,
                    secret_ref,
                    plaintext_secret_json: secret_json,
                    master_key: self.secret_master_key.clone(),
                    secret_version: 1,
                })
                .await?;
            self.repository
                .get_instance(command.workspace_id, instance.id)
                .await?
                .unwrap_or(instance)
        } else {
            instance
        };

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.instance_created",
                json!({
                    "installation_id": command.installation_id,
                    "source_code": instance.source_code,
                    "secret_ref": instance.secret_ref.clone(),
                    "secret_version": instance.secret_version,
                }),
            ),
        )
        .await?;

        Ok(DataSourceInstanceView {
            instance,
            catalog: None,
        })
    }

    pub async fn validate_instance(
        &self,
        command: ValidateDataSourceInstanceCommand,
    ) -> Result<ValidateDataSourceInstanceResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_VALIDATE_OPERATION_ID,
        )
        .await?;

        let existing = self
            .repository
            .get_instance(command.workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        let installation = self.ready_installation(existing.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            command.workspace_id,
            existing.installation_id,
        )
        .await?;

        let secret_json = self
            .repository
            .get_secret_json(existing.id, &self.secret_master_key)
            .await?
            .unwrap_or_else(|| json!({}));

        self.ensure_runtime_loaded(&installation).await?;
        let secret_values = collect_secret_strings(&secret_json);
        let output = self
            .runtime
            .validate_config(
                &installation,
                existing.config_json.clone(),
                secret_json.clone(),
            )
            .await?;
        let output = redact_value(&output, &secret_values);
        self.runtime
            .test_connection(
                &installation,
                existing.config_json.clone(),
                secret_json.clone(),
            )
            .await?;

        let instance = self
            .repository
            .update_instance_status(&UpdateDataSourceInstanceStatusInput {
                workspace_id: command.workspace_id,
                instance_id: existing.id,
                status: domain::DataSourceInstanceStatus::Ready,
                metadata_json: existing.metadata_json.clone(),
                updated_by: actor.user_id,
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.instance_validated",
                json!({
                    "status": instance.status.as_str(),
                }),
            ),
        )
        .await?;

        Ok(ValidateDataSourceInstanceResult { instance, output })
    }

    pub async fn update_defaults(
        &self,
        command: UpdateDataSourceDefaultsCommand,
    ) -> Result<domain::DataSourceInstanceRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID,
        )
        .await?;
        ensure_data_source_defaults_compatible(command.defaults)?;

        let instance = self
            .repository
            .update_instance_defaults(&UpdateDataSourceDefaultsInput {
                workspace_id: command.workspace_id,
                instance_id: command.instance_id,
                defaults: command.defaults,
                updated_by: actor.user_id,
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.defaults_updated",
                json!({
                    "default_data_model_status": instance.defaults.data_model_status.as_str(),
                }),
            ),
        )
        .await?;

        Ok(instance)
    }

    pub async fn get_main_source_defaults(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<domain::DataSourceDefaults> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_workspace_matches(&actor, workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_LIST_OPERATION_ID,
        )
        .await?;
        DataSourceRepository::get_main_source_defaults(&self.repository, workspace_id).await
    }

    pub async fn update_main_data_source_defaults(
        &self,
        command: UpdateMainDataSourceDefaultsCommand,
    ) -> Result<domain::DataSourceDefaults> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID,
        )
        .await?;
        ensure_data_source_defaults_compatible(command.defaults)?;

        let defaults = self
            .repository
            .update_main_source_defaults(&UpdateMainSourceDefaultsInput {
                workspace_id: command.workspace_id,
                defaults: command.defaults,
                updated_by: actor.user_id,
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                None,
                "data_source.main_source_defaults_updated",
                json!({
                    "default_data_model_status": defaults.data_model_status.as_str(),
                }),
            ),
        )
        .await?;

        Ok(defaults)
    }

    pub async fn list_resources(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<DataSourceResourcesView> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_workspace_matches(&actor, workspace_id)?;
        let visibility = self.data_source_view_visibility(&actor).await?;

        let instance = self
            .repository
            .get_instance_for_visibility(workspace_id, instance_id, actor.user_id, visibility)
            .await?
            .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        ensure_ready_connection(&instance, "list_resources")?;

        let Some(cache) = self
            .repository
            .get_catalog_cache(workspace_id, instance_id)
            .await?
        else {
            return Ok(DataSourceResourcesView {
                entries: Vec::new(),
                refresh_status: domain::DataSourceCatalogRefreshStatus::Idle,
                last_error_message: None,
                refreshed_at: None,
            });
        };
        let entries = serde_json::from_value(cache.catalog_json)?;
        Ok(DataSourceResourcesView {
            entries,
            refresh_status: cache.refresh_status,
            last_error_message: cache.last_error_message,
            refreshed_at: cache.refreshed_at,
        })
    }

    pub async fn discover_resources(
        &self,
        command: DiscoverDataSourceResourcesCommand,
    ) -> Result<DataSourceResourcesView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_DISCOVER_OPERATION_ID,
        )
        .await?;

        let instance = self
            .repository
            .get_instance(command.workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        ensure_ready_connection(&instance, "discover_resources")?;
        let installation = self.ready_installation(instance.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            command.workspace_id,
            instance.installation_id,
        )
        .await?;

        let secret_json = self
            .repository
            .get_secret_json(instance.id, &self.secret_master_key)
            .await?
            .unwrap_or_else(|| json!({}));
        let secret_values = collect_secret_strings(&secret_json);
        self.ensure_runtime_loaded(&installation).await?;
        let catalog_json = self
            .runtime
            .discover_catalog(&installation, instance.config_json, secret_json)
            .await?;
        let catalog_json = redact_value(&catalog_json, &secret_values);
        let entries: Vec<DataSourceCatalogEntry> = serde_json::from_value(catalog_json.clone())?;
        let cache = self
            .repository
            .upsert_catalog_cache(&UpsertDataSourceCatalogCacheInput {
                data_source_instance_id: instance.id,
                refresh_status: domain::DataSourceCatalogRefreshStatus::Ready,
                catalog_json,
                last_error_message: None,
                refreshed_at: Some(OffsetDateTime::now_utc()),
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.resources_discovered",
                json!({ "resource_count": entries.len() }),
            ),
        )
        .await?;

        Ok(DataSourceResourcesView {
            entries,
            refresh_status: cache.refresh_status,
            last_error_message: cache.last_error_message,
            refreshed_at: cache.refreshed_at,
        })
    }

    pub async fn rotate_secret(
        &self,
        command: RotateDataSourceSecretCommand,
    ) -> Result<DataSourceInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_sources_other_console_policy_group(),
            access_control::DATA_SOURCES_SECRET_ROTATE_OPERATION_ID,
        )
        .await?;

        let existing = self
            .repository
            .get_instance(command.workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        let installation = self.ready_installation(existing.installation_id).await?;
        ensure_data_source_installation(&installation, &existing.source_code)?;
        let config_schema = load_data_source_config_schema(&installation).await?;
        let secret_ref = existing
            .secret_ref
            .clone()
            .unwrap_or_else(|| domain::data_source_secret_ref(existing.id));
        let secret_json =
            validate_data_source_secret_rotation(&config_schema, &command.secret_json)?;

        let secret = self
            .repository
            .rotate_secret(&RotateDataSourceSecretInput {
                workspace_id: command.workspace_id,
                data_source_instance_id: existing.id,
                secret_ref: secret_ref.clone(),
                plaintext_secret_json: secret_json,
                master_key: self.secret_master_key.clone(),
                updated_by: actor.user_id,
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(secret.instance.id),
                "data_source.secret_rotated",
                json!({
                    "secret_ref": secret_ref,
                    "secret_version": secret.secret.secret_version,
                }),
            ),
        )
        .await?;

        Ok(DataSourceInstanceView {
            instance: secret.instance,
            catalog: None,
        })
    }

    pub async fn map_resource_to_model(
        &self,
        command: MapDataSourceResourceToModelCommand,
    ) -> Result<MapDataSourceResourceToModelResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID,
        )
        .await?;

        let instance = self
            .repository
            .get_instance(command.workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        ensure_ready_connection(&instance, "map_resource_to_model")?;
        let installation = self.ready_installation(instance.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            command.workspace_id,
            instance.installation_id,
        )
        .await?;

        let resource_key = normalize_required_text(&command.resource_key, "resource_key")?;
        let secret_json = self
            .repository
            .get_secret_json(instance.id, &self.secret_master_key)
            .await?
            .unwrap_or_else(|| json!({}));
        let secret_values = collect_secret_strings(&secret_json);
        self.ensure_runtime_loaded(&installation).await?;
        let descriptor = self
            .runtime
            .describe_resource(
                &installation,
                DataSourceDescribeResourceInput {
                    connection: DataSourceConfigInput {
                        config_json: instance.config_json.clone(),
                        secret_json,
                    },
                    resource_key,
                },
            )
            .await?;
        let descriptor = redact_value(&serde_json::to_value(descriptor)?, &secret_values);
        let descriptor: DataSourceResourceDescriptor = serde_json::from_value(descriptor)?;

        let descriptor_resource_key =
            normalize_required_text(&descriptor.resource_key, "external_resource_key")?;
        let defaults = instance.defaults;
        let status = defaults.data_model_status;
        let model_code = normalize_code_identifier(&descriptor_resource_key, "resource_key")?;
        let model_title = descriptor
            .metadata
            .get("display_name")
            .or_else(|| descriptor.metadata.get("title"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&descriptor_resource_key)
            .to_string();

        let model = self
            .repository
            .create_model_definition(&CreateModelDefinitionInput {
                actor_user_id: actor.user_id,
                scope_kind: domain::DataModelScopeKind::System,
                scope_id: domain::SYSTEM_SCOPE_ID,
                data_source_instance_id: Some(instance.id),
                source_kind: domain::DataModelSourceKind::ExternalSource,
                external_resource_key: Some(descriptor_resource_key.clone()),
                external_table_id: None,
                external_capability_snapshot: Some(serde_json::to_value(&descriptor.capabilities)?),
                code: model_code,
                title: model_title,
                status,
                protection: domain::DataModelProtection::default(),
            })
            .await?;

        self.repository
            .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
                grant_id: Uuid::now_v7(),
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: command.workspace_id,
                data_model_id: model.id,
                enabled: true,
                permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
                created_by: Some(actor.user_id),
            })
            .await?;

        let mut fields = Vec::new();
        for schema in descriptor.fields {
            let external_field_key = normalize_required_text(&schema.key, "external_field_key")?;
            let field = self
                .repository
                .add_model_field(&AddModelFieldInput {
                    actor_user_id: actor.user_id,
                    model_id: model.id,
                    physical_column_name: None,
                    code: normalize_code_identifier(&external_field_key, "external_field_key")?,
                    title: field_title(&schema),
                    description: schema.description.clone(),
                    external_field_key: Some(external_field_key),
                    field_kind: model_field_kind_from_schema(&schema),
                    is_system: false,
                    is_writable: true,
                    apply_physical_schema: false,
                    is_required: schema.required.unwrap_or(false),
                    api_required: schema.required.unwrap_or(false),
                    is_unique: descriptor
                        .primary_key
                        .as_deref()
                        .map(|primary_key| primary_key == schema.key)
                        .unwrap_or(false),
                    default_value: schema.default_value.clone(),
                    display_interface: schema.control.clone(),
                    display_options: serde_json::to_value(&schema)?,
                    relation_target_model_id: None,
                    relation_options: json!({}),
                })
                .await?;
            fields.push(field);
        }

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "state_model",
                Some(model.id),
                "data_source.resource_mapped_to_model",
                json!({
                    "data_source_instance_id": instance.id,
                    "resource_key": descriptor_resource_key,
                    "field_count": fields.len(),
                }),
            ),
        )
        .await?;

        Ok(MapDataSourceResourceToModelResult { model, fields })
    }

    pub async fn preview_read(
        &self,
        command: PreviewDataSourceReadCommand,
    ) -> Result<PreviewDataSourceReadResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_PREVIEW_OPERATION_ID,
        )
        .await?;

        let instance = self
            .repository
            .get_instance(command.workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        ensure_ready_connection(&instance, "preview_read")?;
        let installation = self.ready_installation(instance.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            command.workspace_id,
            instance.installation_id,
        )
        .await?;

        let secret_json = self
            .repository
            .get_secret_json(instance.id, &self.secret_master_key)
            .await?
            .unwrap_or_else(|| json!({}));
        let secret_values = collect_secret_strings(&secret_json);
        let preview_input = DataSourcePreviewReadInput {
            connection: DataSourceConfigInput {
                config_json: instance.config_json.clone(),
                secret_json,
            },
            resource_key: normalize_required_text(&command.resource_key, "resource_key")?,
            limit: command.limit,
            cursor: command.cursor,
            options_json: command.options_json,
        };
        self.ensure_runtime_loaded(&installation).await?;
        let output = self
            .runtime
            .preview_read(&installation, preview_input.clone())
            .await?;
        let output = redact_preview_output(output, &secret_values);
        let preview_json = serde_json::to_value(&output)?;
        let preview_session = self
            .repository
            .create_preview_session(&CreateDataSourcePreviewSessionInput {
                session_id: Uuid::now_v7(),
                workspace_id: command.workspace_id,
                actor_user_id: actor.user_id,
                data_source_instance_id: Some(instance.id),
                config_fingerprint: build_preview_fingerprint(&preview_input, &secret_values)?,
                preview_json,
                expires_at: OffsetDateTime::now_utc() + Duration::minutes(15),
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.preview_read",
                json!({
                    "resource_key": preview_input.resource_key,
                }),
            ),
        )
        .await?;

        Ok(PreviewDataSourceReadResult {
            preview_session,
            output,
        })
    }

    async fn ensure_runtime_loaded(
        &self,
        installation: &domain::LocalPluginInstallationRecord,
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
        let Some(node_id) = self.node_id.as_deref() else {
            return Ok(());
        };
        if self.install_root.is_none() {
            return Ok(());
        }
        mark_current_node_plugin_runtime_status(
            &self.repository,
            node_id,
            installation,
            runtime_status,
            last_error,
        )
        .await?;
        Ok(())
    }
}

async fn load_actor_context_for_user<R>(
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

fn ensure_workspace_matches(actor: &domain::ActorContext, workspace_id: Uuid) -> Result<()> {
    if actor.current_workspace_id == workspace_id {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("workspace_id").into())
    }
}

fn data_models_console_policy_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID,
    )
    .expect("compiled data models settings feature id must be valid")
}

fn data_sources_other_console_policy_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::other("other.data-sources")
        .expect("compiled data source other group id must be valid")
}

fn ensure_console_simple_operation(
    policies: &[domain::RoleConsolePolicy],
    group: &domain::ConsolePolicyGroup,
    operation_id: &str,
) -> Result<(), ControlPlaneError> {
    let operation_id = domain::ConsoleOperationId::try_from(operation_id)
        .expect("compiled data source simple operation id must be valid");
    if domain::effective_console_simple_operation(policies, group, &operation_id) {
        Ok(())
    } else {
        Err(ControlPlaneError::PermissionDenied("permission_denied"))
    }
}

fn resolve_data_source_instance_visibility(
    policies: &[domain::RoleConsolePolicy],
) -> Result<DataSourceInstanceVisibility, ControlPlaneError> {
    let operation_id =
        domain::ConsoleOperationId::try_from(access_control::DATA_SOURCES_VIEW_OPERATION_ID)
            .expect("compiled data source view operation id must be valid");
    match domain::effective_console_row_scope(
        policies,
        &data_models_console_policy_group(),
        &operation_id,
    ) {
        domain::ConsoleOperationRowScope::ScopeAll => Ok(DataSourceInstanceVisibility::ScopeAll),
        domain::ConsoleOperationRowScope::Own => Ok(DataSourceInstanceVisibility::Own),
        domain::ConsoleOperationRowScope::Disabled => {
            Err(ControlPlaneError::PermissionDenied("permission_denied"))
        }
    }
}

async fn ensure_installation_assigned<R>(
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

fn ensure_data_source_installation(
    installation: &domain::PluginInstallationRecord,
    source_code: &str,
) -> Result<()> {
    if installation.contract_version != "1flowbase.data_source/v1" {
        return Err(ControlPlaneError::InvalidInput("plugin_installation").into());
    }
    if installation.provider_code != source_code {
        return Err(ControlPlaneError::InvalidInput("source_code").into());
    }
    Ok(())
}

fn ensure_ready_connection(
    instance: &domain::DataSourceInstanceRecord,
    action: &'static str,
) -> Result<()> {
    if instance.status == domain::DataSourceInstanceStatus::Ready {
        return Ok(());
    }

    Err(ControlPlaneError::InvalidStateTransition {
        resource: "data_source_instance",
        action,
        from: instance.status.as_str().to_string(),
        to: domain::DataSourceInstanceStatus::Ready.as_str().to_string(),
    }
    .into())
}

fn normalize_required_text(value: &str, field: &'static str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ControlPlaneError::InvalidInput(field).into())
    } else {
        Ok(trimmed.to_string())
    }
}

fn normalize_code_identifier(value: &str, field: &'static str) -> Result<String> {
    let mut code = String::new();
    let mut last_was_separator = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            code.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            code.push('_');
            last_was_separator = true;
        }
    }
    let code = code.trim_matches('_').to_string();
    if code.is_empty() {
        Err(ControlPlaneError::InvalidInput(field).into())
    } else {
        Ok(code)
    }
}

fn field_title(schema: &PluginFormFieldSchema) -> String {
    let label = schema.label.trim();
    if label.is_empty() {
        schema.key.clone()
    } else {
        label.to_string()
    }
}

fn model_field_kind_from_schema(schema: &PluginFormFieldSchema) -> domain::ModelFieldKind {
    match schema.field_type.trim().to_ascii_lowercase().as_str() {
        "number" | "integer" | "float" | "decimal" => domain::ModelFieldKind::Number,
        "boolean" | "bool" => domain::ModelFieldKind::Boolean,
        "datetime" | "date_time" | "timestamp" | "date" => domain::ModelFieldKind::Datetime,
        "enum" | "select" | "multi_select" => domain::ModelFieldKind::Enum,
        "textarea" | "text" | "markdown" | "rich_text" => domain::ModelFieldKind::Text,
        "json" | "object" | "array" => domain::ModelFieldKind::Json,
        _ => domain::ModelFieldKind::String,
    }
}

pub fn collect_secret_strings(value: &Value) -> HashSet<String> {
    let mut secrets = HashSet::new();
    collect_secret_strings_into(value, &mut secrets);
    secrets
}

fn collect_secret_strings_into(value: &Value, secrets: &mut HashSet<String>) {
    match value {
        Value::String(raw) if !raw.is_empty() => {
            secrets.insert(raw.clone());
        }
        Value::Array(items) => {
            for item in items {
                collect_secret_strings_into(item, secrets);
            }
        }
        Value::Object(object) => {
            for child in object.values() {
                collect_secret_strings_into(child, secrets);
            }
        }
        _ => {}
    }
}

pub fn redact_value(value: &Value, secrets: &HashSet<String>) -> Value {
    match value {
        Value::String(raw) => Value::String(redact_string(raw, secrets)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| redact_value(item, secrets))
                .collect(),
        ),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, child)| (key.clone(), redact_value(child, secrets)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn redact_string(raw: &str, secrets: &HashSet<String>) -> String {
    if secrets.is_empty() {
        return raw.to_string();
    }

    let mut ordered_secrets = secrets.iter().collect::<Vec<_>>();
    ordered_secrets
        .sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));

    let mut redacted = raw.to_string();
    for secret in ordered_secrets {
        if !secret.is_empty() {
            redacted = redacted.replace(secret, "***");
        }
    }
    redacted
}

fn redact_preview_output(
    output: DataSourcePreviewReadOutput,
    secrets: &HashSet<String>,
) -> DataSourcePreviewReadOutput {
    DataSourcePreviewReadOutput {
        rows: output
            .rows
            .into_iter()
            .map(|row| redact_value(&row, secrets))
            .collect(),
        next_cursor: output
            .next_cursor
            .map(|cursor| redact_string(&cursor, secrets)),
    }
}

fn ensure_data_source_defaults_compatible(defaults: domain::DataSourceDefaults) -> Result<()> {
    let _ = defaults;
    Ok(())
}

fn build_preview_fingerprint(
    input: &DataSourcePreviewReadInput,
    secret_values: &HashSet<String>,
) -> Result<String> {
    let mut sanitized = input.clone();
    sanitized.connection.config_json =
        redact_value(&sanitized.connection.config_json, secret_values);
    sanitized.connection.secret_json =
        redact_value(&sanitized.connection.secret_json, secret_values);
    let bytes = serde_json::to_vec(&sanitized)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{}", to_hex(&digest)))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
