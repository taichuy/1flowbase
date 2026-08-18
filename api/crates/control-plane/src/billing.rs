use crate::ports::{
    BillingRepository, CreditCommandInput, CreditOutboxEvent, CreditTransactionRecord,
    PluginCreditCommandRequest, PluginCreditCommandResult, ReserveCreditInput, SettleCreditInput,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, Time, Weekday};
use uuid::Uuid;

pub const CREDIT_UNIT_USD: &str = "USD";
pub const GLOBAL_ZERO_PROVIDER_CODE: &str = domain::DEFAULT_MODEL_PRICING_PROVIDER_CODE;
pub const GLOBAL_ZERO_MODEL_ID: &str = domain::DEFAULT_MODEL_PRICING_MODEL_ID;

pub fn pricing_rules_cache_key(provider_code: &str, upstream_model_id: &str) -> String {
    let digest = Sha256::digest(format!("{provider_code}\0{upstream_model_id}"));
    format!("model-pricing:rules:{digest:x}")
}

pub fn required_credit_permission(command: &str) -> Option<&'static str> {
    match command {
        "grant" => Some("credit.grant"),
        "charge" => Some("credit.charge"),
        "adjustment" => Some("credit.adjust"),
        "enable_charge" | "disable_charge" => Some("credit.toggle"),
        "refund" => Some("credit.refund"),
        "reserve" => Some("credit.reserve"),
        "settle" => Some("credit.settle"),
        "release" => Some("credit.release"),
        _ => None,
    }
}

pub struct CreditCommandService<R> {
    repository: R,
}

impl<R: BillingRepository> CreditCommandService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn execute_plugin_command(
        &self,
        plugin_id: &str,
        granted_permissions: &std::collections::BTreeSet<String>,
        mut input: CreditCommandInput,
    ) -> Result<CreditTransactionRecord> {
        let required = required_credit_permission(&input.command)
            .ok_or_else(|| anyhow!("credit_command_not_supported"))?;
        if !granted_permissions.contains(required) {
            self.repository
                .record_credit_command_rejected(
                    input.workspace_id,
                    plugin_id,
                    &input.command,
                    "credit_command_permission_denied",
                    &input.idempotency_key,
                )
                .await?;
            return Err(anyhow!("credit_command_permission_denied"));
        }
        if input.actor_user_id.is_some() {
            return Err(anyhow!("plugin_credit_actor_invalid"));
        }
        input.actor_plugin_id = Some(plugin_id.to_string());
        self.repository.execute_credit_command(&input).await
    }

    pub async fn execute_plugin_request(
        &self,
        workspace_id: Uuid,
        plugin_id: &str,
        granted_permissions: &std::collections::BTreeSet<String>,
        request: PluginCreditCommandRequest,
    ) -> Result<PluginCreditCommandResult> {
        let command = request.command.clone();
        let idempotency_key = request.idempotency_key.clone();
        let result = self
            .execute_verified_plugin_request(workspace_id, plugin_id, granted_permissions, request)
            .await;
        if let Err(error) = &result {
            self.repository
                .record_credit_command_rejected(
                    workspace_id,
                    plugin_id,
                    &command,
                    &error.to_string(),
                    &idempotency_key,
                )
                .await?;
        }
        result
    }

    async fn execute_verified_plugin_request(
        &self,
        workspace_id: Uuid,
        plugin_id: &str,
        granted_permissions: &std::collections::BTreeSet<String>,
        request: PluginCreditCommandRequest,
    ) -> Result<PluginCreditCommandResult> {
        let required = required_credit_permission(&request.command)
            .ok_or_else(|| anyhow!("credit_command_not_supported"))?;
        if !granted_permissions.contains(required) {
            return Err(anyhow!("credit_command_permission_denied"));
        }
        if request.credit_unit != CREDIT_UNIT_USD
            || request.reason.trim().is_empty()
            || request.idempotency_key.trim().is_empty()
            || request.source_type.is_some() != request.source_id.is_some()
        {
            return Err(anyhow!("credit_command_invalid"));
        }
        match request.command.as_str() {
            "grant" | "charge" | "adjustment" | "enable_charge" | "disable_charge" | "refund" => {
                let transaction = self
                    .execute_plugin_command(
                        plugin_id,
                        granted_permissions,
                        CreditCommandInput {
                            workspace_id,
                            user_id: request.user_id,
                            amount: request.amount,
                            credit_unit: request.credit_unit,
                            command: request.command,
                            reason: request.reason,
                            source_type: request.source_type,
                            source_id: request.source_id,
                            idempotency_key: request.idempotency_key,
                            actor_user_id: None,
                            actor_plugin_id: None,
                            metadata: request.metadata,
                        },
                    )
                    .await?;
                Ok(PluginCreditCommandResult::Transaction { transaction })
            }
            "reserve" => {
                let reservation = self
                    .repository
                    .reserve_credit(&ReserveCreditInput {
                        workspace_id,
                        user_id: request.user_id,
                        amount: request.amount,
                        flow_run_id: request.flow_run_id,
                        provider_invocation_id: request
                            .provider_invocation_id
                            .ok_or_else(|| anyhow!("provider_invocation_id_required"))?,
                        pricing_rule_id: request
                            .pricing_rule_id
                            .ok_or_else(|| anyhow!("pricing_rule_id_required"))?,
                        charge_enabled_default: !self
                            .repository
                            .credit_target_is_root(request.user_id)
                            .await?,
                        reservation_expires_at: request.reservation_expires_at.unwrap_or_else(
                            || OffsetDateTime::now_utc() + time::Duration::minutes(15),
                        ),
                    })
                    .await?;
                Ok(PluginCreditCommandResult::Reservation { reservation })
            }
            "settle" => {
                let billing_session_id = request
                    .billing_session_id
                    .ok_or_else(|| anyhow!("billing_session_id_required"))?;
                self.ensure_session_scope(workspace_id, request.user_id, billing_session_id)
                    .await?;
                let transaction = self
                    .repository
                    .settle_credit(&SettleCreditInput {
                        billing_session_id,
                        actual_amount: request.amount,
                        cost_ledger_id: None,
                        usage_ledger_id: None,
                        price_snapshot: request.price_snapshot,
                        usage_snapshot: request.usage_snapshot,
                    })
                    .await?;
                Ok(PluginCreditCommandResult::Transaction { transaction })
            }
            "release" => {
                let billing_session_id = request
                    .billing_session_id
                    .ok_or_else(|| anyhow!("billing_session_id_required"))?;
                if request.amount.parse::<Decimal>()? != Decimal::ZERO {
                    return Err(anyhow!("release_amount_must_be_zero"));
                }
                self.ensure_session_scope(workspace_id, request.user_id, billing_session_id)
                    .await?;
                let transaction = self
                    .repository
                    .release_credit(billing_session_id, &request.reason)
                    .await?
                    .ok_or_else(|| anyhow!("billing_session_not_releasable"))?;
                Ok(PluginCreditCommandResult::Released { transaction })
            }
            _ => Err(anyhow!("credit_command_not_supported")),
        }
    }

    async fn ensure_session_scope(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        billing_session_id: Uuid,
    ) -> Result<()> {
        if self
            .repository
            .billing_session_scope(billing_session_id)
            .await?
            != Some((workspace_id, user_id))
        {
            return Err(anyhow!("billing_session_scope_mismatch"));
        }
        Ok(())
    }
}

#[async_trait]
pub trait CreditEventPublisher: Send + Sync {
    async fn publish(&self, event: &CreditOutboxEvent) -> Result<()>;
}

pub async fn dispatch_credit_events<R: BillingRepository, P: CreditEventPublisher>(
    repository: &R,
    publisher: &P,
    worker_id: &str,
    limit: i64,
) -> Result<usize> {
    let events = repository
        .claim_credit_outbox_events(
            worker_id,
            limit,
            OffsetDateTime::now_utc() + time::Duration::seconds(30),
        )
        .await?;
    let mut published = 0;
    for event in events {
        match publisher.publish(&event).await {
            Ok(()) => {
                repository
                    .complete_credit_outbox_event(event.event_id, worker_id)
                    .await?;
                published += 1;
            }
            Err(error) => {
                repository
                    .fail_credit_outbox_event(event.event_id, worker_id, &error.to_string())
                    .await?;
            }
        }
    }
    Ok(published)
}

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
            validated_input_token_tier_policy(&self.rating_policy)?;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenRate {
    pub unit_size: i64,
    pub unit_price: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedTokenRates {
    pub input: TokenRate,
    pub output: TokenRate,
    pub cache_hit: TokenRate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RatingPolicyMatch {
    pub schema_version: &'static str,
    pub policy_type: &'static str,
    pub tier_index: usize,
    pub input_tokens: i64,
    pub operator: String,
    pub threshold: i64,
}

fn token_rate(document: &TokenRateDocument) -> Result<TokenRate> {
    let unit_price = document
        .unit_price
        .parse::<Decimal>()
        .map_err(|_| anyhow!("rating_policy_invalid"))?;
    if document.unit_size <= 0 || unit_price.is_sign_negative() {
        return Err(anyhow!("rating_policy_invalid"));
    }
    Ok(TokenRate {
        unit_size: document.unit_size,
        unit_price,
    })
}

fn validated_input_token_tier_policy(
    value: &serde_json::Value,
) -> Result<InputTokenTierPolicyDocument> {
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
        token_rate(&tier.rates.input)?;
        token_rate(&tier.rates.output)?;
        token_rate(&tier.rates.cache_hit)?;
    }
    Ok(policy)
}

fn base_token_rates(rule: &PricingRule) -> AppliedTokenRates {
    AppliedTokenRates {
        input: TokenRate {
            unit_size: rule.input_token_unit_size,
            unit_price: rule.input_token_unit_price,
        },
        output: TokenRate {
            unit_size: rule.output_token_unit_size,
            unit_price: rule.output_token_unit_price,
        },
        cache_hit: TokenRate {
            unit_size: rule.cache_hit_token_unit_size,
            unit_price: rule.cache_hit_token_unit_price,
        },
    }
}

fn applied_token_rates(
    rule: &PricingRule,
    input_tokens: i64,
) -> Result<(AppliedTokenRates, Option<RatingPolicyMatch>)> {
    if !rule.rating_policy_enabled {
        return Ok((base_token_rates(rule), None));
    }
    let policy = validated_input_token_tier_policy(&rule.rating_policy)?;
    let matched =
        policy
            .tiers
            .iter()
            .enumerate()
            .rev()
            .find(|(_, tier)| match tier.when.operator.as_str() {
                "gt" => input_tokens > tier.when.value,
                "gte" => input_tokens >= tier.when.value,
                _ => false,
            });
    let Some((tier_index, tier)) = matched else {
        return Ok((base_token_rates(rule), None));
    };
    Ok((
        AppliedTokenRates {
            input: token_rate(&tier.rates.input)?,
            output: token_rate(&tier.rates.output)?,
            cache_hit: token_rate(&tier.rates.cache_hit)?,
        },
        Some(RatingPolicyMatch {
            schema_version: RATING_POLICY_SCHEMA_V1,
            policy_type: "input_token_tiers",
            tier_index,
            input_tokens,
            operator: tier.when.operator.clone(),
            threshold: tier.when.value,
        }),
    ))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub input_cache_hit_tokens: i64,
    pub input_cache_miss_tokens: Option<i64>,
    pub output_tokens: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RatedTokenCost {
    pub ordinary_input_tokens: i64,
    pub cache_hit_tokens: i64,
    pub output_tokens: i64,
    pub input_cost: Decimal,
    pub output_cost: Decimal,
    pub cache_hit_cost: Decimal,
    pub total_cost: Decimal,
    pub applied_rates: AppliedTokenRates,
    pub rating_policy_match: Option<RatingPolicyMatch>,
}

pub fn rate_token_usage(rule: &PricingRule, usage: &TokenUsage) -> Result<RatedTokenCost> {
    rule.validate()?;
    if usage.input_tokens < 0
        || usage.input_cache_hit_tokens < 0
        || usage.input_cache_miss_tokens.is_some_and(|value| value < 0)
        || usage.output_tokens < 0
    {
        return Err(anyhow!("provider_usage_invalid"));
    }
    let cache_hit_tokens = usage.input_cache_hit_tokens;
    let ordinary_input_tokens = usage
        .input_cache_miss_tokens
        .unwrap_or_else(|| usage.input_tokens.saturating_sub(cache_hit_tokens));
    let (applied_rates, rating_policy_match) = applied_token_rates(rule, usage.input_tokens)?;
    let input_cost = Decimal::from(ordinary_input_tokens) * applied_rates.input.unit_price
        / Decimal::from(applied_rates.input.unit_size);
    let output_cost = Decimal::from(usage.output_tokens) * applied_rates.output.unit_price
        / Decimal::from(applied_rates.output.unit_size);
    let cache_hit_cost = Decimal::from(cache_hit_tokens) * applied_rates.cache_hit.unit_price
        / Decimal::from(applied_rates.cache_hit.unit_size);
    Ok(RatedTokenCost {
        ordinary_input_tokens,
        cache_hit_tokens,
        output_tokens: usage.output_tokens,
        input_cost,
        output_cost,
        cache_hit_cost,
        total_cost: input_cost + output_cost + cache_hit_cost,
        applied_rates,
        rating_policy_match,
    })
}

pub fn weekday_bit(weekday: Weekday) -> i16 {
    match weekday {
        Weekday::Monday => 1,
        Weekday::Tuesday => 2,
        Weekday::Wednesday => 4,
        Weekday::Thursday => 8,
        Weekday::Friday => 16,
        Weekday::Saturday => 32,
        Weekday::Sunday => 64,
    }
}

pub fn rule_matches_local_window(rule: &PricingRule, at: OffsetDateTime) -> Result<bool> {
    use time_tz::{timezones, OffsetDateTimeExt};
    if !rule.enabled || at < rule.effective_from || rule.effective_to.is_some_and(|end| at >= end) {
        return Ok(false);
    }
    let timezone = timezones::get_by_name(&rule.timezone)
        .ok_or_else(|| anyhow!("pricing_timezone_invalid"))?;
    let local = at.to_timezone(timezone);
    if rule.weekday_mask & weekday_bit(local.weekday()) == 0 {
        return Ok(false);
    }
    Ok(match (rule.local_time_start, rule.local_time_end) {
        (None, None) => true,
        (Some(start), Some(end)) if start < end => local.time() >= start && local.time() < end,
        (Some(start), Some(end)) if start > end => local.time() >= start || local.time() < end,
        _ => false,
    })
}

pub async fn resolve_pricing_rule<R: BillingRepository + ?Sized>(
    repository: &R,
    provider_code: &str,
    upstream_model_id: &str,
    at: OffsetDateTime,
) -> Result<Option<PricingRule>> {
    choose_pricing_rule_for(
        provider_code,
        upstream_model_id,
        repository
            .match_pricing_rules(provider_code, upstream_model_id, at)
            .await?,
        at,
    )
}

pub fn choose_pricing_rule_for(
    provider_code: &str,
    upstream_model_id: &str,
    candidates: Vec<PricingRule>,
    at: OffsetDateTime,
) -> Result<Option<PricingRule>> {
    let mut exact = Vec::new();
    let mut fallback = Vec::new();
    for rule in candidates {
        if rule.provider_code == provider_code && rule.upstream_model_id == upstream_model_id {
            exact.push(rule);
        } else if rule.provider_code == GLOBAL_ZERO_PROVIDER_CODE
            && rule.upstream_model_id == GLOBAL_ZERO_MODEL_ID
        {
            fallback.push(rule);
        }
    }
    match choose_pricing_rule(exact, at)? {
        Some(rule) => Ok(Some(rule)),
        None => choose_pricing_rule(fallback, at),
    }
}

pub fn choose_pricing_rule(
    candidates: Vec<PricingRule>,
    at: OffsetDateTime,
) -> Result<Option<PricingRule>> {
    let mut matches = candidates
        .into_iter()
        .filter_map(|rule| match rule_matches_local_window(&rule, at) {
            Ok(true) => Some(Ok(rule)),
            Ok(false) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>>>()?;
    matches.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| right.effective_from.cmp(&left.effective_from))
            .then_with(|| left.id.cmp(&right.id))
    });
    if matches.len() > 1 && matches[0].priority == matches[1].priority {
        return Err(anyhow!("pricing_rule_conflict"));
    }
    Ok(matches.into_iter().next())
}
