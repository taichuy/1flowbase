use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::*;
use crate::{
    provider_runtime::ApiProviderRuntime,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(super) enum ModelDefinitionsInput {
    List {
        query: ListModelsQuery,
        locale: ConsoleLocaleHints,
    },
    ListCompatibleTemplates(CompatibleTemplateCatalogQuery),
    ListAgentFlowOptions,
    Create(CreateModelDefinitionBody),
    AdvisorFindings {
        model_id: String,
    },
    ListScopeGrants {
        model_id: String,
    },
    Update {
        model_id: String,
        body: UpdateModelDefinitionBody,
    },
    Delete {
        model_id: String,
        confirmed: bool,
    },
    BatchDelete(BatchDeleteModelDefinitionsBody),
    CreateField {
        model_id: String,
        body: CreateModelFieldBody,
    },
    UpdateField {
        model_id: String,
        field_id: String,
        body: UpdateModelFieldBody,
    },
    DeleteField {
        model_id: String,
        field_id: String,
        confirmed: bool,
    },
    CreateScopeGrant {
        model_id: String,
        body: CreateScopeGrantBody,
    },
    UpdateScopeGrant {
        model_id: String,
        grant_id: String,
        body: UpdateScopeGrantBody,
    },
}

impl InterfaceContract for ModelDefinitionsInput {
    const CONTRACT_ID: &'static str = "console-model-definitions-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(super) enum ModelDefinitionsOutput {
    Models(Vec<ModelDefinitionResponse>),
    Templates(Vec<CompatibleTemplateCatalogEntryResponse>),
    AgentFlowOptions(Vec<AgentFlowDataModelOptionResponse>),
    Model(ModelDefinitionResponse),
    AdvisorFindings(Vec<DataModelAdvisorFindingResponse>),
    ScopeGrants(Vec<ScopeGrantResponse>),
    ScopeGrant(ScopeGrantResponse),
    Field(ModelFieldResponse),
    Deleted,
    BatchDeleted(BatchDeletedResponse),
}

impl InterfaceContract for ModelDefinitionsOutput {
    const CONTRACT_ID: &'static str = "console-model-definitions-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ModelDefinitionDependencies {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    runtime_registry_sync: ApiRuntimeRegistrySync,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
}

pub(crate) fn dependencies(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    provider_secret_master_key: String,
    api_node_id: String,
    provider_install_root: String,
) -> ModelDefinitionDependencies {
    let runtime_registry_sync =
        ApiRuntimeRegistrySync::new(store.clone(), runtime_engine.registry().clone());
    ModelDefinitionDependencies {
        store,
        bootstrap_workspace_id,
        runtime_engine,
        runtime_registry_sync,
        provider_runtime,
        provider_secret_master_key,
        api_node_id,
        provider_install_root,
    }
}

struct ModelDefinitionsAdapter {
    dependencies: ModelDefinitionDependencies,
}

impl ModelDefinitionsAdapter {
    fn settings_service(
        &self,
        operation_id: &'static str,
    ) -> crate::app_state::ApiModelDefinitionService {
        ModelDefinitionService::for_console_operation(
            self.dependencies.store.clone(),
            domain::ConsolePolicyGroup::settings_feature("system.data-models")
                .expect("compiled data-model settings group must be valid"),
            operation_id,
        )
    }

    fn mutation_service(
        &self,
        operation_id: &'static str,
    ) -> crate::app_state::ApiModelDefinitionMutationService {
        ModelDefinitionMutationService::for_console_operation(
            self.dependencies.store.clone(),
            self.dependencies.runtime_registry_sync.clone(),
            domain::ConsolePolicyGroup::settings_feature("system.data-models")
                .expect("compiled data-model settings group must be valid"),
            operation_id,
        )
    }

    fn data_source_service(
        &self,
        actor: &domain::ActorContext,
    ) -> crate::app_state::ApiDataSourceService {
        control_plane::data_source::DataSourceService::for_data_model_settings(
            self.dependencies.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.dependencies.provider_runtime.clone()),
            self.dependencies.provider_secret_master_key.clone(),
        )
        .with_node_artifact_context(
            self.dependencies.api_node_id.clone(),
            self.dependencies.provider_install_root.clone(),
        )
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ModelDefinitionsInput,
    ) -> Result<ModelDefinitionsOutput, ApiError> {
        let actor = principal.actor();
        let user_id = actor.user_id;
        match input {
            ModelDefinitionsInput::List { query, locale } => {
                let mut models = self
                    .settings_service(access_control::MODEL_DEFINITIONS_LIST_OPERATION_ID)
                    .list_models(user_id)
                    .await?;
                if let Some(data_source_id) = query.data_source_id.as_deref() {
                    if data_source_id == "main" {
                        models.retain(|model| {
                            model.source_kind == domain::DataModelSourceKind::MainSource
                                && model.data_source_instance_id.is_none()
                        });
                    } else {
                        let data_source_id = helpers::parse_uuid(data_source_id, "data_source_id")?;
                        models.retain(|model| {
                            model.source_kind == domain::DataModelSourceKind::ExternalSource
                                && model.data_source_instance_id == Some(data_source_id)
                        });
                    }
                }
                let filter = parse_resource_filter(query.filter.as_deref())?;
                models = STATE_MODEL_RESOURCE.filter_records(models, filter.as_ref())?;
                let preferred_locale = self
                    .dependencies
                    .store
                    .find_user_by_id(user_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
                    .preferred_locale;
                let locale = locale.resolve(preferred_locale);
                let resolver = CatalogResolver::new(
                    self.dependencies.store.clone(),
                    self.dependencies.bootstrap_workspace_id,
                );
                for model in &mut models {
                    project_system_metadata_titles(
                        &resolver,
                        self.dependencies.bootstrap_workspace_id,
                        &locale,
                        model,
                    )
                    .await?;
                    project_attachments_model_titles(
                        &resolver,
                        self.dependencies.bootstrap_workspace_id,
                        &locale,
                        model,
                    )
                    .await?;
                }
                Ok(ModelDefinitionsOutput::Models(
                    models
                        .into_iter()
                        .map(|model| {
                            to_model_definition_response(
                                model,
                                self.dependencies.runtime_engine.template_catalog(),
                            )
                        })
                        .collect(),
                ))
            }
            ModelDefinitionsInput::ListCompatibleTemplates(query) => {
                let templates = if query.data_source_id == "main" {
                    let source = plugin_framework::DataModelTemplateSource {
                        kind: plugin_framework::DataModelSourceKind::MainSource,
                        provider: None,
                    };
                    let capabilities =
                        runtime_core::general_data_model_template::source_capabilities(
                            &source, None,
                        );
                    self.dependencies
                        .runtime_engine
                        .template_catalog()
                        .compatible_templates(&source, capabilities.iter().map(String::as_str))
                        .into_iter()
                        .map(|template| compatible_template_response(template.descriptor()))
                        .collect()
                } else {
                    let instance_id = helpers::parse_uuid(&query.data_source_id, "data_source_id")?;
                    let resource_key = query.resource_key.as_deref().ok_or(
                        control_plane::errors::ControlPlaneError::InvalidInput("resource_key"),
                    )?;
                    self.data_source_service(actor)
                        .compatible_data_model_templates(
                            control_plane::data_source::ListCompatibleDataModelTemplatesCommand {
                                actor_user_id: user_id,
                                workspace_id: actor.current_workspace_id,
                                instance_id,
                                resource_key: resource_key.to_owned(),
                            },
                        )
                        .await?
                        .into_iter()
                        .map(|view| compatible_template_response(&view.descriptor))
                        .collect()
                };
                Ok(ModelDefinitionsOutput::Templates(templates))
            }
            ModelDefinitionsInput::ListAgentFlowOptions => {
                let models = ModelDefinitionService::for_console_operation(
                    self.dependencies.store.clone(),
                    domain::ConsolePolicyGroup::other("other.agent-flow")
                        .expect("compiled agent-flow policy group must be valid"),
                    "agent_flow.data_model_options.list",
                )
                .list_models(user_id)
                .await?;
                Ok(ModelDefinitionsOutput::AgentFlowOptions(
                    models
                        .into_iter()
                        .map(to_agent_flow_data_model_option_response)
                        .collect(),
                ))
            }
            ModelDefinitionsInput::Create(body) => {
                let model = self
                    .mutation_service(access_control::MODEL_DEFINITIONS_CREATE_OPERATION_ID)
                    .create_model(CreateModelDefinitionCommand {
                        actor_user_id: user_id,
                        scope_kind: parse_scope_kind(&body.scope_kind)?,
                        data_source_instance_id: None,
                        external_resource_key: None,
                        external_table_id: None,
                        external_capabilities: None,
                        template_provider: body.template_provider,
                        template_code: body.template_code,
                        template_version: body.template_version,
                        code: body.code,
                        title: body.title,
                        description: body.description,
                        status: body.status.as_deref().map(parse_model_status).transpose()?,
                    })
                    .await?;
                Ok(ModelDefinitionsOutput::Model(to_model_definition_response(
                    model,
                    self.dependencies.runtime_engine.template_catalog(),
                )))
            }
            ModelDefinitionsInput::AdvisorFindings { model_id } => {
                let findings = self
                    .settings_service(access_control::MODEL_DEFINITIONS_ADVISOR_VIEW_OPERATION_ID)
                    .advisor_findings(user_id, helpers::parse_uuid(&model_id, "model_id")?)
                    .await?;
                Ok(ModelDefinitionsOutput::AdvisorFindings(
                    findings
                        .into_iter()
                        .map(to_advisor_finding_response)
                        .collect(),
                ))
            }
            ModelDefinitionsInput::ListScopeGrants { model_id } => {
                let grants = self
                    .settings_service(access_control::MODEL_SCOPE_GRANTS_LIST_OPERATION_ID)
                    .list_scope_grants(user_id, helpers::parse_uuid(&model_id, "model_id")?)
                    .await?;
                Ok(ModelDefinitionsOutput::ScopeGrants(
                    grants.into_iter().map(to_scope_grant_response).collect(),
                ))
            }
            ModelDefinitionsInput::Update { model_id, body } => {
                let model_id = helpers::parse_uuid(&model_id, "model_id")?;
                let requested_status =
                    body.status.as_deref().map(parse_model_status).transpose()?;
                let mutation =
                    self.mutation_service(access_control::MODEL_DEFINITIONS_UPDATE_OPERATION_ID);
                let mut model = None;
                if body.title.is_some()
                    || body.description.is_some()
                    || body.external_table_id.is_some()
                {
                    let current = self
                        .settings_service(access_control::MODEL_DEFINITIONS_UPDATE_OPERATION_ID)
                        .get_model(user_id, model_id)
                        .await?;
                    model = Some(
                        mutation
                            .update_model(UpdateModelDefinitionCommand {
                                actor_user_id: user_id,
                                model_id,
                                external_table_id: body
                                    .external_table_id
                                    .or(current.external_table_id),
                                title: body.title.unwrap_or(current.title),
                                description: body.description.unwrap_or(current.description),
                            })
                            .await?,
                    );
                }
                if let Some(status) = requested_status {
                    model = Some(
                        mutation
                            .update_model_status(UpdateModelDefinitionStatusCommand {
                                actor_user_id: user_id,
                                model_id,
                                status,
                            })
                            .await?,
                    );
                }
                let model = model.ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                    "model_update",
                ))?;
                Ok(ModelDefinitionsOutput::Model(to_model_definition_response(
                    model,
                    self.dependencies.runtime_engine.template_catalog(),
                )))
            }
            ModelDefinitionsInput::Delete {
                model_id,
                confirmed,
            } => {
                self.mutation_service(access_control::MODEL_DEFINITIONS_DELETE_OPERATION_ID)
                    .delete_model(DeleteModelDefinitionCommand {
                        actor_user_id: user_id,
                        model_id: helpers::parse_uuid(&model_id, "model_id")?,
                        confirmed,
                    })
                    .await?;
                Ok(ModelDefinitionsOutput::Deleted)
            }
            ModelDefinitionsInput::BatchDelete(body) => {
                let models = self
                    .settings_service(access_control::MODEL_DEFINITIONS_DELETE_OPERATION_ID)
                    .list_models(user_id)
                    .await?;
                let model_ids = STATE_MODEL_RESOURCE.select_batch_ids(
                    models,
                    ResourceBatchSelection::new(body.filter_by_tk, body.filter),
                    |value| {
                        Uuid::parse_str(&value).map_err(|_| {
                            control_plane::errors::ControlPlaneError::InvalidInput("model_id")
                        })
                    },
                    |model| model.id,
                )?;
                let deleted_ids = self
                    .mutation_service(access_control::MODEL_DEFINITIONS_DELETE_OPERATION_ID)
                    .batch_delete_models(BatchDeleteModelDefinitionsCommand {
                        actor_user_id: user_id,
                        model_ids,
                        confirmed: body.confirmed,
                    })
                    .await?;
                Ok(ModelDefinitionsOutput::BatchDeleted(BatchDeletedResponse {
                    deleted: true,
                    deleted_count: deleted_ids.len(),
                    deleted_ids: deleted_ids.into_iter().map(|id| id.to_string()).collect(),
                }))
            }
            ModelDefinitionsInput::CreateField { model_id, body } => {
                let model_id = helpers::parse_uuid(&model_id, "model_id")?;
                let field = self
                    .mutation_service(access_control::MODEL_FIELDS_CREATE_OPERATION_ID)
                    .add_field(AddModelFieldCommand {
                        actor_user_id: user_id,
                        model_id,
                        code: body.code,
                        title: body.title,
                        description: body.description,
                        external_field_key: body.external_field_key,
                        field_kind: parse_field_kind(&body.field_kind)?,
                        is_required: body.is_required,
                        api_required: body.api_required,
                        is_unique: body.is_unique,
                        default_value: body.default_value,
                        display_interface: body.display_interface,
                        display_options: body.display_options,
                        relation_target_model_id: body
                            .relation_target_model_id
                            .as_deref()
                            .map(|value| helpers::parse_uuid(value, "relation_target_model_id"))
                            .transpose()?,
                        relation_options: body.relation_options,
                    })
                    .await?;
                let model = self
                    .settings_service(access_control::MODEL_FIELDS_CREATE_OPERATION_ID)
                    .get_model(user_id, model_id)
                    .await?;
                Ok(ModelDefinitionsOutput::Field(to_model_field_response(
                    &model, field,
                )))
            }
            ModelDefinitionsInput::UpdateField {
                model_id,
                field_id,
                body,
            } => {
                let model_id = helpers::parse_uuid(&model_id, "model_id")?;
                let field = self
                    .mutation_service(access_control::MODEL_FIELDS_UPDATE_OPERATION_ID)
                    .update_field(UpdateModelFieldCommand {
                        actor_user_id: user_id,
                        model_id,
                        field_id: helpers::parse_uuid(&field_id, "field_id")?,
                        title: body.title,
                        description: body.description,
                        is_required: body.is_required,
                        api_required: body.api_required,
                        is_unique: body.is_unique,
                        default_value: body.default_value,
                        display_interface: body.display_interface,
                        display_options: body.display_options,
                        relation_options: body.relation_options,
                    })
                    .await?;
                let model = self
                    .settings_service(access_control::MODEL_FIELDS_UPDATE_OPERATION_ID)
                    .get_model(user_id, model_id)
                    .await?;
                Ok(ModelDefinitionsOutput::Field(to_model_field_response(
                    &model, field,
                )))
            }
            ModelDefinitionsInput::DeleteField {
                model_id,
                field_id,
                confirmed,
            } => {
                self.mutation_service(access_control::MODEL_FIELDS_DELETE_OPERATION_ID)
                    .delete_field(DeleteModelFieldCommand {
                        actor_user_id: user_id,
                        model_id: helpers::parse_uuid(&model_id, "model_id")?,
                        field_id: helpers::parse_uuid(&field_id, "field_id")?,
                        confirmed,
                    })
                    .await?;
                Ok(ModelDefinitionsOutput::Deleted)
            }
            ModelDefinitionsInput::CreateScopeGrant { model_id, body } => {
                let grant = self
                    .settings_service(access_control::MODEL_SCOPE_GRANTS_CREATE_OPERATION_ID)
                    .create_scope_grant(CreateScopeDataModelGrantCommand {
                        actor_user_id: user_id,
                        scope_kind: parse_scope_kind(&body.scope_kind)?,
                        scope_id: body.scope_id,
                        data_model_id: helpers::parse_uuid(&model_id, "model_id")?,
                        enabled: body.enabled,
                        permission_profile: body.permission_profile,
                        confirm_unsafe_external_source_system_all: body
                            .confirm_unsafe_external_source_system_all,
                    })
                    .await?;
                Ok(ModelDefinitionsOutput::ScopeGrant(to_scope_grant_response(
                    grant,
                )))
            }
            ModelDefinitionsInput::UpdateScopeGrant {
                model_id,
                grant_id,
                body,
            } => {
                if body.enabled.is_none() && body.permission_profile.is_none() {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "scope_grant_update",
                    )
                    .into());
                }
                let grant = self
                    .settings_service(access_control::MODEL_SCOPE_GRANTS_UPDATE_OPERATION_ID)
                    .update_scope_grant(UpdateScopeDataModelGrantCommand {
                        actor_user_id: user_id,
                        data_model_id: helpers::parse_uuid(&model_id, "model_id")?,
                        grant_id: helpers::parse_uuid(&grant_id, "grant_id")?,
                        enabled: body.enabled,
                        permission_profile: body.permission_profile,
                        confirm_unsafe_external_source_system_all: body
                            .confirm_unsafe_external_source_system_all,
                    })
                    .await?;
                Ok(ModelDefinitionsOutput::ScopeGrant(to_scope_grant_response(
                    grant,
                )))
            }
        }
    }
}

impl ConsoleInterfacePort<ModelDefinitionsInput, ModelDefinitionsOutput>
    for ModelDefinitionsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ModelDefinitionsInput,
    ) -> ConsoleInterfaceFuture<'a, ModelDefinitionsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model_definitions.list",
        binding_id: "http.console.model-definitions.list.v1",
        method: "GET",
        path: "/api/console/settings/data-models/model-definitions",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_templates.list",
        binding_id: "http.console.model-definitions.templates.list.v1",
        method: "GET",
        path: "/api/console/settings/data-models/model-templates",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "agent_flow.data_model_options.list",
        binding_id: "http.console.model-definitions.agent-flow-options.list.v1",
        method: "GET",
        path: "/api/console/models/agent-flow-options",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_definitions.create",
        binding_id: "http.console.model-definitions.create.v1",
        method: "POST",
        path: "/api/console/settings/data-models/model-definitions",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_definitions.advisor.view",
        binding_id: "http.console.model-definitions.advisor-findings.list.v1",
        method: "GET",
        path: "/api/console/settings/data-models/model-definitions/:id/advisor-findings",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_scope_grants.list",
        binding_id: "http.console.model-definitions.scope-grants.list.v1",
        method: "GET",
        path: "/api/console/settings/data-models/model-definitions/:id/scope-grants",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_definitions.update",
        binding_id: "http.console.model-definitions.update.v1",
        method: "PATCH",
        path: "/api/console/settings/data-models/model-definitions/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_definitions.delete",
        binding_id: "http.console.model-definitions.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/data-models/model-definitions/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_definitions.delete",
        binding_id: "http.console.model-definitions.batch-delete.v1",
        method: "POST",
        path: "/api/console/settings/data-models/model-definitions:batchDelete",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_fields.create",
        binding_id: "http.console.model-definitions.fields.create.v1",
        method: "POST",
        path: "/api/console/settings/data-models/model-definitions/:id/fields",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_fields.update",
        binding_id: "http.console.model-definitions.fields.update.v1",
        method: "PATCH",
        path: "/api/console/settings/data-models/model-definitions/:id/fields/:field_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_fields.delete",
        binding_id: "http.console.model-definitions.fields.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/data-models/model-definitions/:id/fields/:field_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_scope_grants.create",
        binding_id: "http.console.model-definitions.scope-grants.create.v1",
        method: "POST",
        path: "/api/console/settings/data-models/model-definitions/:id/scope-grants",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_scope_grants.update",
        binding_id: "http.console.model-definitions.scope-grants.update.v1",
        method: "PATCH",
        path: "/api/console/settings/data-models/model-definitions/:id/scope-grants/:grant_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: ModelDefinitionDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-model-definitions",
        "graph:console-model-definitions-v1",
        DECLARATIONS,
        Arc::new(ModelDefinitionsAdapter { dependencies }),
    )
}
