use std::{
    collections::{BTreeMap, BTreeSet},
    panic::AssertUnwindSafe,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use async_trait::async_trait;
use futures_util::FutureExt;
use plugin_framework::{
    extension_bus::{
        Cardinality, DeliverySemantics, EffectiveExtensionGraph, ExtensionPointKind,
        FailureSemantics, LifecycleSemantics, OrderingSemantics, OverridePolicy, ScopeSemantics,
    },
    provider_contract::ProviderInvocationInput,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const PROVIDER_INPUT_PIPELINE_POINT_ID: &str = "1flowbase.application.provider-input";
pub const PROVIDER_INPUT_PIPELINE_OWNER_MODULE_ID: &str = "1flowbase.boot-core";
pub const PROVIDER_INPUT_PIPELINE_CONTRACT_ID: &str = "application-provider-input-pipeline";
pub const PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION: &str = "1";
pub const REWRITE_MESSAGES_PERMISSION: &str = "provider-input.messages.write";
pub const REWRITE_SYSTEM_PERMISSION: &str = "provider-input.system.write";
pub const REWRITE_TOOLS_PERMISSION: &str = "provider-input.tools.write";
pub const REWRITE_RESPONSE_FORMAT_PERMISSION: &str = "provider-input.response-format.write";
pub const REWRITE_MODEL_PARAMETERS_PERMISSION: &str = "provider-input.model-parameters.write";

#[async_trait]
pub trait ProviderInputPipelineContribution: Send + Sync {
    async fn rewrite(&self, input: ProviderInvocationInput) -> Result<ProviderInvocationInput>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderInputContributionFailurePolicy {
    FailClosed,
}

#[derive(Clone)]
pub struct TrustedProviderInputContributionRegistration {
    pub contribution_id: String,
    pub timeout: Duration,
    pub failure_policy: ProviderInputContributionFailurePolicy,
    pub executor: Arc<dyn ProviderInputPipelineContribution>,
}

#[derive(Clone)]
struct OrderedContribution {
    contribution_id: String,
    permissions: BTreeSet<String>,
    registration: TrustedProviderInputContributionRegistration,
}

#[derive(Clone)]
pub struct ProviderInputPipeline {
    graph_fingerprint: String,
    contributions: Vec<OrderedContribution>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderInputPipelineOutput {
    pub input: ProviderInvocationInput,
    pub receipt: Option<PipelineExecutionReceipt>,
}

impl ProviderInputPipelineOutput {
    pub fn unchanged(input: ProviderInvocationInput) -> Self {
        Self {
            input,
            receipt: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PipelineExecutionReceipt {
    pub graph_fingerprint: String,
    pub point_id: String,
    pub before_digest: String,
    pub after_digest: String,
    pub receipt_digest: String,
    pub contributions: Vec<PipelineContributionReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PipelineContributionReceipt {
    pub contribution_id: String,
    pub order: usize,
    pub before_digest: String,
    pub after_digest: String,
    pub changed_fields: Vec<String>,
    pub duration_micros: u64,
    pub status: PipelineContributionStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineContributionStatus {
    Succeeded,
    Failed,
}

#[derive(Debug)]
pub struct ProviderInputPipelineError {
    pub code: &'static str,
    pub receipt: PipelineExecutionReceipt,
}

impl std::fmt::Display for ProviderInputPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "provider input pipeline failed: {}", self.code)
    }
}

impl std::error::Error for ProviderInputPipelineError {}

impl ProviderInputPipeline {
    pub fn from_graph(
        graph: Arc<EffectiveExtensionGraph>,
        registrations: Vec<TrustedProviderInputContributionRegistration>,
    ) -> Result<Self> {
        let point = graph
            .points()
            .iter()
            .find(|point| point.descriptor().point_id.as_str() == PROVIDER_INPUT_PIPELINE_POINT_ID)
            .ok_or_else(|| anyhow::anyhow!("provider input pipeline point is unavailable"))?;
        let descriptor = point.descriptor();
        if descriptor.point_kind != ExtensionPointKind::Pipeline
            || descriptor.owner_module_id.as_str() != PROVIDER_INPUT_PIPELINE_OWNER_MODULE_ID
            || descriptor.contract.contract_id.as_str() != PROVIDER_INPUT_PIPELINE_CONTRACT_ID
            || descriptor.contract.contract_version.as_str()
                != PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION
            || descriptor.lifecycle != LifecycleSemantics::Invocation
            || descriptor.delivery != DeliverySemantics::Synchronous
            || descriptor.failure != FailureSemantics::FailClosed
            || descriptor.scope != ScopeSemantics::Global
            || descriptor.cardinality != Cardinality::Many
            || descriptor.ordering != OrderingSemantics::Dependency
            || descriptor.override_policy != OverridePolicy::Sealed
        {
            anyhow::bail!("provider input pipeline point contract mismatch");
        }
        let mut registrations_by_id = BTreeMap::new();
        for registration in registrations {
            if registrations_by_id
                .insert(registration.contribution_id.clone(), registration)
                .is_some()
            {
                anyhow::bail!("duplicate provider input contribution registration");
            }
        }
        let mut contributions = Vec::new();
        for contribution in point.contributions() {
            let contribution_id = contribution.descriptor().contribution_id.as_str();
            let registration = registrations_by_id
                .remove(contribution_id)
                .ok_or_else(|| anyhow::anyhow!("provider input contribution is not registered"))?;
            if registration.timeout.is_zero()
                || registration.failure_policy != ProviderInputContributionFailurePolicy::FailClosed
            {
                anyhow::bail!("provider input contribution policy is invalid");
            }
            contributions.push(OrderedContribution {
                contribution_id: contribution_id.to_string(),
                permissions: contribution
                    .descriptor()
                    .required_permissions
                    .iter()
                    .map(|permission| permission.as_str().to_string())
                    .collect(),
                registration,
            });
        }
        if !registrations_by_id.is_empty() {
            anyhow::bail!("provider input contribution is absent from the effective graph");
        }
        Ok(Self {
            graph_fingerprint: graph.fingerprint().as_str().to_string(),
            contributions,
        })
    }

    // The receipt is deliberately returned with the failure so callers can retain the
    // fail-closed contribution audit trail; boxing it would weaken that boundary.
    #[allow(clippy::result_large_err)]
    pub async fn execute(
        &self,
        mut input: ProviderInvocationInput,
    ) -> std::result::Result<ProviderInputPipelineOutput, ProviderInputPipelineError> {
        let pipeline_before = model_visible_digest(&input);
        let mut receipts = Vec::new();
        for (order, contribution) in self.contributions.iter().enumerate() {
            let before = input.clone();
            let before_digest = model_visible_digest(&before);
            let started = Instant::now();
            let execution =
                AssertUnwindSafe(contribution.registration.executor.rewrite(before.clone()))
                    .catch_unwind();
            let result = tokio::time::timeout(contribution.registration.timeout, execution).await;
            let candidate = match result {
                Ok(Ok(Ok(candidate))) => candidate,
                Ok(Ok(Err(_))) => {
                    return Err(self.failure(
                        "contribution_error",
                        &pipeline_before,
                        &input,
                        receipts,
                        contribution,
                        order,
                        before_digest,
                        None,
                        started,
                    ));
                }
                Ok(Err(_)) => {
                    return Err(self.failure(
                        "contribution_panic",
                        &pipeline_before,
                        &input,
                        receipts,
                        contribution,
                        order,
                        before_digest,
                        None,
                        started,
                    ));
                }
                Err(_) => {
                    return Err(self.failure(
                        "contribution_timeout",
                        &pipeline_before,
                        &input,
                        receipts,
                        contribution,
                        order,
                        before_digest,
                        None,
                        started,
                    ));
                }
            };
            let changed_fields = changed_model_visible_fields(&before, &candidate);
            if has_forbidden_changes(&before, &candidate)
                || changed_fields.iter().any(|field| {
                    !contribution
                        .permissions
                        .contains(permission_for_field(field))
                })
            {
                return Err(self.failure(
                    "unauthorized_rewrite",
                    &pipeline_before,
                    &input,
                    receipts,
                    contribution,
                    order,
                    before_digest,
                    Some(&candidate),
                    started,
                ));
            }
            let mut validated_candidate = candidate.clone();
            if validated_candidate
                .synchronize_required_capabilities()
                .is_err()
            {
                return Err(self.failure(
                    "invalid_model_visible_input",
                    &pipeline_before,
                    &input,
                    receipts,
                    contribution,
                    order,
                    before_digest,
                    Some(&candidate),
                    started,
                ));
            }
            let after_digest = model_visible_digest(&candidate);
            receipts.push(PipelineContributionReceipt {
                contribution_id: contribution.contribution_id.clone(),
                order,
                before_digest,
                after_digest,
                changed_fields,
                duration_micros: elapsed_micros(started),
                status: PipelineContributionStatus::Succeeded,
                error: None,
            });
            // Permission checks compare the contribution output before Core derives capability
            // metadata, so these host-owned fields cannot be mistaken for plugin rewrites.
            input = validated_candidate;
        }
        let receipt = build_receipt(
            &self.graph_fingerprint,
            pipeline_before,
            model_visible_digest(&input),
            receipts,
        );
        Ok(ProviderInputPipelineOutput {
            input,
            receipt: Some(receipt),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn failure(
        &self,
        code: &'static str,
        pipeline_before: &str,
        input: &ProviderInvocationInput,
        mut receipts: Vec<PipelineContributionReceipt>,
        contribution: &OrderedContribution,
        order: usize,
        before_digest: String,
        candidate: Option<&ProviderInvocationInput>,
        started: Instant,
    ) -> ProviderInputPipelineError {
        let (after_digest, changed_fields) = candidate.map_or_else(
            || (before_digest.clone(), Vec::new()),
            |candidate| {
                (
                    model_visible_digest(candidate),
                    changed_model_visible_fields(input, candidate),
                )
            },
        );
        receipts.push(PipelineContributionReceipt {
            contribution_id: contribution.contribution_id.clone(),
            order,
            before_digest,
            after_digest,
            changed_fields,
            duration_micros: elapsed_micros(started),
            status: PipelineContributionStatus::Failed,
            error: Some(code.to_string()),
        });
        ProviderInputPipelineError {
            code,
            receipt: build_receipt(
                &self.graph_fingerprint,
                pipeline_before.to_string(),
                model_visible_digest(input),
                receipts,
            ),
        }
    }
}

fn changed_model_visible_fields(
    before: &ProviderInvocationInput,
    after: &ProviderInvocationInput,
) -> Vec<String> {
    let mut changed = Vec::new();
    if before.messages != after.messages {
        changed.push("messages".to_string());
    }
    if before.system != after.system {
        changed.push("system".to_string());
    }
    if before.tools != after.tools {
        changed.push("tools".to_string());
    }
    if before.response_format != after.response_format {
        changed.push("response_format".to_string());
    }
    if before.model_parameters != after.model_parameters {
        changed.push("model_parameters".to_string());
    }
    changed
}

fn has_forbidden_changes(
    before: &ProviderInvocationInput,
    after: &ProviderInvocationInput,
) -> bool {
    let mut normalized = after.clone();
    normalized.messages = before.messages.clone();
    normalized.system = before.system.clone();
    normalized.tools = before.tools.clone();
    normalized.response_format = before.response_format.clone();
    normalized.model_parameters = before.model_parameters.clone();
    normalized != *before
}

fn permission_for_field(field: &str) -> &'static str {
    match field {
        "messages" => REWRITE_MESSAGES_PERMISSION,
        "system" => REWRITE_SYSTEM_PERMISSION,
        "tools" => REWRITE_TOOLS_PERMISSION,
        "response_format" => REWRITE_RESPONSE_FORMAT_PERMISSION,
        "model_parameters" => REWRITE_MODEL_PARAMETERS_PERMISSION,
        _ => "provider-input.invalid.write",
    }
}

fn model_visible_digest(input: &ProviderInvocationInput) -> String {
    let projection = (
        &input.messages,
        &input.system,
        &input.tools,
        &input.response_format,
        &input.model_parameters,
    );
    let encoded = serde_json::to_vec(&projection).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(encoded))
}

fn build_receipt(
    graph_fingerprint: &str,
    before_digest: String,
    after_digest: String,
    contributions: Vec<PipelineContributionReceipt>,
) -> PipelineExecutionReceipt {
    let digest_material = contributions
        .iter()
        .map(|receipt| {
            (
                &receipt.contribution_id,
                receipt.order,
                &receipt.before_digest,
                &receipt.after_digest,
                &receipt.changed_fields,
                receipt.status,
                &receipt.error,
            )
        })
        .collect::<Vec<_>>();
    let encoded = serde_json::to_vec(&(
        graph_fingerprint,
        PROVIDER_INPUT_PIPELINE_POINT_ID,
        &before_digest,
        &after_digest,
        digest_material,
    ))
    .unwrap_or_default();
    PipelineExecutionReceipt {
        graph_fingerprint: graph_fingerprint.to_string(),
        point_id: PROVIDER_INPUT_PIPELINE_POINT_ID.to_string(),
        before_digest,
        after_digest,
        receipt_digest: format!("sha256:{:x}", Sha256::digest(encoded)),
        contributions,
    }
}

fn elapsed_micros(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}
