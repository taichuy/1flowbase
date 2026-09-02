use std::{str::FromStr, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use control_plane::billing::{
    pricing_rules_cache_key, PricingRule, GLOBAL_ZERO_MODEL_ID, GLOBAL_ZERO_PROVIDER_CODE,
};

pub(crate) async fn invalidate_pricing_rules_cache(
    cache_store: &dyn control_plane::ports::CacheStore,
    rule: &PricingRule,
) -> Result<(), ApiError> {
    if rule.provider_code == GLOBAL_ZERO_PROVIDER_CODE
        && rule.upstream_model_id == GLOBAL_ZERO_MODEL_ID
    {
        cache_store.clear_cache_domain("model-pricing").await?;
        return Ok(());
    }
    cache_store
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
use time::{OffsetDateTime, Time};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
};

#[derive(Debug, Deserialize)]
pub struct PricingRuleQuery {
    pub(crate) provider_code: Option<String>,
    pub(crate) upstream_model_id: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) source_kind: Option<String>,
    pub(crate) page: Option<i64>,
    pub(crate) page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PricingRulesPageResponse {
    pub(crate) items: Vec<PricingRuleResponse>,
    pub(crate) total_count: i64,
    pub(crate) page: i64,
    pub(crate) page_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct PricingCatalogQuery {
    pub(crate) provider_code: Option<String>,
    pub(crate) upstream_model_id: Option<String>,
    pub(crate) page: Option<usize>,
    pub(crate) page_size: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct PricingCatalogPageResponse {
    pub(crate) schema_version: &'static str,
    pub(crate) catalog_version: String,
    pub(crate) currency_code: &'static str,
    pub(crate) items: Vec<PricingRuleBody>,
    pub(crate) total_count: usize,
    pub(crate) page: usize,
    pub(crate) page_size: usize,
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

pub(crate) fn body_to_rule(
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
    let super::billing_interface::BillingOutput::PricingRules(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.pricing-rules.list.v1",
        super::billing_interface::BillingInput::ListPricingRules(q),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
pub async fn create_pricing_rule(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PricingRuleBody>,
) -> Result<Json<ApiSuccess<PricingRuleResponse>>, ApiError> {
    let super::billing_interface::BillingOutput::PricingRule(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.pricing-rules.create.v1",
        super::billing_interface::BillingInput::CreatePricingRule(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
pub async fn update_pricing_rule(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PricingRuleBody>,
) -> Result<Json<ApiSuccess<PricingRuleResponse>>, ApiError> {
    let super::billing_interface::BillingOutput::PricingRule(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.pricing-rules.update.v1",
        super::billing_interface::BillingInput::UpdatePricingRule { id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
pub async fn delete_pricing_rule(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let super::billing_interface::BillingOutput::Deleted(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.pricing-rules.delete.v1",
        super::billing_interface::BillingInput::DeletePricingRule { id },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

pub async fn get_pricing_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<PricingCatalogQuery>,
) -> Result<Json<ApiSuccess<PricingCatalogPageResponse>>, ApiError> {
    let super::billing_interface::BillingOutput::PricingCatalog(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.pricing-catalog.get.v1",
        super::billing_interface::BillingInput::GetPricingCatalog(query),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
#[derive(Debug, Deserialize)]
pub struct ImportCatalogBody {
    pub catalog_ids: Vec<Uuid>,
}
pub async fn import_pricing_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ImportCatalogBody>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let super::billing_interface::BillingOutput::Imported(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.pricing-catalog.import.v1",
        super::billing_interface::BillingInput::ImportPricingCatalog(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub(crate) limit: Option<i64>,
    pub(crate) offset: Option<i64>,
    pub(crate) user_id: Option<Uuid>,
    pub(crate) before_created_at: Option<OffsetDateTime>,
    pub(crate) before_id: Option<Uuid>,
}
pub async fn list_credit_accounts(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiSuccess<Vec<control_plane::ports::CreditAccountRecord>>>, ApiError> {
    let super::billing_interface::BillingOutput::CreditAccounts(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.credit-accounts.list.v1",
        super::billing_interface::BillingInput::ListCreditAccounts(q),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
pub async fn get_credit_account(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Option<control_plane::ports::CreditAccountRecord>>>, ApiError> {
    let super::billing_interface::BillingOutput::CreditAccount(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.credit-accounts.get.v1",
        super::billing_interface::BillingInput::GetCreditAccount { user_id },
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
pub async fn list_credit_ledger(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiSuccess<Vec<control_plane::ports::CreditTransactionRecord>>>, ApiError> {
    let super::billing_interface::BillingOutput::CreditLedger(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.credit-ledger.list.v1",
        super::billing_interface::BillingInput::ListCreditLedger(q),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[derive(Debug, Deserialize)]
pub struct CreditCommandBody {
    pub(crate) amount: Option<String>,
    pub(crate) reason: String,
    pub(crate) source_type: Option<String>,
    pub(crate) source_id: Option<String>,
    pub(crate) idempotency_key: String,
    pub(crate) metadata: Option<Value>,
}
macro_rules! credit_handler {
    ($name:ident,$input:ident,$binding_id:literal) => {
        pub async fn $name(
            State(state): State<Arc<ApiState>>,
            headers: HeaderMap,
            Path(user_id): Path<Uuid>,
            Json(body): Json<CreditCommandBody>,
        ) -> Result<Json<ApiSuccess<control_plane::ports::CreditTransactionRecord>>, ApiError> {
            let super::billing_interface::BillingOutput::CreditTransaction(response) = invoke(
                state,
                headers,
                $binding_id,
                super::billing_interface::BillingInput::$input { user_id, body },
                true,
            )
            .await?
            else {
                unreachable!()
            };
            Ok(Json(ApiSuccess::new(response)))
        }
    };
}
credit_handler!(
    grant_credit,
    GrantCredit,
    "http.console.settings.billing.credits.grant.v1"
);
credit_handler!(
    charge_credit,
    ChargeCredit,
    "http.console.settings.billing.credits.charge.v1"
);
credit_handler!(
    adjust_credit,
    AdjustCredit,
    "http.console.settings.billing.credits.adjust.v1"
);
credit_handler!(
    enable_charge,
    EnableCharge,
    "http.console.settings.billing.credits.enable.v1"
);
credit_handler!(
    disable_charge,
    DisableCharge,
    "http.console.settings.billing.credits.disable.v1"
);
credit_handler!(
    refund_credit,
    RefundCredit,
    "http.console.settings.billing.credits.refund.v1"
);

async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: super::billing_interface::BillingInput,
    mutating: bool,
) -> Result<super::billing_interface::BillingOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
}
