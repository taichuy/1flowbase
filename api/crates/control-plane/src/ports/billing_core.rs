use super::*;
use crate::billing::PricingRule;

#[derive(Debug, Clone)]
pub struct ListPricingRulesInput {
    pub provider_code: Option<String>,
    pub upstream_model_id: Option<String>,
    pub include_disabled: bool,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct UpsertPricingRuleInput {
    pub rule: PricingRule,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreditAccountRecord {
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
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreditTransactionRecord {
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
    pub metadata: serde_json::Value,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct CreditCommandInput {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub amount: String,
    pub credit_unit: String,
    pub command: String,
    pub reason: String,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub idempotency_key: String,
    pub actor_user_id: Option<Uuid>,
    pub actor_plugin_id: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCreditCommandRequest {
    pub command: String,
    pub user_id: Uuid,
    pub amount: String,
    #[serde(default = "default_credit_unit")]
    pub credit_unit: String,
    pub reason: String,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub idempotency_key: String,
    pub billing_session_id: Option<Uuid>,
    pub provider_invocation_id: Option<Uuid>,
    pub pricing_rule_id: Option<Uuid>,
    pub flow_run_id: Option<Uuid>,
    pub reservation_expires_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub price_snapshot: serde_json::Value,
    #[serde(default)]
    pub usage_snapshot: serde_json::Value,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn default_credit_unit() -> String {
    "USD".to_string()
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "result_kind", rename_all = "snake_case")]
pub enum PluginCreditCommandResult {
    Transaction {
        transaction: CreditTransactionRecord,
    },
    Reservation {
        reservation: CreditReservation,
    },
    Released {
        transaction: CreditTransactionRecord,
    },
}

#[derive(Debug, Clone)]
pub struct ReserveCreditInput {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub amount: String,
    pub flow_run_id: Option<Uuid>,
    pub provider_invocation_id: Uuid,
    pub pricing_rule_id: Uuid,
    pub charge_enabled_default: bool,
    pub reservation_expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreditReservation {
    pub billing_session_id: Uuid,
    pub account_id: Uuid,
    pub reserved_amount: String,
    pub charge_skipped: bool,
    pub charge_skip_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SettleCreditInput {
    pub billing_session_id: Uuid,
    pub actual_amount: String,
    pub cost_ledger_id: Option<Uuid>,
    pub usage_ledger_id: Option<Uuid>,
    pub price_snapshot: serde_json::Value,
    pub usage_snapshot: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ListCreditLedgerInput {
    pub workspace_id: Uuid,
    pub user_id: Option<Uuid>,
    pub before_created_at: Option<OffsetDateTime>,
    pub before_id: Option<Uuid>,
    pub limit: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreditOutboxEvent {
    pub event_id: Uuid,
    pub workspace_id: Uuid,
    pub account_id: Option<Uuid>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
    pub delivery_attempts: i32,
}

#[async_trait]
pub trait BillingRepository: Send + Sync {
    async fn list_pricing_rules(
        &self,
        input: &ListPricingRulesInput,
    ) -> anyhow::Result<Vec<PricingRule>>;
    async fn get_pricing_rule(&self, id: Uuid) -> anyhow::Result<Option<PricingRule>>;
    async fn match_pricing_rules(
        &self,
        provider_code: &str,
        upstream_model_id: &str,
        at: OffsetDateTime,
    ) -> anyhow::Result<Vec<PricingRule>>;
    async fn upsert_pricing_rule(
        &self,
        input: &UpsertPricingRuleInput,
    ) -> anyhow::Result<PricingRule>;
    async fn delete_pricing_rule(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn billing_enabled_at(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Option<OffsetDateTime>>;
    async fn list_credit_accounts(
        &self,
        workspace_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<CreditAccountRecord>>;
    async fn get_credit_account(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<Option<CreditAccountRecord>>;
    async fn credit_target_is_root(&self, user_id: Uuid) -> anyhow::Result<bool>;
    async fn billing_session_scope(
        &self,
        billing_session_id: Uuid,
    ) -> anyhow::Result<Option<(Uuid, Uuid)>>;
    async fn execute_credit_command(
        &self,
        input: &CreditCommandInput,
    ) -> anyhow::Result<CreditTransactionRecord>;
    async fn record_credit_command_rejected(
        &self,
        workspace_id: Uuid,
        actor_plugin_id: &str,
        command: &str,
        reason: &str,
        idempotency_key: &str,
    ) -> anyhow::Result<()>;
    async fn reserve_credit(&self, input: &ReserveCreditInput)
        -> anyhow::Result<CreditReservation>;
    async fn settle_credit(
        &self,
        input: &SettleCreditInput,
    ) -> anyhow::Result<CreditTransactionRecord>;
    async fn release_credit(
        &self,
        billing_session_id: Uuid,
        reason: &str,
    ) -> anyhow::Result<Option<CreditTransactionRecord>>;
    async fn heartbeat_credit_reservation(
        &self,
        billing_session_id: Uuid,
        reservation_expires_at: OffsetDateTime,
    ) -> anyhow::Result<bool>;
    async fn list_credit_ledger(
        &self,
        input: &ListCreditLedgerInput,
    ) -> anyhow::Result<Vec<CreditTransactionRecord>>;
    async fn claim_credit_outbox_events(
        &self,
        worker_id: &str,
        limit: i64,
        locked_until: OffsetDateTime,
    ) -> anyhow::Result<Vec<CreditOutboxEvent>>;
    async fn complete_credit_outbox_event(
        &self,
        event_id: Uuid,
        worker_id: &str,
    ) -> anyhow::Result<bool>;
    async fn fail_credit_outbox_event(
        &self,
        event_id: Uuid,
        worker_id: &str,
        error: &str,
    ) -> anyhow::Result<bool>;
    async fn recover_expired_credit_reservations(
        &self,
        now: OffsetDateTime,
        limit: i64,
    ) -> anyhow::Result<usize>;
}
