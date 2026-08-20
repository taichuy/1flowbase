use std::{str::FromStr, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use control_plane::{
    billing::{
        pricing_rules_cache_key, PricingRule, GLOBAL_ZERO_MODEL_ID, GLOBAL_ZERO_PROVIDER_CODE,
    },
    ports::{
        BillingRepository, CreditCommandInput, ListCreditLedgerInput, ListPricingRulesInput,
        UpsertPricingRuleInput,
    },
};

async fn invalidate_pricing_rules_cache(
    state: &ApiState,
    rule: &PricingRule,
) -> Result<(), ApiError> {
    if rule.provider_code == GLOBAL_ZERO_PROVIDER_CODE
        && rule.upstream_model_id == GLOBAL_ZERO_MODEL_ID
    {
        state
            .infrastructure
            .cache_store()
            .clear_cache_domain("model-pricing")
            .await?;
        return Ok(());
    }
    state
        .infrastructure
        .cache_store()
        .delete(&pricing_rules_cache_key(
            &rule.provider_code,
            &rule.upstream_model_id,
        ))
        .await?;
    Ok(())
}
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, Time};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
};

#[derive(Debug, Deserialize)]
pub struct PricingRuleQuery {
    provider_code: Option<String>,
    upstream_model_id: Option<String>,
    enabled: Option<bool>,
    source_kind: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PricingRulesPageResponse {
    items: Vec<PricingRuleResponse>,
    total_count: i64,
    page: i64,
    page_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct PricingCatalogQuery {
    provider_code: Option<String>,
    upstream_model_id: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct PricingCatalogPageResponse {
    schema_version: &'static str,
    catalog_version: String,
    currency_code: &'static str,
    items: Vec<PricingRuleBody>,
    total_count: usize,
    page: usize,
    page_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    pub currency_code: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub effective_from: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub effective_to: Option<OffsetDateTime>,
    pub timezone: String,
    pub weekday_mask: i16,
    pub local_time_start: Option<String>,
    pub local_time_end: Option<String>,
    pub priority: i32,
    pub enabled: bool,
    #[serde(default)]
    pub rating_policy_enabled: bool,
    pub rating_policy: Option<Value>,
    pub source_kind: Option<String>,
    pub source_catalog_id: Option<String>,
    pub source_version: Option<String>,
    pub source_checksum: Option<String>,
    pub extensions: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct PricingRuleResponse {
    pub id: Uuid,
    pub provider_code: String,
    pub upstream_model_id: String,
    pub input_token_unit_size: i64,
    pub input_token_unit_price: String,
    pub output_token_unit_size: i64,
    pub output_token_unit_price: String,
    pub cache_hit_token_unit_size: i64,
    pub cache_hit_token_unit_price: String,
    pub currency_code: String,
    #[serde(with = "time::serde::rfc3339")]
    pub effective_from: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub effective_to: Option<OffsetDateTime>,
    pub timezone: String,
    pub weekday_mask: i16,
    pub local_time_start: Option<String>,
    pub local_time_end: Option<String>,
    pub priority: i32,
    pub enabled: bool,
    pub rating_policy_enabled: bool,
    pub rating_policy: Value,
    pub source_kind: String,
    pub source_catalog_id: Option<String>,
    pub source_version: Option<String>,
    pub source_checksum: Option<String>,
    pub extensions: Value,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<PricingRule> for PricingRuleResponse {
    fn from(rule: PricingRule) -> Self {
        Self {
            id: rule.id,
            provider_code: rule.provider_code,
            upstream_model_id: rule.upstream_model_id,
            input_token_unit_size: rule.input_token_unit_size,
            input_token_unit_price: rule.input_token_unit_price.to_string(),
            output_token_unit_size: rule.output_token_unit_size,
            output_token_unit_price: rule.output_token_unit_price.to_string(),
            cache_hit_token_unit_size: rule.cache_hit_token_unit_size,
            cache_hit_token_unit_price: rule.cache_hit_token_unit_price.to_string(),
            currency_code: rule.currency_code,
            effective_from: rule.effective_from,
            effective_to: rule.effective_to,
            timezone: rule.timezone,
            weekday_mask: rule.weekday_mask,
            local_time_start: rule.local_time_start.map(|value| value.to_string()),
            local_time_end: rule.local_time_end.map(|value| value.to_string()),
            priority: rule.priority,
            enabled: rule.enabled,
            rating_policy_enabled: rule.rating_policy_enabled,
            rating_policy: rule.rating_policy,
            source_kind: rule.source_kind,
            source_catalog_id: rule.source_catalog_id,
            source_version: rule.source_version,
            source_checksum: rule.source_checksum,
            extensions: rule.extensions,
            created_by: rule.created_by,
            created_at: rule.created_at,
            updated_at: rule.updated_at,
        }
    }
}

fn body_to_rule(
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
        rating_policy_enabled: body.rating_policy_enabled,
        rating_policy: body.rating_policy.unwrap_or_else(|| serde_json::json!({})),
        source_kind: body.source_kind.unwrap_or_else(|| "manual".into()),
        source_catalog_id: body.source_catalog_id,
        source_version: body.source_version,
        source_checksum: body.source_checksum,
        extensions: body.extensions.unwrap_or_else(|| serde_json::json!({})),
        created_by: Some(actor_user_id),
        created_at: now,
        updated_at: now,
    };
    rule.validate()?;
    Ok(rule)
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;
    ConsoleRouteAssembly::new()
        .route(
            "/settings/billing/pricing-rules",
            console_get(
                list_pricing_rules,
                ConsoleOperation("billing.pricing_rules.list".into()),
            )
            .post(
                create_pricing_rule,
                ConsoleOperation("billing.pricing_rules.create".into()),
            ),
        )
        .route(
            "/settings/billing/pricing-rules/:id",
            console_patch(
                update_pricing_rule,
                ConsoleOperation("billing.pricing_rules.update".into()),
            )
            .delete(
                delete_pricing_rule,
                ConsoleOperation("billing.pricing_rules.delete".into()),
            ),
        )
        .route(
            "/settings/billing/pricing-catalog",
            console_get(
                get_pricing_catalog,
                ConsoleOperation("billing.pricing_catalog.view".into()),
            ),
        )
        .route(
            "/settings/billing/pricing-catalog/import",
            console_post(
                import_pricing_catalog,
                ConsoleOperation("billing.pricing_catalog.import".into()),
            ),
        )
        .route(
            "/settings/billing/credit-accounts",
            console_get(
                list_credit_accounts,
                ConsoleOperation("billing.credit_accounts.list".into()),
            ),
        )
        .route(
            "/settings/billing/credit-accounts/:user_id",
            console_get(
                get_credit_account,
                ConsoleOperation("billing.credit_accounts.view".into()),
            ),
        )
        .route(
            "/settings/billing/credit-ledger",
            console_get(
                list_credit_ledger,
                ConsoleOperation("billing.credit_ledger.list".into()),
            ),
        )
        .route(
            "/settings/billing/credits/:user_id/grant",
            console_post(
                grant_credit,
                ConsoleOperation("billing.credit.grant".into()),
            ),
        )
        .route(
            "/settings/billing/credits/:user_id/charge",
            console_post(
                charge_credit,
                ConsoleOperation("billing.credit.charge".into()),
            ),
        )
        .route(
            "/settings/billing/credits/:user_id/adjust",
            console_post(
                adjust_credit,
                ConsoleOperation("billing.credit.adjust".into()),
            ),
        )
        .route(
            "/settings/billing/credits/:user_id/enable",
            console_post(
                enable_charge,
                ConsoleOperation("billing.credit.enable".into()),
            ),
        )
        .route(
            "/settings/billing/credits/:user_id/disable",
            console_post(
                disable_charge,
                ConsoleOperation("billing.credit.disable".into()),
            ),
        )
        .route(
            "/settings/billing/credits/:user_id/refund",
            console_post(
                refund_credit,
                ConsoleOperation("billing.credit.refund".into()),
            ),
        )
}

pub async fn list_pricing_rules(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<PricingRuleQuery>,
) -> Result<Json<ApiSuccess<PricingRulesPageResponse>>, ApiError> {
    require_session(&state, &headers).await?;
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(20).clamp(1, 100);
    let result = state
        .store
        .list_pricing_rules(&ListPricingRulesInput {
            provider_code: q.provider_code,
            upstream_model_id: q.upstream_model_id,
            enabled: q.enabled,
            source_kind: q.source_kind,
            page_size,
            offset: (page - 1) * page_size,
        })
        .await?;
    Ok(Json(ApiSuccess::new(PricingRulesPageResponse {
        items: result.items.into_iter().map(Into::into).collect(),
        total_count: result.total_count,
        page,
        page_size,
    })))
}
pub async fn create_pricing_rule(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PricingRuleBody>,
) -> Result<Json<ApiSuccess<PricingRuleResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let row = state
        .store
        .upsert_pricing_rule(&UpsertPricingRuleInput {
            rule: body_to_rule(body, context.user.id, None)?,
        })
        .await?;
    invalidate_pricing_rules_cache(&state, &row).await?;
    Ok(Json(ApiSuccess::new(row.into())))
}
pub async fn update_pricing_rule(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PricingRuleBody>,
) -> Result<Json<ApiSuccess<PricingRuleResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let Some(previous) = state.store.get_pricing_rule(id).await? else {
        return Err(control_plane::errors::ControlPlaneError::NotFound("pricing_rule").into());
    };
    let row = state
        .store
        .upsert_pricing_rule(&UpsertPricingRuleInput {
            rule: body_to_rule(body, context.user.id, Some(id))?,
        })
        .await?;
    invalidate_pricing_rules_cache(&state, &previous).await?;
    invalidate_pricing_rules_cache(&state, &row).await?;
    Ok(Json(ApiSuccess::new(row.into())))
}
pub async fn delete_pricing_rule(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let previous = state.store.get_pricing_rule(id).await?;
    let deleted = state.store.delete_pricing_rule(id).await?;
    if let Some(previous) = previous {
        invalidate_pricing_rules_cache(&state, &previous).await?;
    }
    Ok(Json(ApiSuccess::new(
        serde_json::json!({"deleted":deleted}),
    )))
}

pub async fn get_pricing_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<PricingCatalogQuery>,
) -> Result<Json<ApiSuccess<PricingCatalogPageResponse>>, ApiError> {
    require_session(&state, &headers).await?;
    let (_, catalog) = bundled_pricing_catalog(&state)?;
    let provider_filter = query
        .provider_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let model_filter = query
        .upstream_model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let filtered = catalog
        .rules
        .into_iter()
        .filter(|rule| {
            provider_filter.is_none_or(|filter| {
                rule.provider_code
                    .to_lowercase()
                    .contains(&filter.to_lowercase())
            }) && model_filter.is_none_or(|filter| {
                rule.upstream_model_id
                    .to_lowercase()
                    .contains(&filter.to_lowercase())
            })
        })
        .collect::<Vec<_>>();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let total_count = filtered.len();
    let items = filtered
        .into_iter()
        .skip((page - 1) * page_size)
        .take(page_size)
        .collect();
    Ok(Json(ApiSuccess::new(PricingCatalogPageResponse {
        schema_version: "1flowbase.model-pricing-page/v1",
        catalog_version: catalog.catalog_version,
        currency_code: "USD",
        items,
        total_count,
        page,
        page_size,
    })))
}
#[derive(Debug, Deserialize)]
pub struct ImportCatalogBody {
    pub catalog_ids: Vec<Uuid>,
}
#[derive(Debug, Deserialize)]
struct BundledPricingCatalog {
    catalog_version: String,
    rules: Vec<PricingRuleBody>,
}

#[derive(Debug, Deserialize)]
struct RemotePricingCatalogIndex {
    schema_version: String,
    catalog_version: String,
    currency_code: String,
    total_rules: usize,
    pages: Vec<RemotePricingCatalogPageReference>,
}

#[derive(Debug, Deserialize)]
struct RemotePricingCatalogPageReference {
    page: usize,
    rule_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Deserialize)]
struct RemotePricingCatalogPage {
    schema_version: String,
    catalog_version: String,
    currency_code: String,
    page: usize,
    rules: Vec<PricingRuleBody>,
}

pub(crate) fn catalog_version_is_at_least(candidate: &str, baseline: &str) -> bool {
    fn version_key(value: &str) -> Option<(&str, u64)> {
        let (date, revision) = value.rsplit_once('.')?;
        Some((date, revision.parse().ok()?))
    }
    match (version_key(candidate), version_key(baseline)) {
        (Some(candidate), Some(baseline)) => candidate >= baseline,
        _ => false,
    }
}

fn bundled_pricing_catalog(state: &ApiState) -> Result<(Value, BundledPricingCatalog), ApiError> {
    let (value, catalog, rule_bytes, expected_checksum) = bundled_pricing_catalog_payload()?;
    verify_catalog_signature(state, &value, &rule_bytes, &expected_checksum)?;
    Ok((value, catalog))
}

fn bundled_pricing_catalog_payload(
) -> Result<(Value, BundledPricingCatalog, Vec<u8>, String), ApiError> {
    let value: Value = serde_json::from_str(include_str!(
        "../../../assets/model-pricing/catalog.v1.json"
    ))?;
    pricing_catalog_payload(value)
}

fn pricing_catalog_payload(
    value: Value,
) -> Result<(Value, BundledPricingCatalog, Vec<u8>, String), ApiError> {
    let rules =
        value
            .get("rules")
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "pricing_catalog_rules",
            ))?;
    let rule_bytes = serde_json::to_vec(rules)?;
    let expected_checksum = value
        .get("rules_checksum")
        .and_then(Value::as_str)
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
            "pricing_catalog_checksum",
        ))?
        .to_string();
    let actual_checksum = format!("sha256:{:x}", Sha256::digest(&rule_bytes));
    if actual_checksum != expected_checksum {
        return Err(control_plane::errors::ControlPlaneError::Conflict(
            "pricing_catalog_checksum_mismatch",
        )
        .into());
    }
    let catalog = serde_json::from_value(value.clone())?;
    Ok((value, catalog, rule_bytes, expected_checksum))
}

fn verify_catalog_signature(
    state: &ApiState,
    value: &Value,
    rule_bytes: &[u8],
    expected_checksum: &str,
) -> Result<(), ApiError> {
    verify_catalog_signature_with_keys(
        value,
        rule_bytes,
        expected_checksum,
        &state.official_plugin_source.trusted_public_keys(),
    )
}

fn verify_catalog_signature_with_keys(
    value: &Value,
    rule_bytes: &[u8],
    expected_checksum: &str,
    trusted_public_keys: &[plugin_framework::TrustedPublicKey],
) -> Result<(), ApiError> {
    if let Some(signature) = value.get("signature").filter(|value| !value.is_null()) {
        let algorithm = signature
            .get("algorithm")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let key_id = signature
            .get("key_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let signature_base64 = signature
            .get("signature")
            .and_then(Value::as_str)
            .unwrap_or_default();
        plugin_framework::verify_trusted_ed25519_artifact(
            rule_bytes,
            expected_checksum,
            algorithm,
            key_id,
            signature_base64,
            trusted_public_keys,
        )?;
    }
    Ok(())
}

pub(crate) async fn sync_bundled_pricing_catalog<R: BillingRepository>(
    repository: &R,
    actor_user_id: Uuid,
) -> Result<usize, ApiError> {
    let (_, catalog, _, _) = bundled_pricing_catalog_payload()?;
    let mut synced = 0;
    for body in catalog.rules {
        let rule = body_to_rule(body, actor_user_id, None)?;
        if rule.source_kind != "official" || rule.source_catalog_id.is_none() {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_catalog_rule",
            )
            .into());
        }
        repository
            .upsert_pricing_rule(&UpsertPricingRuleInput { rule })
            .await?;
        synced += 1;
    }
    Ok(synced)
}

pub(crate) async fn sync_remote_pricing_catalog<R: BillingRepository>(
    repository: &R,
    actor_user_id: Uuid,
    catalog_index_url: &str,
) -> Result<usize, ApiError> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let index_bytes =
        fetch_pricing_catalog_document(&client, catalog_index_url, 512 * 1024).await?;
    let index: RemotePricingCatalogIndex = serde_json::from_slice(&index_bytes)?;
    if index.schema_version != "1flowbase.model-pricing-index/v1"
        || index.currency_code != "USD"
        || index.pages.len() > 1_000
        || index.total_rules > 100_000
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "pricing_catalog_index",
        )
        .into());
    }
    let bundled_version = bundled_pricing_catalog_payload()?.1.catalog_version;
    if !catalog_version_is_at_least(&index.catalog_version, &bundled_version) {
        return Ok(0);
    }
    let page_locator_prefix = catalog_index_url.strip_suffix("index.json").ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("pricing_catalog_index_url"),
    )?;
    let mut rules = Vec::with_capacity(index.total_rules);
    for (position, reference) in index.pages.into_iter().enumerate() {
        let expected_page = position + 1;
        let expected_locator = format!("{page_locator_prefix}pages/{expected_page}.json");
        if reference.page != expected_page || reference.locator != expected_locator {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "pricing_catalog_page_locator",
            )
            .into());
        }
        let bytes =
            fetch_pricing_catalog_document(&client, &reference.locator, 2 * 1024 * 1024).await?;
        let actual_checksum = format!("sha256:{:x}", Sha256::digest(&bytes));
        if actual_checksum != reference.checksum {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "pricing_catalog_page_checksum_mismatch",
            )
            .into());
        }
        let page: RemotePricingCatalogPage = serde_json::from_slice(&bytes)?;
        if page.schema_version != "1flowbase.model-pricing-page/v1"
            || page.catalog_version != index.catalog_version
            || page.currency_code != "USD"
            || page.page != reference.page
            || page.rules.len() != reference.rule_count
        {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "pricing_catalog_page",
            )
            .into());
        }
        rules.extend(page.rules);
    }
    if rules.len() != index.total_rules {
        return Err(control_plane::errors::ControlPlaneError::Conflict(
            "pricing_catalog_rule_count_mismatch",
        )
        .into());
    }
    let mut synced = 0;
    for body in rules {
        let rule = body_to_rule(body, actor_user_id, None)?;
        if rule.source_kind != "official" || rule.source_catalog_id.is_none() {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_catalog_rule",
            )
            .into());
        }
        repository
            .upsert_pricing_rule(&UpsertPricingRuleInput { rule })
            .await?;
        synced += 1;
    }
    Ok(synced)
}

async fn fetch_pricing_catalog_document(
    client: &reqwest::Client,
    url: &str,
    size_limit: usize,
) -> Result<Vec<u8>, ApiError> {
    let response = client.get(url).send().await?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|size| size > size_limit as u64)
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "pricing_catalog_too_large",
        )
        .into());
    }
    let bytes = response.bytes().await?;
    if bytes.len() > size_limit {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "pricing_catalog_too_large",
        )
        .into());
    }
    Ok(bytes.to_vec())
}
pub async fn import_pricing_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ImportCatalogBody>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let (_, catalog) = bundled_pricing_catalog(&state)?;
    let selected = catalog
        .rules
        .into_iter()
        .filter(|rule| rule.id.is_some_and(|id| body.catalog_ids.contains(&id)))
        .collect::<Vec<_>>();
    if selected.len() != body.catalog_ids.len() {
        return Err(
            control_plane::errors::ControlPlaneError::InvalidInput("pricing_catalog_id").into(),
        );
    }
    let mut imported = 0;
    for item in selected {
        let rule = body_to_rule(item, context.user.id, None)?;
        if rule.source_kind != "official" || rule.source_catalog_id.is_none() {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_catalog_rule",
            )
            .into());
        }
        let imported_rule = state
            .store
            .upsert_pricing_rule(&UpsertPricingRuleInput { rule })
            .await?;
        invalidate_pricing_rules_cache(&state, &imported_rule).await?;
        imported += 1;
    }
    Ok(Json(ApiSuccess::new(
        serde_json::json!({"imported":imported,"deleted":0}),
    )))
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    user_id: Option<Uuid>,
    before_created_at: Option<OffsetDateTime>,
    before_id: Option<Uuid>,
}
pub async fn list_credit_accounts(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiSuccess<Vec<control_plane::ports::CreditAccountRecord>>>, ApiError> {
    let c = require_session(&state, &headers).await?;
    Ok(Json(ApiSuccess::new(
        state
            .store
            .list_credit_accounts(
                c.actor.current_workspace_id,
                q.limit.unwrap_or(100),
                q.offset.unwrap_or(0),
            )
            .await?,
    )))
}
pub async fn get_credit_account(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Option<control_plane::ports::CreditAccountRecord>>>, ApiError> {
    let c = require_session(&state, &headers).await?;
    Ok(Json(ApiSuccess::new(
        state
            .store
            .get_credit_account(c.actor.current_workspace_id, user_id)
            .await?,
    )))
}
pub async fn list_credit_ledger(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiSuccess<Vec<control_plane::ports::CreditTransactionRecord>>>, ApiError> {
    let c = require_session(&state, &headers).await?;
    Ok(Json(ApiSuccess::new(
        state
            .store
            .list_credit_ledger(&ListCreditLedgerInput {
                workspace_id: c.actor.current_workspace_id,
                user_id: q.user_id,
                before_created_at: q.before_created_at,
                before_id: q.before_id,
                limit: q.limit.unwrap_or(100),
            })
            .await?,
    )))
}

#[derive(Debug, Deserialize)]
pub struct CreditCommandBody {
    pub amount: Option<String>,
    pub reason: String,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub idempotency_key: String,
    pub metadata: Option<Value>,
}
async fn command(
    state: &ApiState,
    headers: &HeaderMap,
    user_id: Uuid,
    body: CreditCommandBody,
    kind: &str,
) -> Result<control_plane::ports::CreditTransactionRecord, ApiError> {
    let c = require_session(state, headers).await?;
    require_csrf(headers, &c)?;
    Ok(state
        .store
        .execute_credit_command(&CreditCommandInput {
            workspace_id: c.actor.current_workspace_id,
            user_id,
            amount: body.amount.unwrap_or_else(|| "0".into()),
            credit_unit: "USD".into(),
            command: kind.into(),
            reason: body.reason,
            source_type: body.source_type,
            source_id: body.source_id,
            idempotency_key: body.idempotency_key,
            actor_user_id: Some(c.user.id),
            actor_plugin_id: None,
            metadata: body.metadata.unwrap_or_else(|| serde_json::json!({})),
        })
        .await?)
}
macro_rules! credit_handler {
    ($name:ident,$kind:literal) => {
        pub async fn $name(
            State(state): State<Arc<ApiState>>,
            headers: HeaderMap,
            Path(user_id): Path<Uuid>,
            Json(body): Json<CreditCommandBody>,
        ) -> Result<Json<ApiSuccess<control_plane::ports::CreditTransactionRecord>>, ApiError> {
            Ok(Json(ApiSuccess::new(
                command(&state, &headers, user_id, body, $kind).await?,
            )))
        }
    };
}
credit_handler!(grant_credit, "grant");
credit_handler!(charge_credit, "charge");
credit_handler!(adjust_credit, "adjustment");
credit_handler!(enable_charge, "enable_charge");
credit_handler!(disable_charge, "disable_charge");
credit_handler!(refund_credit, "refund");
