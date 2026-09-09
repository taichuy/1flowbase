use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::BTreeMap;

pub const RATING_POLICY_SCHEMA_V2: &str = "1flowbase.model-rating-policy/v2";

#[derive(Debug, Clone, PartialEq)]
pub enum CacheWriteRate {
    UnitPrice(Decimal),
    ByTtlSeconds(BTreeMap<String, Decimal>),
}

#[derive(Debug, Clone)]
pub struct TokenPricingRates {
    pub input: Decimal,
    pub output: Decimal,
    pub cache_hit: Decimal,
    pub cache_write: CacheWriteRate,
}

#[derive(Debug)]
pub struct TokenPricingTier {
    pub operator: String,
    pub value: i64,
    pub rates: TokenPricingRates,
}

#[derive(Debug)]
pub struct TokenPricingPolicy {
    pub unit_size: i64,
    pub rates: TokenPricingRates,
    pub input_token_tiers: Vec<TokenPricingTier>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: String,
    #[serde(rename = "type")]
    policy_type: String,
    unit_size: i64,
    rates: RatesDocument,
    #[serde(default)]
    input_token_tiers: Option<Vec<TierDocument>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RatesDocument {
    input: String,
    output: String,
    cache_hit: String,
    cache_write: WriteDocument,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum WriteDocument {
    Unit(UnitDocument),
    Ttl(TtlDocument),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitDocument {
    unit_price: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TtlDocument {
    by_ttl_seconds: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TierDocument {
    when: ThresholdDocument,
    rates: RatesDocument,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdDocument {
    operator: String,
    value: i64,
}

pub fn valid_ttl_seconds(key: &str) -> bool {
    !key.starts_with('0')
        && key.bytes().all(|byte| byte.is_ascii_digit())
        && key
            .parse::<u64>()
            .is_ok_and(|value| value > 0 && value <= 9_007_199_254_740_991)
}

fn decimal(value: &str) -> Result<Decimal> {
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.is_some_and(|part| {
            part.is_empty() || part.len() > 18 || !part.bytes().all(|byte| byte.is_ascii_digit())
        })
        || parts.next().is_some()
    {
        return Err(anyhow!("rating_policy_invalid"));
    }
    Decimal::from_str_exact(value).map_err(|_| anyhow!("rating_policy_invalid"))
}

impl RatesDocument {
    fn parse(self) -> Result<TokenPricingRates> {
        let cache_write = match self.cache_write {
            WriteDocument::Unit(document) => {
                CacheWriteRate::UnitPrice(decimal(&document.unit_price)?)
            }
            WriteDocument::Ttl(document) => {
                if document.by_ttl_seconds.is_empty() {
                    return Err(anyhow!("rating_policy_invalid"));
                }
                let prices = document
                    .by_ttl_seconds
                    .into_iter()
                    .map(|(key, value)| {
                        if !valid_ttl_seconds(&key) {
                            return Err(anyhow!("rating_policy_invalid"));
                        }
                        Ok((key, decimal(&value)?))
                    })
                    .collect::<Result<_>>()?;
                CacheWriteRate::ByTtlSeconds(prices)
            }
        };
        Ok(TokenPricingRates {
            input: decimal(&self.input)?,
            output: decimal(&self.output)?,
            cache_hit: decimal(&self.cache_hit)?,
            cache_write,
        })
    }
}

impl TokenPricingPolicy {
    pub fn parse(value: &serde_json::Value) -> Result<Self> {
        let document: Document =
            serde_json::from_value(value.clone()).map_err(|_| anyhow!("rating_policy_invalid"))?;
        if document.schema_version != RATING_POLICY_SCHEMA_V2
            || document.policy_type != "token_pricing"
            || document.unit_size <= 0
            || document.unit_size > 9_007_199_254_740_991
        {
            return Err(anyhow!("rating_policy_invalid"));
        }
        if value.get("input_token_tiers").is_some()
            && document
                .input_token_tiers
                .as_ref()
                .is_none_or(|tiers| tiers.is_empty())
        {
            return Err(anyhow!("rating_policy_invalid"));
        }
        let mut previous = None;
        let mut tiers = Vec::new();
        for tier in document.input_token_tiers.unwrap_or_default() {
            if tier.when.value < 0
                || tier.when.value > 9_007_199_254_740_991
                || !matches!(tier.when.operator.as_str(), "gt" | "gte")
            {
                return Err(anyhow!("rating_policy_invalid"));
            }
            if previous.is_some_and(|value| tier.when.value <= value) {
                return Err(anyhow!("rating_policy_tiers_not_strictly_ascending"));
            }
            previous = Some(tier.when.value);
            tiers.push(TokenPricingTier {
                operator: tier.when.operator,
                value: tier.when.value,
                rates: tier.rates.parse()?,
            });
        }
        Ok(Self {
            unit_size: document.unit_size,
            rates: document.rates.parse()?,
            input_token_tiers: tiers,
        })
    }
}

#[cfg(test)]
#[path = "_tests/policy_tests.rs"]
mod tests;
