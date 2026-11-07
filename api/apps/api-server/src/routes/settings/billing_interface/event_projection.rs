use super::*;
use crate::event_center::api_projection::{
    to_value, ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
use crate::routes::document_event::query_fields as q;
use serde_json::{json, Value};
use std::sync::Arc;
impl ApiEventInput for BillingInput {
    fn project_document_input(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        Ok(match self {
            Self::ListPricingRules(v) => {
                json!({"query":q(json!({"provider_code":v.provider_code,"upstream_model_id":v.upstream_model_id,"enabled":v.enabled,"source_kind":v.source_kind,"page":v.page,"page_size":v.page_size}))})
            }
            Self::GetPricingCatalog(v) => {
                json!({"query":q(json!({"metadata_only":v.metadata_only,"provider_code":v.provider_code,"upstream_model_id":v.upstream_model_id,"page":v.page,"page_size":v.page_size}))})
            }
            Self::CreatePricingRule(v) => json!({"body":to_value(v)?}),
            Self::UpdatePricingRule { id, body } => {
                json!({"path":{"id":id},"body":to_value(body)?})
            }
            Self::DeletePricingRule { id } => json!({"path":{"id":id}}),
            Self::ImportPricingCatalog(v) => json!({"body":{"catalog_ids":v.catalog_ids}}),
            Self::SyncPricingCatalog => json!({}),
            Self::ListCreditAccounts(v) | Self::ListCreditLedger(v) => {
                let before_created_at = v
                    .before_created_at
                    .map(|value| value.format(&time::format_description::well_known::Rfc3339))
                    .transpose()
                    .map_err(|_| Arc::<str>::from("billing-query-date-invalid"))?;
                json!({"query":q(json!({"limit":v.limit,"offset":v.offset,"user_id":v.user_id,"before_created_at":before_created_at,"before_id":v.before_id}))})
            }
            Self::GetCreditAccount { user_id } => json!({"path":{"user_id":user_id}}),
            Self::GrantCredit { user_id, body }
            | Self::ChargeCredit { user_id, body }
            | Self::AdjustCredit { user_id, body }
            | Self::EnableCharge { user_id, body }
            | Self::DisableCharge { user_id, body }
            | Self::RefundCredit { user_id, body } => {
                json!({"path":{"user_id":user_id},"body":{"amount":body.amount,"reason":body.reason,"source_type":body.source_type,"source_id":body.source_id,"idempotency_key":body.idempotency_key,"metadata":body.metadata}})
            }
        })
    }
}
impl ApiEventOutput for BillingOutput {
    fn project_document_output(&self, _: &ApiEventOperationContext) -> Result<Value, Arc<str>> {
        match self {
            Self::PricingRules(v) => to_value(v),
            Self::PricingRule(v) => to_value(v),
            Self::Deleted(v) | Self::Imported(v) => Ok(v.clone()),
            Self::PricingCatalog(v) => to_value(v),
            Self::CreditAccounts(v) => to_value(v),
            Self::CreditAccount(v) => to_value(v),
            Self::CreditLedger(v) => to_value(v),
            Self::CreditTransaction(v) => to_value(v),
        }
    }
}
