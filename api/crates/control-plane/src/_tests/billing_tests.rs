use crate::{
    billing::{rate_token_usage, CreditCommandService, PricingRule, TokenUsage},
    ports::{
        BillingRepository, CreditAccountRecord, CreditCommandInput, CreditOutboxEvent,
        CreditReservation, CreditTransactionRecord, ListCreditLedgerInput, ListPricingRulesInput,
        PricingRulesPage, ReserveCreditInput, SettleCreditInput, UpsertPricingRuleInput,
    },
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::{
    collections::BTreeSet,
    str::FromStr,
    sync::{Arc, Mutex},
};
use time::{
    macros::{datetime, time},
    OffsetDateTime, Weekday,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedCreditCommand {
    workspace_id: Uuid,
    user_id: Uuid,
    amount: String,
    credit_unit: String,
    command: String,
    reason: String,
    source_type: Option<String>,
    source_id: Option<String>,
    idempotency_key: String,
    actor_user_id: Option<Uuid>,
    actor_plugin_id: Option<String>,
    metadata: serde_json::Value,
}

impl From<&CreditCommandInput> for RecordedCreditCommand {
    fn from(input: &CreditCommandInput) -> Self {
        Self {
            workspace_id: input.workspace_id,
            user_id: input.user_id,
            amount: input.amount.clone(),
            credit_unit: input.credit_unit.clone(),
            command: input.command.clone(),
            reason: input.reason.clone(),
            source_type: input.source_type.clone(),
            source_id: input.source_id.clone(),
            idempotency_key: input.idempotency_key.clone(),
            actor_user_id: input.actor_user_id,
            actor_plugin_id: input.actor_plugin_id.clone(),
            metadata: input.metadata.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BillingWrite {
    Rejected {
        workspace_id: Uuid,
        actor_plugin_id: String,
        command: String,
        reason: String,
        idempotency_key: String,
    },
    Execute(Box<RecordedCreditCommand>),
}

#[derive(Default)]
struct RecordingBillingState {
    writes: Vec<BillingWrite>,
    accepted: Option<(RecordedCreditCommand, CreditTransactionRecord)>,
}

#[derive(Clone, Default)]
struct RecordingBillingRepository {
    state: Arc<Mutex<RecordingBillingState>>,
}

impl RecordingBillingRepository {
    fn writes(&self) -> Vec<BillingWrite> {
        self.state
            .lock()
            .expect("billing recording lock must not be poisoned")
            .writes
            .clone()
    }
}

#[async_trait]
impl BillingRepository for RecordingBillingRepository {
    async fn list_pricing_rules(
        &self,
        _input: &ListPricingRulesInput,
    ) -> anyhow::Result<PricingRulesPage> {
        unreachable!("credit command service fixture does not list pricing rules")
    }

    async fn get_pricing_rule(&self, _id: Uuid) -> anyhow::Result<Option<PricingRule>> {
        unreachable!("credit command service fixture does not read pricing rules")
    }

    async fn match_pricing_rules(
        &self,
        _provider_code: &str,
        _upstream_model_id: &str,
        _at: OffsetDateTime,
    ) -> anyhow::Result<Vec<PricingRule>> {
        unreachable!("credit command service fixture does not match pricing rules")
    }

    async fn upsert_pricing_rule(
        &self,
        _input: &UpsertPricingRuleInput,
    ) -> anyhow::Result<PricingRule> {
        unreachable!("credit command service fixture does not write pricing rules")
    }

    async fn insert_pricing_rule_if_absent(
        &self,
        _input: &UpsertPricingRuleInput,
    ) -> anyhow::Result<Option<PricingRule>> {
        unreachable!("credit command service fixture does not install pricing rules")
    }

    async fn delete_pricing_rule(&self, _id: Uuid) -> anyhow::Result<bool> {
        unreachable!("credit command service fixture does not delete pricing rules")
    }

    async fn billing_enabled_at(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<Option<OffsetDateTime>> {
        unreachable!("credit command service fixture does not inspect billing state")
    }

    async fn list_credit_accounts(
        &self,
        _workspace_id: Uuid,
        _limit: i64,
        _offset: i64,
    ) -> anyhow::Result<Vec<CreditAccountRecord>> {
        unreachable!("credit command service fixture does not list accounts")
    }

    async fn get_credit_account(
        &self,
        _workspace_id: Uuid,
        _user_id: Uuid,
    ) -> anyhow::Result<Option<CreditAccountRecord>> {
        unreachable!("credit command service fixture does not read accounts")
    }

    async fn credit_target_is_root(&self, _user_id: Uuid) -> anyhow::Result<bool> {
        unreachable!("credit command service fixture does not inspect root identity")
    }

    async fn billing_session_scope(
        &self,
        _billing_session_id: Uuid,
    ) -> anyhow::Result<Option<(Uuid, Uuid)>> {
        unreachable!("credit command service fixture does not read billing sessions")
    }

    async fn execute_credit_command(
        &self,
        input: &CreditCommandInput,
    ) -> anyhow::Result<CreditTransactionRecord> {
        let command = RecordedCreditCommand::from(input);
        let mut state = self
            .state
            .lock()
            .expect("billing recording lock must not be poisoned");
        state
            .writes
            .push(BillingWrite::Execute(Box::new(command.clone())));
        if let Some((accepted, transaction)) = &state.accepted {
            if accepted.idempotency_key == command.idempotency_key {
                if accepted == &command {
                    return Ok(transaction.clone());
                }
                anyhow::bail!("credit_idempotency_payload_mismatch");
            }
        }

        let id = Uuid::now_v7();
        let transaction = CreditTransactionRecord {
            id,
            transaction_id: id,
            account_id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            user_id: input.user_id,
            billing_session_id: None,
            actor_user_id: input.actor_user_id,
            actor_plugin_id: input.actor_plugin_id.clone(),
            transaction_type: input.command.clone(),
            amount: input.amount.clone(),
            balance_after: input.amount.clone(),
            reserved_after: "0".into(),
            credit_unit: input.credit_unit.clone(),
            reason: input.reason.clone(),
            source_type: input.source_type.clone(),
            source_id: input.source_id.clone(),
            idempotency_key: input.idempotency_key.clone(),
            status: "posted".into(),
            metadata: input.metadata.clone(),
            created_at: OffsetDateTime::UNIX_EPOCH,
        };
        state.accepted = Some((command, transaction.clone()));
        Ok(transaction)
    }

    async fn record_credit_command_rejected(
        &self,
        workspace_id: Uuid,
        actor_plugin_id: &str,
        command: &str,
        reason: &str,
        idempotency_key: &str,
    ) -> anyhow::Result<()> {
        self.state
            .lock()
            .expect("billing recording lock must not be poisoned")
            .writes
            .push(BillingWrite::Rejected {
                workspace_id,
                actor_plugin_id: actor_plugin_id.into(),
                command: command.into(),
                reason: reason.into(),
                idempotency_key: idempotency_key.into(),
            });
        Ok(())
    }

    async fn reserve_credit(
        &self,
        _input: &ReserveCreditInput,
    ) -> anyhow::Result<CreditReservation> {
        unreachable!("credit command service fixture does not reserve credit")
    }

    async fn settle_credit(
        &self,
        _input: &SettleCreditInput,
    ) -> anyhow::Result<CreditTransactionRecord> {
        unreachable!("credit command service fixture does not settle credit")
    }

    async fn release_credit(
        &self,
        _billing_session_id: Uuid,
        _reason: &str,
    ) -> anyhow::Result<Option<CreditTransactionRecord>> {
        unreachable!("credit command service fixture does not release credit")
    }

    async fn heartbeat_credit_reservation(
        &self,
        _billing_session_id: Uuid,
        _reservation_expires_at: OffsetDateTime,
    ) -> anyhow::Result<bool> {
        unreachable!("credit command service fixture does not heartbeat reservations")
    }

    async fn list_credit_ledger(
        &self,
        _input: &ListCreditLedgerInput,
    ) -> anyhow::Result<Vec<CreditTransactionRecord>> {
        unreachable!("credit command service fixture does not list ledger entries")
    }

    async fn claim_credit_outbox_events(
        &self,
        _worker_id: &str,
        _limit: i64,
        _locked_until: OffsetDateTime,
    ) -> anyhow::Result<Vec<CreditOutboxEvent>> {
        unreachable!("credit command service fixture does not claim outbox events")
    }

    async fn complete_credit_outbox_event(
        &self,
        _event_id: Uuid,
        _worker_id: &str,
    ) -> anyhow::Result<bool> {
        unreachable!("credit command service fixture does not complete outbox events")
    }

    async fn fail_credit_outbox_event(
        &self,
        _event_id: Uuid,
        _worker_id: &str,
        _error: &str,
    ) -> anyhow::Result<bool> {
        unreachable!("credit command service fixture does not fail outbox events")
    }

    async fn recover_expired_credit_reservations(
        &self,
        _now: OffsetDateTime,
        _limit: i64,
    ) -> anyhow::Result<usize> {
        unreachable!("credit command service fixture does not recover reservations")
    }
}

fn rule() -> PricingRule {
    PricingRule {
        id: uuid::Uuid::now_v7(),
        provider_code: "openai".to_string(),
        upstream_model_id: "gpt-test".to_string(),
        input_token_unit_size: 1_000_000,
        input_token_unit_price: Decimal::from_str("1.25").unwrap(),
        output_token_unit_size: 1_000_000,
        output_token_unit_price: Decimal::from_str("5.00").unwrap(),
        cache_hit_token_unit_size: 1_000_000,
        cache_hit_token_unit_price: Decimal::from_str("0.25").unwrap(),
        currency_code: "USD".to_string(),
        effective_from: datetime!(2026-01-01 00:00 UTC),
        effective_to: None,
        timezone: "UTC".to_string(),
        weekday_mask: 0b111_1111,
        local_time_start: None,
        local_time_end: None,
        priority: 0,
        enabled: true,
        rating_policy_enabled: false,
        rating_policy: serde_json::json!({}),
        source_kind: "manual".to_string(),
        source_catalog_id: None,
        source_version: None,
        source_checksum: None,
        extensions: serde_json::json!({}),
        created_by: None,
        created_at: datetime!(2026-01-01 00:00 UTC),
        updated_at: datetime!(2026-01-01 00:00 UTC),
    }
}

#[tokio::test]
async fn plugin_credit_command_requires_capability_permission_and_remains_idempotent() {
    let repository = RecordingBillingRepository::default();
    let service = CreditCommandService::new(repository.clone());
    let workspace_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let command = CreditCommandInput {
        workspace_id,
        user_id,
        amount: "2.50".into(),
        credit_unit: "USD".into(),
        command: "grant".into(),
        reason: "daily_checkin".into(),
        source_type: Some("checkin".into()),
        source_id: Some("2026-08-17".into()),
        idempotency_key: "checkin:user:2026-08-17".into(),
        actor_user_id: None,
        actor_plugin_id: None,
        metadata: serde_json::json!({}),
    };

    let denied = service
        .execute_plugin_command("checkin-plugin", &BTreeSet::new(), command.clone())
        .await
        .unwrap_err();
    assert!(denied
        .to_string()
        .contains("credit_command_permission_denied"));

    let permissions = BTreeSet::from(["credit.grant".to_string()]);
    let first = service
        .execute_plugin_command("checkin-plugin", &permissions, command.clone())
        .await
        .unwrap();
    let repeated = service
        .execute_plugin_command("checkin-plugin", &permissions, command.clone())
        .await
        .unwrap();
    assert_eq!(first.id, repeated.id);
    assert_eq!(first.actor_plugin_id.as_deref(), Some("checkin-plugin"));

    let mut conflicting = command.clone();
    conflicting.amount = "9.00".into();
    let conflict = service
        .execute_plugin_command("checkin-plugin", &permissions, conflicting.clone())
        .await
        .unwrap_err();
    assert!(conflict
        .to_string()
        .contains("credit_idempotency_payload_mismatch"));

    let mut accepted_write = RecordedCreditCommand::from(&command);
    accepted_write.actor_plugin_id = Some("checkin-plugin".into());
    let mut conflicting_write = RecordedCreditCommand::from(&conflicting);
    conflicting_write.actor_plugin_id = Some("checkin-plugin".into());
    assert_eq!(
        repository.writes(),
        vec![
            BillingWrite::Rejected {
                workspace_id,
                actor_plugin_id: "checkin-plugin".into(),
                command: "grant".into(),
                reason: "credit_command_permission_denied".into(),
                idempotency_key: "checkin:user:2026-08-17".into(),
            },
            BillingWrite::Execute(Box::new(accepted_write.clone())),
            BillingWrite::Execute(Box::new(accepted_write)),
            BillingWrite::Execute(Box::new(conflicting_write)),
        ]
    );
}

#[test]
fn input_token_tier_overrides_base_rates_at_the_configured_threshold() {
    let mut tiered = rule();
    tiered.rating_policy_enabled = true;
    tiered.rating_policy = serde_json::json!({
        "schema_version": "1flowbase.model-rating-policy/v1",
        "type": "input_token_tiers",
        "tiers": [{
            "when": { "operator": "gt", "value": 272000 },
            "rates": {
                "input": { "unit_size": 1000000, "unit_price": "10" },
                "output": { "unit_size": 1000000, "unit_price": "45" },
                "cache_hit": { "unit_size": 1000000, "unit_price": "1" }
            }
        }]
    });

    let below = rate_token_usage(
        &tiered,
        &TokenUsage {
            input_tokens: 272_000,
            input_cache_hit_tokens: 0,
            input_cache_miss_tokens: Some(272_000),
            output_tokens: 0,
        },
    )
    .unwrap();
    assert!(below.rating_policy_match.is_none());
    assert_eq!(
        below.applied_rates.input.unit_price,
        Decimal::from_str("1.25").unwrap()
    );

    let above = rate_token_usage(
        &tiered,
        &TokenUsage {
            input_tokens: 300_000,
            input_cache_hit_tokens: 0,
            input_cache_miss_tokens: Some(300_000),
            output_tokens: 10_000,
        },
    )
    .unwrap();
    assert_eq!(above.rating_policy_match.unwrap().tier_index, 0);
    assert_eq!(above.applied_rates.input.unit_price, Decimal::TEN);
    assert_eq!(above.applied_rates.output.unit_price, Decimal::from(45));
    assert_eq!(above.total_cost, Decimal::from_str("3.45").unwrap());
}

#[test]
fn enabled_rating_policy_rejects_unknown_or_ambiguous_tiers() {
    let mut invalid = rule();
    invalid.rating_policy_enabled = true;
    invalid.rating_policy = serde_json::json!({
        "schema_version": "1flowbase.model-rating-policy/v1",
        "type": "unknown",
        "tiers": []
    });
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "rating_policy_invalid"
    );

    invalid.rating_policy = serde_json::json!({
        "schema_version": "1flowbase.model-rating-policy/v1",
        "type": "input_token_tiers",
        "tiers": [
            {
                "when": { "operator": "gte", "value": 200000 },
                "rates": {
                    "input": { "unit_size": 1000000, "unit_price": "2" },
                    "output": { "unit_size": 1000000, "unit_price": "6" },
                    "cache_hit": { "unit_size": 1000000, "unit_price": "0.5" }
                }
            },
            {
                "when": { "operator": "gt", "value": 200000 },
                "rates": {
                    "input": { "unit_size": 1000000, "unit_price": "4" },
                    "output": { "unit_size": 1000000, "unit_price": "12" },
                    "cache_hit": { "unit_size": 1000000, "unit_price": "1" }
                }
            }
        ]
    });
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "rating_policy_tiers_not_strictly_ascending"
    );
}

#[test]
fn token_rating_uses_mutually_exclusive_input_and_cache_quantities() {
    let cost = rate_token_usage(
        &rule(),
        &TokenUsage {
            input_tokens: 300_000,
            input_cache_hit_tokens: 200_000,
            input_cache_miss_tokens: None,
            output_tokens: 50_000,
        },
    )
    .unwrap();

    assert_eq!(cost.ordinary_input_tokens, 100_000);
    assert_eq!(cost.cache_hit_tokens, 200_000);
    assert_eq!(cost.input_cost, Decimal::from_str("0.125").unwrap());
    assert_eq!(cost.output_cost, Decimal::from_str("0.25").unwrap());
    assert_eq!(cost.cache_hit_cost, Decimal::from_str("0.05").unwrap());
    assert_eq!(cost.total_cost, Decimal::from_str("0.425").unwrap());
}

#[test]
fn explicit_cache_miss_quantity_wins_over_derived_input_quantity() {
    let cost = rate_token_usage(
        &rule(),
        &TokenUsage {
            input_tokens: 999_999,
            input_cache_hit_tokens: 200_000,
            input_cache_miss_tokens: Some(10),
            output_tokens: 0,
        },
    )
    .unwrap();
    assert_eq!(cost.ordinary_input_tokens, 10);
}

#[test]
fn pricing_rule_rejects_invalid_core_rates_and_currency() {
    let mut invalid = rule();
    invalid.currency_code = "CNY".to_string();
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "billing_currency_not_supported"
    );
    invalid.currency_code = "USD".to_string();
    invalid.input_token_unit_size = 0;
    assert_eq!(
        invalid.validate().unwrap_err().to_string(),
        "pricing_unit_size_invalid"
    );
}

#[test]
fn weekday_mask_uses_monday_as_low_bit() {
    assert_eq!(crate::billing::weekday_bit(Weekday::Monday), 1);
    assert_eq!(crate::billing::weekday_bit(Weekday::Sunday), 64);
}

#[test]
fn daily_window_can_end_at_midnight_without_a_pricing_gap() {
    let mut wrapping = rule();
    wrapping.local_time_start = Some(time!(10:00));
    wrapping.local_time_end = Some(time!(00:00));
    assert!(crate::billing::rule_matches_local_window(
        &wrapping,
        datetime!(2026-08-18 23:59:59.999_999_999 UTC)
    )
    .unwrap());
    assert!(!crate::billing::rule_matches_local_window(
        &wrapping,
        datetime!(2026-08-18 09:59:59 UTC)
    )
    .unwrap());

    wrapping.weekday_mask = 0b000_1111;
    assert_eq!(
        wrapping.validate().unwrap_err().to_string(),
        "pricing_local_time_range_invalid"
    );
}

#[test]
fn pricing_cache_key_is_unambiguous_and_stable() {
    assert_eq!(
        crate::billing::pricing_rules_cache_key("open:ai", "gpt"),
        crate::billing::pricing_rules_cache_key("open:ai", "gpt")
    );
    assert_ne!(
        crate::billing::pricing_rules_cache_key("open:ai", "gpt"),
        crate::billing::pricing_rules_cache_key("open", "ai:gpt")
    );
}

#[test]
fn exact_pricing_rule_wins_over_global_zero_fallback() {
    let at = datetime!(2026-08-17 00:00 UTC);
    let mut exact = rule();
    exact.priority = 0;
    let mut fallback = rule();
    fallback.provider_code = "zero".to_string();
    fallback.upstream_model_id = "any".to_string();
    fallback.priority = 999;
    fallback.input_token_unit_price = Decimal::ZERO;
    fallback.output_token_unit_price = Decimal::ZERO;
    fallback.cache_hit_token_unit_price = Decimal::ZERO;

    let selected = crate::billing::choose_pricing_rule_for(
        "openai",
        "gpt-test",
        vec![fallback.clone(), exact.clone()],
        at,
    )
    .unwrap()
    .unwrap();
    assert_eq!(selected.id, exact.id);

    let selected = crate::billing::choose_pricing_rule_for(
        "anthropic",
        "claude-test",
        vec![fallback.clone()],
        at,
    )
    .unwrap()
    .unwrap();
    assert_eq!(selected.id, fallback.id);
}
