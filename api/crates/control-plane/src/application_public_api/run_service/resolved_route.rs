use std::collections::BTreeSet;

use anyhow::Result;
use async_trait::async_trait;
use orchestration_runtime::compiled_plan::{CompiledLlmRuntime, CompiledPlan};
use plugin_framework::provider_contract::{
    ProviderCompactProfile, ProviderInvocationCapability, ProviderWireOperation,
};
use uuid::Uuid;

#[cfg(not(test))]
use crate::ports::{ModelProviderRepository, PluginRepository};
#[cfg(not(test))]
use plugin_framework::{
    provider_contract::CURRENT_PROVIDER_CONTRACT, provider_package::ProviderPackage,
};

use super::super::publications::ApplicationPublicationVersionRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerateExecutionProfile {
    Standard,
    LocalSummary,
}

impl GenerateExecutionProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "generate",
            Self::LocalSummary => "local_summary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishedRouteDispatch {
    OperationBinding,
    ApplicationFlow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProviderRoute {
    pub operation: ProviderWireOperation,
    pub profile: GenerateExecutionProfile,
    pub target_node_id: String,
    pub llm_runtime: CompiledLlmRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCountTokensProviderRoute {
    pub operation: ProviderWireOperation,
    pub target_node_id: String,
    pub llm_runtime: CompiledLlmRuntime,
}

/// A remote Compact target resolved from the immutable publication snapshot.
/// Local summary remains a Generate execution and is deliberately not routable
/// through this unary provider path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCompactProviderRoute {
    pub operation: ProviderWireOperation,
    pub profile: ProviderCompactProfile,
    pub target_node_id: String,
    pub llm_runtime: CompiledLlmRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedPublishedRoute {
    ApplicationFlow { compiled_plan_id: Uuid },
    Provider(ResolvedProviderRoute),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishedRouteResolutionError {
    CompiledPlanMismatch,
    CompiledPlanInvalid,
    OperationUnbound,
    TargetMissing,
    TargetNotLlm,
    IncompleteLlmRuntime,
    ProviderCapabilityMismatch,
}

impl PublishedRouteResolutionError {
    pub fn code(self) -> &'static str {
        match self {
            Self::CompiledPlanMismatch => "compiled_plan_mismatch",
            Self::CompiledPlanInvalid => "compiled_plan_invalid",
            Self::OperationUnbound => "operation_unbound",
            Self::TargetMissing => "operation_target_missing",
            Self::TargetNotLlm => "operation_target_not_llm",
            Self::IncompleteLlmRuntime => "operation_target_runtime_incomplete",
            Self::ProviderCapabilityMismatch => "provider_capability_mismatch",
        }
    }
}

#[async_trait]
pub trait PublishedProviderManifestCapabilityRepository: Send + Sync {
    async fn supports_published_generate(
        &self,
        workspace_id: Uuid,
        runtime: &CompiledLlmRuntime,
        profile: GenerateExecutionProfile,
        required_semantic_capabilities: &BTreeSet<ProviderInvocationCapability>,
    ) -> Result<bool>;
    async fn supports_published_count_tokens(
        &self,
        workspace_id: Uuid,
        runtime: &CompiledLlmRuntime,
    ) -> Result<bool>;
    async fn supports_published_compact(
        &self,
        workspace_id: Uuid,
        runtime: &CompiledLlmRuntime,
        profile: ProviderCompactProfile,
    ) -> Result<bool> {
        let _ = (workspace_id, runtime, profile);
        Ok(false)
    }
}

// Unit tests inject an explicit in-memory implementation from test_support. Excluding the
// production blanket from that build prevents overlapping implementations.
#[cfg(not(test))]
#[async_trait]
impl<T> PublishedProviderManifestCapabilityRepository for T
where
    T: ModelProviderRepository + PluginRepository + Send + Sync,
{
    async fn supports_published_generate(
        &self,
        workspace_id: Uuid,
        runtime: &CompiledLlmRuntime,
        _profile: GenerateExecutionProfile,
        required_semantic_capabilities: &BTreeSet<ProviderInvocationCapability>,
    ) -> Result<bool> {
        supports_published_manifest_capabilities(
            self,
            workspace_id,
            runtime,
            required_semantic_capabilities,
        )
        .await
    }

    async fn supports_published_count_tokens(
        &self,
        workspace_id: Uuid,
        runtime: &CompiledLlmRuntime,
    ) -> Result<bool> {
        let required_capabilities = BTreeSet::from([ProviderInvocationCapability::CountTokens]);
        supports_published_manifest_capabilities(
            self,
            workspace_id,
            runtime,
            &required_capabilities,
        )
        .await
    }

    async fn supports_published_compact(
        &self,
        workspace_id: Uuid,
        runtime: &CompiledLlmRuntime,
        profile: ProviderCompactProfile,
    ) -> Result<bool> {
        let required_capabilities = BTreeSet::from([profile.required_capability()]);
        supports_published_manifest_capabilities(
            self,
            workspace_id,
            runtime,
            &required_capabilities,
        )
        .await
    }
}

#[cfg(not(test))]
async fn supports_published_manifest_capabilities<T>(
    repository: &T,
    workspace_id: Uuid,
    runtime: &CompiledLlmRuntime,
    required_capabilities: &BTreeSet<ProviderInvocationCapability>,
) -> Result<bool>
where
    T: ModelProviderRepository + PluginRepository + Send + Sync,
{
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
            return Ok(false);
        };
        let Some(instance) = repository
            .get_instance(workspace_id, provider_instance_id)
            .await?
        else {
            return Ok(false);
        };
        if instance.provider_code != provider_code || instance.protocol != protocol {
            return Ok(false);
        }
        let Some(installation) = repository
            .get_installation(instance.installation_id)
            .await?
        else {
            return Ok(false);
        };
        if installation.contract_version != CURRENT_PROVIDER_CONTRACT
            || installation.provider_code != provider_code
            || installation.protocol != protocol
        {
            return Ok(false);
        }
        if !required_capabilities.is_empty() {
            let Ok(package) = ProviderPackage::load_from_dir(&installation.installed_path) else {
                return Ok(false);
            };
            if required_capabilities.iter().any(|capability| {
                !package
                    .manifest
                    .runtime
                    .capabilities
                    .iter()
                    .any(|declared| declared == capability.manifest_capability_name())
            }) {
                return Ok(false);
            }
        }
    }

    Ok(true)
}

pub struct PublishedRouteResolver<'a, R> {
    repository: &'a R,
}

impl<'a, R> PublishedRouteResolver<'a, R>
where
    R: PublishedProviderManifestCapabilityRepository,
{
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub async fn resolve_generate(
        &self,
        workspace_id: Uuid,
        publication: &ApplicationPublicationVersionRecord,
        compiled_plan_record: &domain::CompiledPlanRecord,
        dispatch: PublishedRouteDispatch,
        profile: GenerateExecutionProfile,
        required_semantic_capabilities: &BTreeSet<ProviderInvocationCapability>,
    ) -> std::result::Result<ResolvedPublishedRoute, PublishedRouteResolutionError> {
        if compiled_plan_record.id != publication.compiled_plan_id {
            return Err(PublishedRouteResolutionError::CompiledPlanMismatch);
        }
        if dispatch == PublishedRouteDispatch::ApplicationFlow {
            return Ok(ResolvedPublishedRoute::ApplicationFlow {
                compiled_plan_id: compiled_plan_record.id,
            });
        }

        let binding = publication
            .operation_bindings
            .generate
            .as_ref()
            .ok_or(PublishedRouteResolutionError::OperationUnbound)?;
        let compiled_plan: CompiledPlan = serde_json::from_value(compiled_plan_record.plan.clone())
            .map_err(|_| PublishedRouteResolutionError::CompiledPlanInvalid)?;
        let node = compiled_plan
            .nodes
            .get(&binding.target_node_id)
            .filter(|node| node.node_id == binding.target_node_id)
            .ok_or(PublishedRouteResolutionError::TargetMissing)?;
        if node.node_type != "llm" {
            return Err(PublishedRouteResolutionError::TargetNotLlm);
        }
        let runtime = node
            .llm_runtime
            .as_ref()
            .filter(|runtime| llm_runtime_is_complete(runtime))
            .ok_or(PublishedRouteResolutionError::IncompleteLlmRuntime)?;
        let supported = self
            .repository
            .supports_published_generate(
                workspace_id,
                runtime,
                profile,
                required_semantic_capabilities,
            )
            .await
            .unwrap_or(false);
        if !supported {
            return Err(PublishedRouteResolutionError::ProviderCapabilityMismatch);
        }

        Ok(ResolvedPublishedRoute::Provider(ResolvedProviderRoute {
            operation: ProviderWireOperation::Generate,
            profile,
            target_node_id: binding.target_node_id.clone(),
            llm_runtime: runtime.clone(),
        }))
    }

    pub async fn resolve_count_tokens(
        &self,
        workspace_id: Uuid,
        publication: &ApplicationPublicationVersionRecord,
        compiled_plan_record: &domain::CompiledPlanRecord,
    ) -> std::result::Result<ResolvedCountTokensProviderRoute, PublishedRouteResolutionError> {
        if compiled_plan_record.id != publication.compiled_plan_id {
            return Err(PublishedRouteResolutionError::CompiledPlanMismatch);
        }

        let binding = publication
            .operation_bindings
            .count_tokens
            .as_ref()
            .ok_or(PublishedRouteResolutionError::OperationUnbound)?;
        let compiled_plan: CompiledPlan = serde_json::from_value(compiled_plan_record.plan.clone())
            .map_err(|_| PublishedRouteResolutionError::CompiledPlanInvalid)?;
        let node = compiled_plan
            .nodes
            .get(&binding.target_node_id)
            .filter(|node| node.node_id == binding.target_node_id)
            .ok_or(PublishedRouteResolutionError::TargetMissing)?;
        if node.node_type != "llm" {
            return Err(PublishedRouteResolutionError::TargetNotLlm);
        }
        let runtime = node
            .llm_runtime
            .as_ref()
            .filter(|runtime| llm_runtime_is_complete(runtime))
            .ok_or(PublishedRouteResolutionError::IncompleteLlmRuntime)?;
        let supported = self
            .repository
            .supports_published_count_tokens(workspace_id, runtime)
            .await
            .unwrap_or(false);
        if !supported {
            return Err(PublishedRouteResolutionError::ProviderCapabilityMismatch);
        }

        Ok(ResolvedCountTokensProviderRoute {
            operation: ProviderWireOperation::CountTokens,
            target_node_id: binding.target_node_id.clone(),
            llm_runtime: runtime.clone(),
        })
    }

    pub async fn resolve_compact(
        &self,
        workspace_id: Uuid,
        publication: &ApplicationPublicationVersionRecord,
        compiled_plan_record: &domain::CompiledPlanRecord,
        profile: ProviderCompactProfile,
    ) -> std::result::Result<ResolvedCompactProviderRoute, PublishedRouteResolutionError> {
        if compiled_plan_record.id != publication.compiled_plan_id {
            return Err(PublishedRouteResolutionError::CompiledPlanMismatch);
        }

        let binding = match profile {
            ProviderCompactProfile::ResponsesCompact => publication
                .operation_bindings
                .compact
                .responses_compact
                .as_ref(),
            ProviderCompactProfile::ResponsesCompactionV2 => publication
                .operation_bindings
                .compact
                .responses_compaction_v2
                .as_ref(),
        }
        .ok_or(PublishedRouteResolutionError::OperationUnbound)?;
        let compiled_plan: CompiledPlan = serde_json::from_value(compiled_plan_record.plan.clone())
            .map_err(|_| PublishedRouteResolutionError::CompiledPlanInvalid)?;
        let node = compiled_plan
            .nodes
            .get(&binding.target_node_id)
            .filter(|node| node.node_id == binding.target_node_id)
            .ok_or(PublishedRouteResolutionError::TargetMissing)?;
        if node.node_type != "llm" {
            return Err(PublishedRouteResolutionError::TargetNotLlm);
        }
        let runtime = node
            .llm_runtime
            .as_ref()
            .filter(|runtime| llm_runtime_is_complete(runtime))
            .ok_or(PublishedRouteResolutionError::IncompleteLlmRuntime)?;
        let supported = self
            .repository
            .supports_published_compact(workspace_id, runtime, profile)
            .await
            .unwrap_or(false);
        if !supported {
            return Err(PublishedRouteResolutionError::ProviderCapabilityMismatch);
        }

        Ok(ResolvedCompactProviderRoute {
            operation: ProviderWireOperation::Compact,
            profile,
            target_node_id: binding.target_node_id.clone(),
            llm_runtime: runtime.clone(),
        })
    }
}

fn llm_runtime_is_complete(runtime: &CompiledLlmRuntime) -> bool {
    let root_is_complete = [
        runtime.provider_instance_id.as_str(),
        runtime.provider_code.as_str(),
        runtime.protocol.as_str(),
        runtime.model.as_str(),
    ]
    .into_iter()
    .all(non_empty_trimmed);
    let routes_are_complete = runtime.routing.as_ref().map_or(true, |routing| {
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
