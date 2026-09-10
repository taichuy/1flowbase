use super::*;
use control_plane_contracts::billing::policy::{
    decimal, parse_rules, valid_ttl_seconds, TokenRateOverrides,
};

fn apply(rate: &mut TokenRate, size: Option<i64>, price: &Option<String>) -> Result<()> {
    if let Some(size) = size {
        rate.unit_size = size;
    }
    if let Some(price) = price {
        rate.unit_price = decimal(price)?;
    }
    Ok(())
}
fn apply_rates(rates: &mut AppliedTokenRates, o: &TokenRateOverrides) -> Result<()> {
    apply(
        &mut rates.input,
        o.input_token_unit_size,
        &o.input_token_unit_price,
    )?;
    apply(
        &mut rates.output,
        o.output_token_unit_size,
        &o.output_token_unit_price,
    )?;
    apply(
        &mut rates.cache_hit,
        o.cache_hit_token_unit_size,
        &o.cache_hit_token_unit_price,
    )?;
    apply(
        &mut rates.cache_write,
        o.cache_write_token_unit_size,
        &o.cache_write_token_unit_price,
    )
}
fn cost(tokens: i64, rate: TokenRate) -> Result<Decimal> {
    Decimal::from(tokens)
        .checked_mul(rate.unit_price)
        .and_then(|v| v.checked_div(Decimal::from(rate.unit_size)))
        .ok_or_else(|| anyhow!("rating_cost_overflow"))
}
fn add(a: Decimal, b: Decimal) -> Result<Decimal> {
    a.checked_add(b)
        .ok_or_else(|| anyhow!("rating_cost_overflow"))
}

pub(super) fn rate(
    rule: &PricingRule,
    usage: &TokenUsage,
    at: OffsetDateTime,
) -> Result<RatedTokenCost> {
    rule.validate()?;
    let rules = parse_rules(&rule.rules)?;
    if [
        usage.input_tokens,
        usage.input_cache_hit_tokens,
        usage.cache_write_tokens,
        usage.output_tokens,
    ]
    .into_iter()
    .any(|n| n < 0)
        || usage.input_cache_miss_tokens.is_some_and(|n| n < 0)
    {
        return Err(anyhow!("provider_usage_invalid"));
    }
    let ordinary_input_tokens = usage
        .input_tokens
        .checked_sub(usage.input_cache_hit_tokens)
        .and_then(|n| n.checked_sub(usage.cache_write_tokens))
        .filter(|n| *n >= 0)
        .ok_or_else(|| anyhow!("provider_usage_invalid"))?;
    if usage
        .input_cache_miss_tokens
        .is_some_and(|n| n != ordinary_input_tokens)
    {
        return Err(anyhow!("provider_usage_invalid"));
    }
    if let Some(buckets) = &usage.cache_write_by_ttl_seconds {
        let mut sum = 0i64;
        for (ttl, tokens) in buckets {
            if !valid_ttl_seconds(ttl) || *tokens < 0 {
                return Err(anyhow!("provider_usage_invalid"));
            }
            sum = sum
                .checked_add(*tokens)
                .ok_or_else(|| anyhow!("provider_usage_invalid"))?;
        }
        if sum != usage.cache_write_tokens {
            return Err(anyhow!("cache_write_usage_mismatch"));
        }
    } else if usage.cache_write_tokens > 0
        && rules
            .iter()
            .any(|r| r.when.cache_write_ttl_seconds.is_some())
    {
        return Err(anyhow!("cache_write_ttl_required"));
    }
    let mut applied_rates = base_token_rates(rule);
    let mut matched_rule_indices = Vec::new();
    // Every bucket starts from the same fixed defaults, then receives overrides in source order.
    if let Some(buckets) = &usage.cache_write_by_ttl_seconds {
        for ttl in buckets.keys() {
            applied_rates
                .cache_write_by_ttl_seconds
                .insert(ttl.clone(), applied_rates.cache_write);
        }
    }
    for (index, r) in rules.iter().enumerate() {
        let mut matched = false;
        if r.when.matches(usage.input_tokens, None, at)? {
            apply_rates(&mut applied_rates, &r.overrides)?;
            matched = true;
        }
        for (ttl, rate) in &mut applied_rates.cache_write_by_ttl_seconds {
            if r.when.matches(usage.input_tokens, ttl.parse().ok(), at)? {
                apply(
                    rate,
                    r.overrides.cache_write_token_unit_size,
                    &r.overrides.cache_write_token_unit_price,
                )?;
                matched = true;
            }
        }
        if matched {
            matched_rule_indices.push(index);
        }
    }
    let input_cost = cost(ordinary_input_tokens, applied_rates.input)?;
    let output_cost = cost(usage.output_tokens, applied_rates.output)?;
    let cache_hit_cost = cost(usage.input_cache_hit_tokens, applied_rates.cache_hit)?;
    let cache_write_cost = if let Some(buckets) = &usage.cache_write_by_ttl_seconds {
        let mut total = Decimal::ZERO;
        for (ttl, tokens) in buckets {
            total = add(
                total,
                cost(*tokens, applied_rates.cache_write_by_ttl_seconds[ttl])?,
            )?;
        }
        total
    } else {
        cost(usage.cache_write_tokens, applied_rates.cache_write)?
    };
    let total_cost = add(
        add(add(input_cost, output_cost)?, cache_hit_cost)?,
        cache_write_cost,
    )?;
    Ok(RatedTokenCost {
        ordinary_input_tokens,
        cache_hit_tokens: usage.input_cache_hit_tokens,
        cache_write_tokens: usage.cache_write_tokens,
        output_tokens: usage.output_tokens,
        input_cost,
        output_cost,
        cache_hit_cost,
        cache_write_cost,
        total_cost,
        applied_rates,
        matched_rule_indices,
    })
}
