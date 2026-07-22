use anyhow::Result;
#[cfg(not(test))]
use async_trait::async_trait;
use orchestration_runtime::{
    compiled_plan::{CompiledLlmRuntime, CompiledNode, CompiledPlan},
    compiler::FlowCompiler,
};
use plugin_framework::provider_contract::{
    PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY,
    PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY, PROVIDER_COUNT_TOKENS_CAPABILITY,
};
#[cfg(not(test))]
use plugin_framework::{
    provider_contract::CURRENT_PROVIDER_CONTRACT, provider_package::ProviderPackage,
};
use uuid::Uuid;

#[cfg(not(test))]
use crate::ports::{ModelProviderRepository, PluginRepository};
use crate::{
    application_public_api::{
        application_is_editable, ensure_application_view_permission,
        mapping::{
            ApplicationApiMappingDraft, ApplicationOperationBindings,
            ApplicationOperationTargetBinding,
        },
        publications::ApplicationPublicationVersionRecord,
    },
    errors::ControlPlaneError,
    ports::{
        ApplicationApiMappingRepository, ApplicationCompileContextRepository,
        ApplicationCompiledPlanRepository, ApplicationOperationBindingCapabilityRepository,
        ApplicationPublicationRepository, ApplicationRepository, FlowRepository,
    },
};

/// One operation is one persisted binding slot. `local_summary` intentionally
/// remains on Generate, so it does not create another selectable binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApplicationOperationBindingOperation {
    Generate,
    CountTokens,
    CompactResponsesCompact,
    CompactResponsesCompactionV2,
}

impl ApplicationOperationBindingOperation {
    pub const ALL: [Self; 4] = [
        Self::Generate,
        Self::CountTokens,
        Self::CompactResponsesCompact,
        Self::CompactResponsesCompactionV2,
    ];

    pub fn required_manifest_capability(self) -> Option<&'static str> {
        match self {
            Self::Generate => None,
            Self::CountTokens => Some(PROVIDER_COUNT_TOKENS_CAPABILITY),
            Self::CompactResponsesCompact => Some(PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY),
            Self::CompactResponsesCompactionV2 => {
                Some(PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY)
            }
        }
    }

    fn binding(
        self,
        bindings: &ApplicationOperationBindings,
    ) -> Option<&ApplicationOperationTargetBinding> {
        match self {
            Self::Generate => bindings.generate.as_ref(),
            Self::CountTokens => bindings.count_tokens.as_ref(),
            Self::CompactResponsesCompact => bindings.compact.responses_compact.as_ref(),
            Self::CompactResponsesCompactionV2 => bindings.compact.responses_compaction_v2.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationOperationBindingCapabilitySupport {
    Supported,
    ProviderTargetUnavailable,
    ProviderContractUnsupported,
    ProviderManifestUnavailable,
    ProviderCapabilityUnsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationOperationBindingUnsupportedReason {
    CompiledPlanMissing,
    CompiledPlanMismatch,
    CompiledPlanInvalid,
    TargetMissing,
    TargetNotLlm,
    TargetRuntimeIncomplete,
    ProviderTargetUnavailable,
    ProviderContractUnsupported,
    ProviderManifestUnavailable,
    ProviderCapabilityUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationOperationBindingTargetOption {
    pub target_node_id: String,
    pub node_alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationOperationBindingOptions {
    pub operation: ApplicationOperationBindingOperation,
    pub targets: Vec<ApplicationOperationBindingTargetOption>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDraftOperationBindingProjection {
    pub operation_bindings: ApplicationOperationBindings,
    pub options: Vec<ApplicationOperationBindingOptions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationPublishedOperationBindingSupport {
    Supported {
        target: ApplicationOperationBindingTargetOption,
    },
    Unbound,
    Unsupported {
        target: Option<ApplicationOperationBindingTargetOption>,
        reason: ApplicationOperationBindingUnsupportedReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationPublishedOperationBindingProjection {
    pub operation: ApplicationOperationBindingOperation,
    pub target_node_id: Option<String>,
    pub support: ApplicationPublishedOperationBindingSupport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationPublishedOperationBindingsProjection {
    pub publication_id: Uuid,
    pub compiled_plan_id: Uuid,
    pub bindings: Vec<ApplicationPublishedOperationBindingProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationOperationBindingProjection {
    pub editable: bool,
    pub draft: ApplicationDraftOperationBindingProjection,
    pub published: Option<ApplicationPublishedOperationBindingsProjection>,
}

#[derive(Debug, Clone)]
pub struct GetApplicationOperationBindingProjectionCommand {
    pub actor_user_id: Uuid,
    pub application_id: Uuid,
}

pub struct ApplicationOperationBindingProjectionService<R> {
    repository: R,
}

impl<R> ApplicationOperationBindingProjectionService<R>
where
    R: ApplicationRepository
        + ApplicationApiMappingRepository
        + ApplicationPublicationRepository
        + ApplicationCompiledPlanRepository
        + ApplicationCompileContextRepository
        + ApplicationOperationBindingCapabilityRepository
        + FlowRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn get_projection(
        &self,
        command: GetApplicationOperationBindingProjectionCommand,
    ) -> Result<ApplicationOperationBindingProjection> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        let application = self
            .repository
            .get_application(actor.current_workspace_id, command.application_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("application"))?;
        ensure_application_view_permission(&self.repository, &actor, &application).await?;

        let draft = self
            .repository
            .get_application_api_mapping(application.id)
            .await?
            .unwrap_or_else(ApplicationApiMappingDraft::default_native);
        let draft_plan = self.compile_draft_plan(&application, &actor).await?;
        let options =
            draft_binding_options(&self.repository, application.workspace_id, &draft_plan).await?;
        let published = self
            .published_projection(application.workspace_id, application.id)
            .await?;

        Ok(ApplicationOperationBindingProjection {
            editable: application_is_editable(&self.repository, &actor, &application).await?,
            draft: ApplicationDraftOperationBindingProjection {
                operation_bindings: draft.operation_bindings,
                options,
            },
            published,
        })
    }

    async fn compile_draft_plan(
        &self,
        application: &domain::ApplicationRecord,
        actor: &domain::ActorContext,
    ) -> Result<CompiledPlan> {
        let editor_state = self
            .repository
            .get_or_create_editor_state(application.workspace_id, application.id, actor.user_id)
            .await?;
        let compile_context = self
            .repository
            .build_application_compile_context(application.workspace_id, application.id)
            .await?;

        match application.application_type {
            domain::ApplicationType::AgentFlow => FlowCompiler::compile(
                editor_state.flow.id,
                &editor_state.draft.id.to_string(),
                &editor_state.draft.document,
                &compile_context,
            ),
            domain::ApplicationType::Workflow => FlowCompiler::compile_workflow(
                editor_state.flow.id,
                &editor_state.draft.id.to_string(),
                &editor_state.draft.document,
                &compile_context,
            ),
        }
    }

    async fn published_projection(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> Result<Option<ApplicationPublishedOperationBindingsProjection>> {
        let Some(publication) = self
            .repository
            .load_active_application_publication(application_id)
            .await?
        else {
            return Ok(None);
        };

        let compiled_plan = self
            .repository
            .get_application_compiled_plan(publication.compiled_plan_id)
            .await?;
        let bindings = match compiled_plan {
            None => unsupported_published_bindings(
                &publication,
                ApplicationOperationBindingUnsupportedReason::CompiledPlanMissing,
            ),
            Some(record) if record.id != publication.compiled_plan_id => {
                unsupported_published_bindings(
                    &publication,
                    ApplicationOperationBindingUnsupportedReason::CompiledPlanMismatch,
                )
            }
            Some(record) => match serde_json::from_value::<CompiledPlan>(record.plan) {
                Ok(plan) => {
                    self.published_bindings_for_plan(workspace_id, &publication, &plan)
                        .await?
                }
                Err(_) => unsupported_published_bindings(
                    &publication,
                    ApplicationOperationBindingUnsupportedReason::CompiledPlanInvalid,
                ),
            },
        };

        Ok(Some(ApplicationPublishedOperationBindingsProjection {
            publication_id: publication.id,
            compiled_plan_id: publication.compiled_plan_id,
            bindings,
        }))
    }

    async fn published_bindings_for_plan(
        &self,
        workspace_id: Uuid,
        publication: &ApplicationPublicationVersionRecord,
        compiled_plan: &CompiledPlan,
    ) -> Result<Vec<ApplicationPublishedOperationBindingProjection>> {
        let mut bindings = Vec::with_capacity(ApplicationOperationBindingOperation::ALL.len());
        for operation in ApplicationOperationBindingOperation::ALL {
            bindings.push(
                published_binding_projection(
                    &self.repository,
                    workspace_id,
                    operation,
                    &publication.operation_bindings,
                    compiled_plan,
                )
                .await?,
            );
        }
        Ok(bindings)
    }
}

pub(crate) async fn draft_binding_options<R>(
    repository: &R,
    workspace_id: Uuid,
    compiled_plan: &CompiledPlan,
) -> Result<Vec<ApplicationOperationBindingOptions>>
where
    R: ApplicationOperationBindingCapabilityRepository,
{
    let candidates = valid_operation_binding_targets(compiled_plan);
    let mut options = Vec::with_capacity(ApplicationOperationBindingOperation::ALL.len());
    for operation in ApplicationOperationBindingOperation::ALL {
        let mut targets = Vec::new();
        for candidate in &candidates {
            if repository
                .application_operation_binding_capability(
                    workspace_id,
                    &candidate.runtime,
                    operation,
                )
                .await?
                == ApplicationOperationBindingCapabilitySupport::Supported
            {
                targets.push(candidate.target.clone());
            }
        }
        options.push(ApplicationOperationBindingOptions { operation, targets });
    }
    Ok(options)
}

async fn published_binding_projection<R>(
    repository: &R,
    workspace_id: Uuid,
    operation: ApplicationOperationBindingOperation,
    bindings: &ApplicationOperationBindings,
    compiled_plan: &CompiledPlan,
) -> Result<ApplicationPublishedOperationBindingProjection>
where
    R: ApplicationOperationBindingCapabilityRepository,
{
    let Some(binding) = operation.binding(bindings) else {
        return Ok(ApplicationPublishedOperationBindingProjection {
            operation,
            target_node_id: None,
            support: ApplicationPublishedOperationBindingSupport::Unbound,
        });
    };
    let target_node_id = binding.target_node_id.clone();
    let Some(node) = compiled_plan
        .nodes
        .get(&target_node_id)
        .filter(|node| node.node_id == target_node_id)
    else {
        return Ok(unsupported_published_binding(
            operation,
            target_node_id,
            None,
            ApplicationOperationBindingUnsupportedReason::TargetMissing,
        ));
    };
    let target = operation_binding_target_option(node);
    if node.node_type != "llm" {
        return Ok(unsupported_published_binding(
            operation,
            target_node_id,
            Some(target),
            ApplicationOperationBindingUnsupportedReason::TargetNotLlm,
        ));
    }
    let Some(runtime) = node
        .llm_runtime
        .as_ref()
        .filter(|runtime| compiled_llm_runtime_is_complete(runtime))
    else {
        return Ok(unsupported_published_binding(
            operation,
            target_node_id,
            Some(target),
            ApplicationOperationBindingUnsupportedReason::TargetRuntimeIncomplete,
        ));
    };

    match repository
        .application_operation_binding_capability(workspace_id, runtime, operation)
        .await?
    {
        ApplicationOperationBindingCapabilitySupport::Supported => {
            Ok(ApplicationPublishedOperationBindingProjection {
                operation,
                target_node_id: Some(target_node_id),
                support: ApplicationPublishedOperationBindingSupport::Supported { target },
            })
        }
        ApplicationOperationBindingCapabilitySupport::ProviderTargetUnavailable => {
            Ok(unsupported_published_binding(
                operation,
                target_node_id,
                Some(target),
                ApplicationOperationBindingUnsupportedReason::ProviderTargetUnavailable,
            ))
        }
        ApplicationOperationBindingCapabilitySupport::ProviderContractUnsupported => {
            Ok(unsupported_published_binding(
                operation,
                target_node_id,
                Some(target),
                ApplicationOperationBindingUnsupportedReason::ProviderContractUnsupported,
            ))
        }
        ApplicationOperationBindingCapabilitySupport::ProviderManifestUnavailable => {
            Ok(unsupported_published_binding(
                operation,
                target_node_id,
                Some(target),
                ApplicationOperationBindingUnsupportedReason::ProviderManifestUnavailable,
            ))
        }
        ApplicationOperationBindingCapabilitySupport::ProviderCapabilityUnsupported => {
            Ok(unsupported_published_binding(
                operation,
                target_node_id,
                Some(target),
                ApplicationOperationBindingUnsupportedReason::ProviderCapabilityUnsupported,
            ))
        }
    }
}

fn unsupported_published_bindings(
    publication: &ApplicationPublicationVersionRecord,
    reason: ApplicationOperationBindingUnsupportedReason,
) -> Vec<ApplicationPublishedOperationBindingProjection> {
    ApplicationOperationBindingOperation::ALL
        .into_iter()
        .map(
            |operation| match operation.binding(&publication.operation_bindings) {
                Some(binding) => unsupported_published_binding(
                    operation,
                    binding.target_node_id.clone(),
                    None,
                    reason,
                ),
                None => ApplicationPublishedOperationBindingProjection {
                    operation,
                    target_node_id: None,
                    support: ApplicationPublishedOperationBindingSupport::Unbound,
                },
            },
        )
        .collect()
}

fn unsupported_published_binding(
    operation: ApplicationOperationBindingOperation,
    target_node_id: String,
    target: Option<ApplicationOperationBindingTargetOption>,
    reason: ApplicationOperationBindingUnsupportedReason,
) -> ApplicationPublishedOperationBindingProjection {
    ApplicationPublishedOperationBindingProjection {
        operation,
        target_node_id: Some(target_node_id),
        support: ApplicationPublishedOperationBindingSupport::Unsupported { target, reason },
    }
}

#[derive(Debug, Clone)]
struct ValidOperationBindingTarget {
    target: ApplicationOperationBindingTargetOption,
    runtime: CompiledLlmRuntime,
}

fn valid_operation_binding_targets(
    compiled_plan: &CompiledPlan,
) -> Vec<ValidOperationBindingTarget> {
    compiled_plan
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            if node.node_id != node_id.as_str() || node.node_type != "llm" {
                return None;
            }
            let runtime = node
                .llm_runtime
                .as_ref()
                .filter(|runtime| compiled_llm_runtime_is_complete(runtime))?
                .clone();
            Some(ValidOperationBindingTarget {
                target: operation_binding_target_option(node),
                runtime,
            })
        })
        .collect()
}

fn operation_binding_target_option(node: &CompiledNode) -> ApplicationOperationBindingTargetOption {
    ApplicationOperationBindingTargetOption {
        target_node_id: node.node_id.clone(),
        node_alias: node.alias.clone(),
    }
}

fn compiled_llm_runtime_is_complete(runtime: &CompiledLlmRuntime) -> bool {
    let root_is_complete = [
        runtime.provider_instance_id.as_str(),
        runtime.provider_code.as_str(),
        runtime.protocol.as_str(),
        runtime.model.as_str(),
    ]
    .into_iter()
    .all(non_empty_trimmed);
    let routes_are_complete = runtime.routing.as_ref().is_none_or(|routing| {
        routing.queue_targets.iter().all(|target| {
            [
                target.provider_instance_id.as_str(),
                target.provider_code.as_str(),
                target.protocol.as_str(),
                target.upstream_model_id.as_str(),
            ]
            .into_iter()
            .all(non_empty_trimmed)
        })
    });
    root_is_complete && routes_are_complete
}

fn non_empty_trimmed(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

#[cfg(not(test))]
#[async_trait]
impl<T> ApplicationOperationBindingCapabilityRepository for T
where
    T: ModelProviderRepository + PluginRepository + Send + Sync,
{
    async fn application_operation_binding_capability(
        &self,
        workspace_id: Uuid,
        runtime: &CompiledLlmRuntime,
        operation: ApplicationOperationBindingOperation,
    ) -> Result<ApplicationOperationBindingCapabilitySupport> {
        let mut targets = vec![(
            runtime.provider_instance_id.as_str(),
            runtime.provider_code.as_str(),
            runtime.protocol.as_str(),
        )];
        if let Some(routing) = runtime.routing.as_ref() {
            targets.extend(routing.queue_targets.iter().map(|target| {
                (
                    target.provider_instance_id.as_str(),
                    target.provider_code.as_str(),
                    target.protocol.as_str(),
                )
            }));
        }
        targets.sort_unstable();
        targets.dedup();

        for (provider_instance_id, provider_code, protocol) in targets {
            let Ok(provider_instance_id) = Uuid::parse_str(provider_instance_id) else {
                return Ok(ApplicationOperationBindingCapabilitySupport::ProviderTargetUnavailable);
            };
            let Some(instance) = self
                .get_instance(workspace_id, provider_instance_id)
                .await?
            else {
                return Ok(ApplicationOperationBindingCapabilitySupport::ProviderTargetUnavailable);
            };
            if instance.provider_code != provider_code || instance.protocol != protocol {
                return Ok(ApplicationOperationBindingCapabilitySupport::ProviderTargetUnavailable);
            }
            let Some(installation) = self.get_installation(instance.installation_id).await? else {
                return Ok(ApplicationOperationBindingCapabilitySupport::ProviderTargetUnavailable);
            };
            if installation.contract_version != CURRENT_PROVIDER_CONTRACT
                || installation.provider_code != provider_code
                || installation.protocol != protocol
            {
                return Ok(
                    ApplicationOperationBindingCapabilitySupport::ProviderContractUnsupported,
                );
            }
            let Some(required_capability) = operation.required_manifest_capability() else {
                continue;
            };
            let Ok(package) = ProviderPackage::load_from_dir(&installation.installed_path) else {
                return Ok(
                    ApplicationOperationBindingCapabilitySupport::ProviderManifestUnavailable,
                );
            };
            if !package
                .manifest
                .runtime
                .capabilities
                .iter()
                .any(|capability| capability == required_capability)
            {
                return Ok(
                    ApplicationOperationBindingCapabilitySupport::ProviderCapabilityUnsupported,
                );
            }
        }

        Ok(ApplicationOperationBindingCapabilitySupport::Supported)
    }
}
