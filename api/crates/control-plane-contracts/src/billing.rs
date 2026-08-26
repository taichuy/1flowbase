use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, Time};
use uuid::Uuid;

pub const CREDIT_UNIT_USD: &str = "USD";
pub const GLOBAL_ZERO_PROVIDER_CODE: &str = domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE;
pub const GLOBAL_ZERO_MODEL_ID: &str = domain::DEFAULT_MODEL_PRICING_MODEL_ID;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PricingRule {
    pub id: Uuid,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub input_token_unit_size: i64,
    pub input_token_unit_price: Decimal,
    pub output_token_unit_size: i64,
    pub output_token_unit_price: Decimal,
    pub cache_hit_token_unit_size: i64,
    pub cache_hit_token_unit_price: Decimal,
    pub currency_code: String,
    pub effective_from: OffsetDateTime,
    pub effective_to: Option<OffsetDateTime>,
    pub timezone: String,
    pub weekday_mask: i16,
    pub local_time_start: Option<Time>,
    pub local_time_end: Option<Time>,
    pub priority: i32,
    pub enabled: bool,
    pub rating_policy_enabled: bool,
    pub rating_policy: serde_json::Value,
    pub source_kind: String,
    pub source_catalog_id: Option<String>,
    pub source_version: Option<String>,
    pub source_checksum: Option<String>,
    pub extensions: serde_json::Value,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl PricingRule {
    pub fn validate(&self) -> Result<()> {
        if self.currency_code != CREDIT_UNIT_USD {
            return Err(anyhow!("billing_currency_not_supported"));
        }
        if [
            self.input_token_unit_size,
            self.output_token_unit_size,
            self.cache_hit_token_unit_size,
        ]
        .into_iter()
        .any(|value| value <= 0)
        {
            return Err(anyhow!("pricing_unit_size_invalid"));
        }
        if [
            self.input_token_unit_price,
            self.output_token_unit_price,
            self.cache_hit_token_unit_price,
        ]
        .into_iter()
        .any(|value| value.is_sign_negative())
        {
            return Err(anyhow!("pricing_unit_price_invalid"));
        }
        if self
            .effective_to
            .is_some_and(|end| end <= self.effective_from)
        {
            return Err(anyhow!("pricing_effective_range_invalid"));
        }
        if self.weekday_mask <= 0 || self.weekday_mask > 0b111_1111 {
            return Err(anyhow!("pricing_weekday_mask_invalid"));
        }
        match (self.local_time_start, self.local_time_end) {
            (None, None) => {}
            (Some(start), Some(end)) if start < end => {}
            (Some(start), Some(end)) if start > end && self.weekday_mask == 0b111_1111 => {}
            _ => return Err(anyhow!("pricing_local_time_range_invalid")),
        }
        if self.priority < 0 {
            return Err(anyhow!("pricing_priority_invalid"));
        }
        if self.rating_policy_enabled {
            validate_input_token_tier_policy(&self.rating_policy)?;
        }
        Ok(())
    }
}

const RATING_POLICY_SCHEMA_V1: &str = "1flowbase.model-rating-policy/v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputTokenTierPolicyDocument {
    schema_version: String,
    #[serde(rename = "type")]
    policy_type: String,
    tiers: Vec<InputTokenTierDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputTokenTierDocument {
    when: InputTokenThresholdDocument,
    rates: TokenRateSetDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputTokenThresholdDocument {
    operator: String,
    value: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenRateSetDocument {
    input: TokenRateDocument,
    output: TokenRateDocument,
    cache_hit: TokenRateDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenRateDocument {
    unit_size: i64,
    unit_price: String,
}

fn validate_token_rate(document: &TokenRateDocument) -> Result<()> {
    let unit_price = document
        .unit_price
        .parse::<Decimal>()
        .map_err(|_| anyhow!("rating_policy_invalid"))?;
    if document.unit_size <= 0 || unit_price.is_sign_negative() {
        return Err(anyhow!("rating_policy_invalid"));
    }
    Ok(())
}

fn validate_input_token_tier_policy(value: &serde_json::Value) -> Result<()> {
    let policy: InputTokenTierPolicyDocument =
        serde_json::from_value(value.clone()).map_err(|_| anyhow!("rating_policy_invalid"))?;
    if policy.schema_version != RATING_POLICY_SCHEMA_V1
        || policy.policy_type != "input_token_tiers"
        || policy.tiers.is_empty()
    {
        return Err(anyhow!("rating_policy_invalid"));
    }
    let mut previous_threshold = None;
    for tier in &policy.tiers {
        if tier.when.value < 0 || !matches!(tier.when.operator.as_str(), "gt" | "gte") {
            return Err(anyhow!("rating_policy_invalid"));
        }
        if previous_threshold.is_some_and(|previous| tier.when.value <= previous) {
            return Err(anyhow!("rating_policy_tiers_not_strictly_ascending"));
        }
        previous_threshold = Some(tier.when.value);
        validate_token_rate(&tier.rates.input)?;
        validate_token_rate(&tier.rates.output)?;
        validate_token_rate(&tier.rates.cache_hit)?;
    }
    Ok(())
}
