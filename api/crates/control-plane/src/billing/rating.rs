use super::*;
use control_plane_contracts::billing::policy::{
    valid_ttl_seconds, TokenPricingPolicy, RATING_POLICY_SCHEMA_V2,
};

pub(super) fn applied_v2_rates(
    rule: &PricingRule,
    input_tokens: i64,
) -> Result<(AppliedTokenRates, Option<RatingPolicyMatch>)> {
    let policy = TokenPricingPolicy::parse(&rule.rating_policy)?;
    let matched = policy
        .input_token_tiers
        .iter()
        .enumerate()
        .rev()
        .find(|(_, tier)| match tier.operator.as_str() {
            "gt" => input_tokens > tier.value,
            "gte" => input_tokens >= tier.value,
            _ => false,
        });
    let rates = matched.map_or(&policy.rates, |(_, tier)| &tier.rates);
    let rate = |unit_price| TokenRate {
        unit_size: policy.unit_size,
        unit_price,
    };
    Ok((
        AppliedTokenRates {
            input: rate(rates.input),
            output: rate(rates.output),
            cache_hit: rate(rates.cache_hit),
            cache_write: Some(rates.cache_write.clone()),
        },
        matched.map(|(tier_index, tier)| RatingPolicyMatch {
            schema_version: RATING_POLICY_SCHEMA_V2,
            policy_type: "token_pricing",
            tier_index,
            input_tokens,
            operator: tier.operator.clone(),
            threshold: tier.value,
        }),
    ))
}

fn cost(tokens: i64, rate: TokenRate) -> Result<Decimal> {
    Decimal::from(tokens)
        .checked_mul(rate.unit_price)
        .and_then(|value| value.checked_div(Decimal::from(rate.unit_size)))
        .ok_or_else(|| anyhow!("rating_cost_overflow"))
}
fn add(left: Decimal, right: Decimal) -> Result<Decimal> {
    left.checked_add(right)
        .ok_or_else(|| anyhow!("rating_cost_overflow"))
}

pub(super) fn rate(rule: &PricingRule, usage: &TokenUsage) -> Result<RatedTokenCost> {
    rule.validate()?;
    if usage.input_tokens < 0
        || usage.input_cache_hit_tokens < 0
        || usage.cache_write_tokens < 0
        || usage.input_cache_miss_tokens.is_some_and(|value| value < 0)
        || usage.output_tokens < 0
    {
        return Err(anyhow!("provider_usage_invalid"));
    }
    let v2 = rule.rating_policy_enabled
        && rule.rating_policy["schema_version"] == RATING_POLICY_SCHEMA_V2;
    let cache_hit_tokens = usage.input_cache_hit_tokens;
    let ordinary_input_tokens = if v2 {
        let ordinary = usage
            .input_tokens
            .checked_sub(cache_hit_tokens)
            .and_then(|value| value.checked_sub(usage.cache_write_tokens))
            .filter(|value| *value >= 0)
            .ok_or_else(|| anyhow!("provider_usage_invalid"))?;
        if usage
            .input_cache_miss_tokens
            .is_some_and(|value| value != ordinary)
        {
            return Err(anyhow!("provider_usage_invalid"));
        }
        ordinary
    } else {
        usage
            .input_cache_miss_tokens
            .unwrap_or_else(|| usage.input_tokens.saturating_sub(cache_hit_tokens))
    };
    let (applied_rates, rating_policy_match) = applied_token_rates(rule, usage.input_tokens)?;
    let input_cost = cost(ordinary_input_tokens, applied_rates.input)?;
    let output_cost = cost(usage.output_tokens, applied_rates.output)?;
    let cache_hit_cost = cost(cache_hit_tokens, applied_rates.cache_hit)?;
    let mut cache_write_cost = Decimal::ZERO;
    if v2 {
        if let Some(buckets) = &usage.cache_write_by_ttl_seconds {
            let mut sum = 0i64;
            for (ttl, tokens) in buckets {
                if !valid_ttl_seconds(ttl) || *tokens < 0 {
                    return Err(anyhow!("provider_usage_invalid"));
                }
                sum = sum
                    .checked_add(*tokens)
                    .ok_or_else(|| anyhow!("provider_usage_invalid"))?;
                if let Some(CacheWriteRate::ByTtlSeconds(prices)) = &applied_rates.cache_write {
                    if !prices.contains_key(ttl) {
                        return Err(anyhow!("cache_write_ttl_unpriced"));
                    }
                }
            }
            if sum != usage.cache_write_tokens {
                return Err(anyhow!("cache_write_usage_mismatch"));
            }
        }
        if usage.cache_write_tokens > 0 {
            match &applied_rates.cache_write {
                Some(CacheWriteRate::UnitPrice(price)) => {
                    cache_write_cost = cost(
                        usage.cache_write_tokens,
                        TokenRate {
                            unit_size: applied_rates.input.unit_size,
                            unit_price: *price,
                        },
                    )?;
                }
                Some(CacheWriteRate::ByTtlSeconds(prices)) => {
                    let buckets = usage
                        .cache_write_by_ttl_seconds
                        .as_ref()
                        .ok_or_else(|| anyhow!("cache_write_ttl_required"))?;
                    for (ttl, tokens) in buckets {
                        let price = prices
                            .get(ttl)
                            .ok_or_else(|| anyhow!("cache_write_ttl_unpriced"))?;
                        cache_write_cost = add(
                            cache_write_cost,
                            cost(
                                *tokens,
                                TokenRate {
                                    unit_size: applied_rates.input.unit_size,
                                    unit_price: *price,
                                },
                            )?,
                        )?;
                    }
                }
                None => return Err(anyhow!("cache_write_rate_required")),
            }
        }
    }
    let total_cost = add(
        add(add(input_cost, output_cost)?, cache_hit_cost)?,
        cache_write_cost,
    )?;
    Ok(RatedTokenCost {
        ordinary_input_tokens,
        cache_hit_tokens,
        cache_write_tokens: usage.cache_write_tokens,
        output_tokens: usage.output_tokens,
        input_cost,
        output_cost,
        cache_hit_cost,
        cache_write_cost,
        total_cost,
        applied_rates,
        rating_policy_match,
    })
}
