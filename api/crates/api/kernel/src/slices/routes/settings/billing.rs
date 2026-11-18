use crate::error_response::ApiError;
use control_plane::billing::PricingRule;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::str::FromStr;
use time::{OffsetDateTime, Time};
use utoipa::ToSchema;
use uuid::Uuid;
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PricingRuleBody {
    pub id: Option<Uuid>,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub input_token_unit_size: i64,
    pub input_token_unit_price: String,
    pub output_token_unit_size: i64,
    pub output_token_unit_price: String,
    pub cache_hit_token_unit_size: i64,
    pub cache_hit_token_unit_price: String,
    pub cache_write_token_unit_size: i64,
    pub cache_write_token_unit_price: String,
    pub currency_code: Option<String>,
    #[schema(value_type = String, format = DateTime)]
    #[serde(with = "time::serde::rfc3339")]
    pub effective_from: OffsetDateTime,
    #[schema(value_type = Option<String>, format = DateTime)]
    #[serde(with = "time::serde::rfc3339::option")]
    pub effective_to: Option<OffsetDateTime>,
    pub timezone: String,
    pub weekday_mask: i16,
    pub local_time_start: Option<String>,
    pub local_time_end: Option<String>,
    pub priority: i32,
    pub enabled: bool,
    pub rules: Value,
    pub source_kind: Option<String>,
    pub source_catalog_id: Option<String>,
    pub source_version: Option<String>,
    pub source_checksum: Option<String>,
    pub extensions: Option<Value>,
}

pub fn body_to_rule(
    body: PricingRuleBody,
    actor_user_id: Uuid,
    forced_id: Option<Uuid>,
) -> Result<PricingRule, ApiError> {
    let now = OffsetDateTime::now_utc();
    let rule = PricingRule {
        id: forced_id.or(body.id).unwrap_or_else(Uuid::now_v7),
        provider_code: body.provider_code,
        upstream_model_id: body.upstream_model_id,
        input_token_unit_size: body.input_token_unit_size,
        input_token_unit_price: Decimal::from_str(&body.input_token_unit_price).map_err(|_| {
            control_plane::errors::ControlPlaneError::InvalidInput("input_token_unit_price")
        })?,
        output_token_unit_size: body.output_token_unit_size,
        output_token_unit_price: Decimal::from_str(&body.output_token_unit_price).map_err(
            |_| control_plane::errors::ControlPlaneError::InvalidInput("output_token_unit_price"),
        )?,
        cache_hit_token_unit_size: body.cache_hit_token_unit_size,
        cache_hit_token_unit_price: Decimal::from_str(&body.cache_hit_token_unit_price).map_err(
            |_| {
                control_plane::errors::ControlPlaneError::InvalidInput("cache_hit_token_unit_price")
            },
        )?,
        cache_write_token_unit_size: body.cache_write_token_unit_size,
        cache_write_token_unit_price: Decimal::from_str(&body.cache_write_token_unit_price)
            .map_err(|_| {
                control_plane::errors::ControlPlaneError::InvalidInput(
                    "cache_write_token_unit_price",
                )
            })?,
        currency_code: body.currency_code.unwrap_or_else(|| "USD".into()),
        effective_from: body.effective_from,
        effective_to: body.effective_to,
        timezone: body.timezone,
        weekday_mask: body.weekday_mask,
        local_time_start: body
            .local_time_start
            .map(|value| {
                Time::parse(
                    &value,
                    time::macros::format_description!("[hour]:[minute]:[second]"),
                )
            })
            .transpose()
            .map_err(|_| {
                control_plane::errors::ControlPlaneError::InvalidInput("local_time_start")
            })?,
        local_time_end: body
            .local_time_end
            .map(|value| {
                Time::parse(
                    &value,
                    time::macros::format_description!("[hour]:[minute]:[second]"),
                )
            })
            .transpose()
            .map_err(|_| {
                control_plane::errors::ControlPlaneError::InvalidInput("local_time_end")
            })?,
        priority: body.priority,
        enabled: body.enabled,
        rules: body.rules,
        source_kind: body.source_kind.unwrap_or_else(|| "manual".into()),
        source_catalog_id: body.source_catalog_id,
        source_version: body.source_version,
        source_checksum: body.source_checksum,
        extensions: body.extensions.unwrap_or_else(|| serde_json::json!({})),
        created_by: Some(actor_user_id),
        created_at: now,
        updated_at: now,
    };
    rule.validate().map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("pricing_rule_invalid")
    })?;
    Ok(rule)
}
