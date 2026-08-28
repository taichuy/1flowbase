use super::*;
use extension_contracts::{
    ProviderDistributionCandidate, ProviderDistributionDecision,
    ProviderDistributionSelectionReceipt,
};

const BUILTIN_RULE_VERSION: &str = "1";
const LLM_ROUTING_COUNTER_TTL: time::Duration = time::Duration::hours(1);

pub(super) struct ProviderDistributionSelection {
    pub(super) target_index: usize,
    pub(super) receipt: ProviderDistributionSelectionReceipt,
}

pub(super) async fn select_provider_target<I>(
    rule: &crate::compiled_plan::LlmDistributionRule,
    distribution_key: Option<&str>,
    target_ids: &[String],
    attempt_index: usize,
    runtime_context: &ExecutionRuntimeContext,
    invoker: &I,
) -> Result<ProviderDistributionSelection>
where
    I: ProviderInvoker + ?Sized,
{
    let candidates = target_ids
        .iter()
        .enumerate()
        .map(|(index, target_id)| ProviderDistributionCandidate {
            target_id: target_id.clone(),
            order: index as u32,
            ready: true,
            capabilities: BTreeSet::new(),
        })
        .collect::<Vec<_>>();
    let registry_fingerprint = runtime_context
        .provider_distribution_registry_fingerprint(invoker)
        .await?
        .to_string();
    let target_index = match rule {
        crate::compiled_plan::LlmDistributionRule::None => 0,
        crate::compiled_plan::LlmDistributionRule::RetryRoundRobin => {
            attempt_index % candidates.len()
        }
        crate::compiled_plan::LlmDistributionRule::RoundRobin if candidates.len() > 1 => {
            let key = distribution_key
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("round_robin llm routing is missing distribution_key"))?;
            let pin = runtime_context.round_robin_pin(key)?;
            *pin.get_or_try_init(|| async {
                let counter = runtime_context
                    .next_llm_routing_counter(key, Some(LLM_ROUTING_COUNTER_TTL))
                    .await?;
                Ok::<usize, anyhow::Error>(
                    (counter - 1).rem_euclid(candidates.len() as i64) as usize
                )
            })
            .await?
        }
        crate::compiled_plan::LlmDistributionRule::RoundRobin => 0,
        crate::compiled_plan::LlmDistributionRule::Dynamic {
            rule_id,
            contract_version,
            config,
        } => {
            let invocation = extension_contracts::ProviderDistributionInvocation {
                invocation_id: runtime_context
                    .provider_distribution_invocation_id()
                    .to_string(),
                conversation_id: runtime_context
                    .provider_distribution_conversation_id
                    .clone(),
                routing_policy_id: distribution_key.unwrap_or(rule_id).to_string(),
                attempt: attempt_index as u32,
                rule_id: rule_id.clone(),
                rule_version: contract_version.clone(),
                registry_fingerprint: registry_fingerprint.clone(),
                config: config.clone(),
                candidates: candidates.clone(),
            };
            let receipt = invoker
                .select_provider_distribution(rule_id, invocation)
                .await?;
            let target_id = match &receipt.decision {
                ProviderDistributionDecision::Select { target_id } => target_id,
                ProviderDistributionDecision::NoEligibleTarget { reason } => {
                    return Err(anyhow!(
                        "provider distribution found no eligible target: {reason}"
                    ));
                }
            };
            let target_index = candidates
                .iter()
                .position(|candidate| candidate.ready && candidate.target_id == *target_id)
                .ok_or_else(|| anyhow!("provider distribution selected an ineligible target"))?;
            return Ok(ProviderDistributionSelection {
                target_index,
                receipt,
            });
        }
    };
    let selected = candidates
        .get(target_index)
        .filter(|candidate| candidate.ready)
        .ok_or_else(|| anyhow!("provider distribution selected an ineligible target"))?;
    let rule_id = match rule {
        crate::compiled_plan::LlmDistributionRule::None => "builtin.none",
        crate::compiled_plan::LlmDistributionRule::RoundRobin => "builtin.round_robin",
        crate::compiled_plan::LlmDistributionRule::RetryRoundRobin => "builtin.retry_round_robin",
        crate::compiled_plan::LlmDistributionRule::Dynamic { .. } => unreachable!(),
    };
    Ok(ProviderDistributionSelection {
        target_index,
        receipt: ProviderDistributionSelectionReceipt {
            invocation_id: runtime_context
                .provider_distribution_invocation_id()
                .to_string(),
            rule_id: rule_id.to_string(),
            rule_version: BUILTIN_RULE_VERSION.to_string(),
            registry_fingerprint,
            attempt: attempt_index as u32,
            decision: ProviderDistributionDecision::Select {
                target_id: selected.target_id.clone(),
            },
        },
    })
}
