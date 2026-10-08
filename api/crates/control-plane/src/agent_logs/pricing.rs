//! Imported log estimates use model identity, independently of client provider names.
use crate::ports::{AgentLogUsage, AgentLogUsageBasis, BillingRepository};
use anyhow::Result;

pub(super) async fn rate_usage<R: BillingRepository>(
    repository: &R,
    model_id: Option<&str>,
    occurred_at: &str,
    usage: Option<&AgentLogUsage>,
    inherited: bool,
) -> Result<Option<String>> {
    let Some(usage) = usage else { return Ok(None) };
    if inherited
        || (usage.basis == AgentLogUsageBasis::Cumulative
            && usage.response_id.as_deref().is_none_or(|id| id.is_empty()))
    {
        return Ok(None);
    }
    let at =
        time::OffsetDateTime::parse(occurred_at, &time::format_description::well_known::Rfc3339)?;
    // The repository orders exact IDs before the default, using the pricing list order.
    // Multiple eligible matches intentionally select the first; native billing is unchanged.
    for rule in repository
        .match_agent_log_pricing_rules(model_id.unwrap_or(""), at)
        .await?
    {
        if !crate::billing::rule_matches_local_window(&rule, at)? {
            continue;
        }
        let all_zero = control_plane_contracts::billing::policy::parse_rules(&rule.rules)?
            .is_empty()
            && rule.input_token_unit_price.is_zero()
            && rule.output_token_unit_price.is_zero()
            && rule.cache_hit_token_unit_price.is_zero()
            && rule.cache_write_token_unit_price.is_zero();
        if all_zero
            && [
                usage.total_tokens,
                usage.input_tokens,
                usage.output_tokens,
                usage.input_cache_hit_tokens,
                usage.cache_write_tokens,
            ]
            .into_iter()
            .any(|value| value.is_some())
        {
            return Ok(Some("0".to_owned()));
        }
        return match (
            usage.input_tokens,
            usage.output_tokens,
            usage.input_cache_hit_tokens,
            usage.cache_write_tokens,
        ) {
            (
                Some(input_tokens),
                Some(output_tokens),
                Some(input_cache_hit_tokens),
                Some(cache_write_tokens),
            ) => Ok(Some(
                crate::billing::rate_token_usage_at(
                    &rule,
                    &crate::billing::TokenUsage {
                        input_tokens,
                        output_tokens,
                        input_cache_hit_tokens,
                        cache_write_tokens,
                        ..Default::default()
                    },
                    at,
                )?
                .total_cost
                .to_string(),
            )),
            _ => Ok(None),
        };
    }
    Ok(None)
}
