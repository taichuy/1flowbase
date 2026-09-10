use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, Time};
use time_tz::{timezones, OffsetDateTimeExt};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingOverride {
    pub when: PricingCondition,
    pub overrides: TokenRateOverrides,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputThreshold {
    pub operator: String,
    pub value: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingCondition {
    pub input_tokens: Option<InputThreshold>,
    pub cache_write_ttl_seconds: Option<i64>,
    pub effective_from: Option<String>,
    pub effective_to: Option<String>,
    pub timezone: Option<String>,
    pub weekday_mask: Option<i16>,
    pub local_time_start: Option<String>,
    pub local_time_end: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenRateOverrides {
    pub input_token_unit_size: Option<i64>,
    pub input_token_unit_price: Option<String>,
    pub output_token_unit_size: Option<i64>,
    pub output_token_unit_price: Option<String>,
    pub cache_hit_token_unit_size: Option<i64>,
    pub cache_hit_token_unit_price: Option<String>,
    pub cache_write_token_unit_size: Option<i64>,
    pub cache_write_token_unit_price: Option<String>,
}

pub fn valid_ttl_seconds(key: &str) -> bool {
    !key.starts_with('0')
        && key.bytes().all(|b| b.is_ascii_digit())
        && key.parse::<i64>().is_ok_and(|n| n > 0)
}

pub fn decimal(value: &str) -> Result<Decimal> {
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction
            .is_some_and(|p| p.is_empty() || p.len() > 18 || !p.bytes().all(|b| b.is_ascii_digit()))
        || parts.next().is_some()
    {
        return Err(anyhow!("pricing_rules_invalid"));
    }
    Decimal::from_str_exact(value).map_err(|_| anyhow!("pricing_rules_invalid"))
}

fn local_time(value: &str) -> Result<Time> {
    let bytes = value.as_bytes();
    if bytes.len() != 8
        || bytes[2] != b':'
        || bytes[5] != b':'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 2 || i == 5 || b.is_ascii_digit())
    {
        return Err(anyhow!("pricing_rules_invalid"));
    }
    Time::from_hms(
        (bytes[0] - b'0') * 10 + bytes[1] - b'0',
        (bytes[3] - b'0') * 10 + bytes[4] - b'0',
        (bytes[6] - b'0') * 10 + bytes[7] - b'0',
    )
    .map_err(|_| anyhow!("pricing_rules_invalid"))
}
fn date(value: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| anyhow!("pricing_rules_invalid"))
}

pub fn parse_rules(value: &serde_json::Value) -> Result<Vec<PricingOverride>> {
    let array = value
        .as_array()
        .ok_or_else(|| anyhow!("pricing_rules_invalid"))?;
    for item in array {
        for name in ["when", "overrides"] {
            let object = item
                .get(name)
                .and_then(|v| v.as_object())
                .ok_or_else(|| anyhow!("pricing_rules_invalid"))?;
            if object.is_empty() || object.values().any(|v| v.is_null()) {
                return Err(anyhow!("pricing_rules_invalid"));
            }
        }
    }
    let rules: Vec<PricingOverride> =
        serde_json::from_value(value.clone()).map_err(|_| anyhow!("pricing_rules_invalid"))?;
    for rule in &rules {
        let c = &rule.when;
        if c.input_tokens
            .as_ref()
            .is_some_and(|t| t.value < 0 || !matches!(t.operator.as_str(), "gt" | "gte"))
            || c.cache_write_ttl_seconds.is_some_and(|v| v <= 0)
            || c.weekday_mask.is_some_and(|v| !(1..=127).contains(&v))
        {
            return Err(anyhow!("pricing_rules_invalid"));
        }
        let from = c.effective_from.as_deref().map(date).transpose()?;
        let to = c.effective_to.as_deref().map(date).transpose()?;
        if from.zip(to).is_some_and(|(a, b)| a >= b) {
            return Err(anyhow!("pricing_rules_invalid"));
        }
        if let Some(tz) = &c.timezone {
            if tz.contains(' ') || timezones::get_by_name(tz).is_none() {
                return Err(anyhow!("pricing_rules_invalid"));
            }
        }
        if c.weekday_mask.is_some() && c.timezone.is_none() {
            return Err(anyhow!("pricing_rules_invalid"));
        }
        match (&c.local_time_start, &c.local_time_end) {
            (None, None) => {}
            (Some(a), Some(b)) if c.timezone.is_some() => {
                if local_time(a)? == local_time(b)? {
                    return Err(anyhow!("pricing_rules_invalid"));
                }
            }
            _ => return Err(anyhow!("pricing_rules_invalid")),
        }
        // A timezone alone does not constitute a pricing condition.
        if c.input_tokens.is_none()
            && c.cache_write_ttl_seconds.is_none()
            && from.is_none()
            && to.is_none()
            && c.weekday_mask.is_none()
            && c.local_time_start.is_none()
        {
            return Err(anyhow!("pricing_rules_invalid"));
        }
        let o = &rule.overrides;
        for size in [
            o.input_token_unit_size,
            o.output_token_unit_size,
            o.cache_hit_token_unit_size,
            o.cache_write_token_unit_size,
        ]
        .into_iter()
        .flatten()
        {
            if size <= 0 {
                return Err(anyhow!("pricing_rules_invalid"));
            }
        }
        for price in [
            &o.input_token_unit_price,
            &o.output_token_unit_price,
            &o.cache_hit_token_unit_price,
            &o.cache_write_token_unit_price,
        ]
        .into_iter()
        .flatten()
        {
            decimal(price)?;
        }
        if c.cache_write_ttl_seconds.is_some()
            && (o.input_token_unit_size.is_some()
                || o.input_token_unit_price.is_some()
                || o.output_token_unit_size.is_some()
                || o.output_token_unit_price.is_some()
                || o.cache_hit_token_unit_size.is_some()
                || o.cache_hit_token_unit_price.is_some())
        {
            return Err(anyhow!("pricing_rules_invalid"));
        }
    }
    Ok(rules)
}

impl PricingCondition {
    pub fn matches(&self, input_tokens: i64, ttl: Option<i64>, at: OffsetDateTime) -> Result<bool> {
        if self.input_tokens.as_ref().is_some_and(|t| {
            if t.operator == "gt" {
                input_tokens <= t.value
            } else {
                input_tokens < t.value
            }
        }) || self.cache_write_ttl_seconds.is_some_and(|n| Some(n) != ttl)
        {
            return Ok(false);
        }
        if self
            .effective_from
            .as_deref()
            .map(date)
            .transpose()?
            .is_some_and(|v| at < v)
            || self
                .effective_to
                .as_deref()
                .map(date)
                .transpose()?
                .is_some_and(|v| at >= v)
        {
            return Ok(false);
        }
        let local = match &self.timezone {
            Some(tz) => at.to_timezone(
                timezones::get_by_name(tz).ok_or_else(|| anyhow!("pricing_rules_invalid"))?,
            ),
            None => at,
        };
        if self
            .weekday_mask
            .is_some_and(|mask| mask & (1 << local.weekday().number_days_from_monday()) == 0)
        {
            return Ok(false);
        }
        if let (Some(a), Some(b)) = (&self.local_time_start, &self.local_time_end) {
            let (a, b) = (local_time(a)?, local_time(b)?);
            return Ok(if a < b {
                local.time() >= a && local.time() < b
            } else {
                local.time() >= a || local.time() < b
            });
        }
        Ok(true)
    }
}

#[cfg(test)]
#[path = "_tests/policy_tests.rs"]
mod tests;
