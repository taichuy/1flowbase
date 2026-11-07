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
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
};

/// Existing `time::OffsetDateTime` serde wire tuple, without an RFC3339 adapter.
/// Slots: year, ordinal day, hour, minute, second, nanosecond, offset hours,
/// offset minutes past the hour, and offset seconds past the minute.
pub struct BillingOffsetDateTimeWire;
impl utoipa::PartialSchema for BillingOffsetDateTimeWire {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        use utoipa::openapi::schema::{ArrayBuilder, ObjectBuilder, Type};
        let slots = [
            "Year",
            "Ordinal day",
            "Hour",
            "Minute",
            "Second",
            "Nanosecond",
            "Offset hours",
            "Offset minutes past the hour",
            "Offset seconds past the minute",
        ];
        ArrayBuilder::new()
            .items(ObjectBuilder::new().schema_type(Type::Integer))
            .prefix_items(slots.map(|slot| {
                ObjectBuilder::new()
                    .schema_type(Type::Integer)
                    .description(Some(slot))
                    .build()
            }))
            .min_items(Some(9))
            .max_items(Some(9))
            .build()
            .into()
    }
}
impl ToSchema for BillingOffsetDateTimeWire {}

// Schema-only DTOs describe the exact existing external record/summary wire
// fields. HTTP handlers keep serializing their original owner types unchanged.
/// Wire fields of `control_plane::ports::CreditAccountRecord`.
#[derive(ToSchema)]
pub struct BillingCreditAccountResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub credit_unit: String,
    pub charge_enabled: bool,
    pub current_balance: String,
    pub reserved_amount: String,
    pub available_balance: String,
    pub credit_insufficient: bool,
    pub revision: i64,
    pub created_at: BillingOffsetDateTimeWire,
    pub updated_at: BillingOffsetDateTimeWire,
}

/// Wire fields of `control_plane::ports::CreditTransactionRecord`.
#[derive(ToSchema)]
pub struct BillingCreditTransactionResponse {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub billing_session_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub actor_plugin_id: Option<String>,
    pub transaction_type: String,
    pub amount: String,
    pub balance_after: String,
    pub reserved_after: String,
    pub credit_unit: String,
    pub reason: String,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub idempotency_key: String,
    pub status: String,
    pub metadata: Value,
    pub created_at: BillingOffsetDateTimeWire,
}

/// Existing delete acknowledgement emitted by the billing interface owner.
#[derive(ToSchema)]
pub struct BillingPricingRuleDeleteResponse {
    pub deleted: bool,
}

/// Wire fields of `model_pricing_catalog::PricingRuleInstallSummary`.
#[derive(ToSchema)]
pub struct BillingPricingCatalogImportResponse {
    pub inserted: usize,
    pub skipped: usize,
    pub updated: usize,
    pub deleted: usize,
}

/// Wire fields of `control_plane::ports::PricingCatalogSyncSummary`.
#[derive(ToSchema)]
pub struct BillingPricingCatalogSyncResponse {
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub retired: usize,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct PricingRuleQuery {
    pub(crate) provider_code: Option<String>,
    pub(crate) upstream_model_id: Option<String>,
    pub(crate) enabled: Option<bool>,
    pub(crate) source_kind: Option<String>,
    pub(crate) page: Option<i64>,
    pub(crate) page_size: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PricingRulesPageResponse {
    pub(crate) items: Vec<PricingRuleResponse>,
    pub(crate) total_count: i64,
    pub(crate) page: i64,
    pub(crate) page_size: i64,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct PricingCatalogQuery {
    /// Return counts without downloading pricing rule pages.
    pub(crate) metadata_only: Option<bool>,
    pub(crate) provider_code: Option<String>,
    pub(crate) upstream_model_id: Option<String>,
    pub(crate) page: Option<usize>,
    pub(crate) page_size: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PricingCatalogPageResponse {
    pub(crate) schema_version: &'static str,
    pub(crate) catalog_version: String,
    pub(crate) currency_code: &'static str,
    pub(crate) items: Vec<PricingRuleBody>,
    pub(crate) total_count: usize,
    pub(crate) page: usize,
    pub(crate) page_size: usize,
}

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

#[derive(Debug, Serialize, ToSchema)]
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
    pub cache_write_token_unit_size: i64,
    pub cache_write_token_unit_price: String,
    pub currency_code: String,
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
    pub source_kind: String,
    pub source_catalog_id: Option<String>,
    pub source_version: Option<String>,
    pub source_checksum: Option<String>,
    pub extensions: Value,
    pub created_by: Option<Uuid>,
    #[schema(value_type = BillingOffsetDateTimeWire)]
    pub created_at: OffsetDateTime,
    #[schema(value_type = BillingOffsetDateTimeWire)]
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
            cache_write_token_unit_size: rule.cache_write_token_unit_size,
            cache_write_token_unit_price: rule.cache_write_token_unit_price.to_string(),
            currency_code: rule.currency_code,
            effective_from: rule.effective_from,
            effective_to: rule.effective_to,
            timezone: rule.timezone,
            weekday_mask: rule.weekday_mask,
            local_time_start: rule.local_time_start.map(|value| value.to_string()),
            local_time_end: rule.local_time_end.map(|value| value.to_string()),
            priority: rule.priority,
            enabled: rule.enabled,
            rules: rule.rules,
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
            "/settings/billing/pricing-catalog/sync",
            console_post(
                sync_pricing_catalog,
                ConsoleOperation("billing.pricing_catalog.sync".into()),
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

#[utoipa::path(
    get,
    path = "/api/console/settings/billing/pricing-rules",
    operation_id = "billing.pricing_rules.list",
    summary = "List billing pricing rules",
    description = "List billing pricing rules in the system backend.",
    params(PricingRuleQuery),
    responses((status = 200, body = PricingRulesPageResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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
#[utoipa::path(
    post,
    path = "/api/console/settings/billing/pricing-rules",
    operation_id = "billing.pricing_rules.create",
    summary = "Create billing pricing rules",
    description = "Create billing pricing rules in the system backend.",
    request_body = PricingRuleBody,
    responses((status = 200, body = PricingRuleResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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
#[utoipa::path(
    patch,
    path = "/api/console/settings/billing/pricing-rules/{id}",
    operation_id = "billing.pricing_rules.update",
    summary = "Update billing pricing rules",
    description = "Update billing pricing rules in the system backend.",
    params(("id" = Uuid, Path, description = "Pricing rule identifier")),
    request_body = PricingRuleBody,
    responses((status = 200, body = PricingRuleResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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
#[utoipa::path(
    delete,
    path = "/api/console/settings/billing/pricing-rules/{id}",
    operation_id = "billing.pricing_rules.delete",
    summary = "Delete billing pricing rules",
    description = "Delete billing pricing rules in the system backend.",
    params(("id" = Uuid, Path, description = "Pricing rule identifier")),
    responses((status = 200, body = BillingPricingRuleDeleteResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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

#[utoipa::path(
    get,
    path = "/api/console/settings/billing/pricing-catalog",
    operation_id = "billing.pricing_catalog.view",
    summary = "View billing pricing catalog",
    description = "View billing pricing catalog in the system backend.",
    params(PricingCatalogQuery),
    responses((status = 200, body = PricingCatalogPageResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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
#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportCatalogBody {
    pub catalog_ids: Vec<Uuid>,
}
#[utoipa::path(
    post,
    path = "/api/console/settings/billing/pricing-catalog/import",
    operation_id = "billing.pricing_catalog.import",
    summary = "Import billing pricing catalog",
    description = "Import billing pricing catalog in the system backend.",
    request_body = ImportCatalogBody,
    responses((status = 200, body = BillingPricingCatalogImportResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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

#[utoipa::path(
    post,
    path = "/api/console/settings/billing/pricing-catalog/sync",
    operation_id = "billing.pricing_catalog.sync",
    summary = "Synchronize all official model prices",
    description = "Fetch the complete official catalog and atomically update official prices and conditional rules, insert new models, and retire duplicate official records while preserving manual rules and historical IDs.",
    responses((status = 200, body = BillingPricingCatalogSyncResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
pub async fn sync_pricing_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let super::billing_interface::BillingOutput::Imported(response) = invoke(
        state,
        headers,
        "http.console.settings.billing.pricing-catalog.sync.v1",
        super::billing_interface::BillingInput::SyncPricingCatalog,
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct PageQuery {
    pub(crate) limit: Option<i64>,
    pub(crate) offset: Option<i64>,
    pub(crate) user_id: Option<Uuid>,
    #[param(value_type = Option<BillingOffsetDateTimeWire>)]
    #[schema(value_type = Option<BillingOffsetDateTimeWire>)]
    pub(crate) before_created_at: Option<OffsetDateTime>,
    pub(crate) before_id: Option<Uuid>,
}
#[utoipa::path(
    get,
    path = "/api/console/settings/billing/credit-accounts",
    operation_id = "billing.credit_accounts.list",
    summary = "List billing credit accounts",
    description = "List billing credit accounts in the system backend.",
    params(PageQuery),
    responses((status = 200, body = Vec<BillingCreditAccountResponse>), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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
#[utoipa::path(
    get,
    path = "/api/console/settings/billing/credit-accounts/{user_id}",
    operation_id = "billing.credit_accounts.view",
    summary = "View billing credit accounts",
    description = "View billing credit accounts in the system backend.",
    params(("user_id" = Uuid, Path, description = "User whose credit account is returned")),
    responses((status = 200, body = Option<BillingCreditAccountResponse>), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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
#[utoipa::path(
    get,
    path = "/api/console/settings/billing/credit-ledger",
    operation_id = "billing.credit_ledger.list",
    summary = "List billing credit ledger",
    description = "List billing credit ledger in the system backend.",
    params(PageQuery),
    responses((status = 200, body = Vec<BillingCreditTransactionResponse>), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
)]
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreditCommandBody {
    pub(crate) amount: Option<String>,
    pub(crate) reason: String,
    pub(crate) source_type: Option<String>,
    pub(crate) source_id: Option<String>,
    pub(crate) idempotency_key: String,
    pub(crate) metadata: Option<Value>,
}
macro_rules! credit_handler {
    ($name:ident,$input:ident,$binding_id:literal,$operation_id:literal,$path:literal,$summary:literal,$description:literal) => {
        #[utoipa::path(
            post,
            path = $path,
            operation_id = $operation_id,
            summary = $summary,
            description = $description,
            params(("user_id" = Uuid, Path, description = "User whose credit account receives the command")),
            request_body = CreditCommandBody,
            responses((status = 200, body = BillingCreditTransactionResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 500, body = crate::error_response::ErrorBody))
        )]
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
    "http.console.settings.billing.credits.grant.v1",
    "billing.credit.grant",
    "/api/console/settings/billing/credits/{user_id}/grant",
    "Billing credit grant",
    "Billing credit grant in the system backend."
);
credit_handler!(
    charge_credit,
    ChargeCredit,
    "http.console.settings.billing.credits.charge.v1",
    "billing.credit.charge",
    "/api/console/settings/billing/credits/{user_id}/charge",
    "Billing credit charge",
    "Billing credit charge in the system backend."
);
credit_handler!(
    adjust_credit,
    AdjustCredit,
    "http.console.settings.billing.credits.adjust.v1",
    "billing.credit.adjust",
    "/api/console/settings/billing/credits/{user_id}/adjust",
    "Billing credit adjust",
    "Billing credit adjust in the system backend."
);
credit_handler!(
    enable_charge,
    EnableCharge,
    "http.console.settings.billing.credits.enable.v1",
    "billing.credit.enable",
    "/api/console/settings/billing/credits/{user_id}/enable",
    "Enable billing credit",
    "Enable billing credit in the system backend."
);
credit_handler!(
    disable_charge,
    DisableCharge,
    "http.console.settings.billing.credits.disable.v1",
    "billing.credit.disable",
    "/api/console/settings/billing/credits/{user_id}/disable",
    "Disable billing credit",
    "Disable billing credit in the system backend."
);
credit_handler!(
    refund_credit,
    RefundCredit,
    "http.console.settings.billing.credits.refund.v1",
    "billing.credit.refund",
    "/api/console/settings/billing/credits/{user_id}/refund",
    "Billing credit refund",
    "Billing credit refund in the system backend."
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

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "billing/_tests/openapi_contract.rs"]
mod openapi_contract_tests;
