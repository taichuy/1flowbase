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

    async fn sync_official_pricing_rules(
        &self,
        _: &[PricingRule],
    ) -> anyhow::Result<crate::ports::PricingCatalogSyncSummary> {
        unreachable!("credit command fixture does not sync pricing")
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
        rules: serde_json::json!([]),
        cache_write_token_unit_size: 1000000,
        cache_write_token_unit_price: Decimal::ZERO,
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
fn token_rating_uses_mutually_exclusive_input_and_cache_quantities() {
    let cost = rate_token_usage(
        &rule(),
        &TokenUsage {
            cache_write_tokens: 0,
            cache_write_by_ttl_seconds: None,
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
fn inconsistent_normalized_cache_miss_quantity_is_rejected() {
    let error = rate_token_usage(
        &rule(),
        &TokenUsage {
            cache_write_tokens: 0,
            cache_write_by_ttl_seconds: None,
            input_tokens: 999_999,
            input_cache_hit_tokens: 200_000,
            input_cache_miss_tokens: Some(10),
            output_tokens: 0,
        },
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "provider_usage_invalid");
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

// AC2: independent denominators, disjoint input and last matching sparse override.
#[test]
fn four_meter_sparse_rules_and_threshold_boundaries() {
    let mut pricing = rule();
    pricing.input_token_unit_size = 10;
    pricing.input_token_unit_price = Decimal::ONE;
    pricing.output_token_unit_size = 20;
    pricing.output_token_unit_price = Decimal::from(2);
    pricing.cache_hit_token_unit_size = 30;
    pricing.cache_hit_token_unit_price = Decimal::from(3);
    pricing.cache_write_token_unit_size = 40;
    pricing.cache_write_token_unit_price = Decimal::from(4);
    let usage = TokenUsage {
        input_tokens: 80,
        input_cache_hit_tokens: 30,
        cache_write_tokens: 40,
        output_tokens: 20,
        ..Default::default()
    };
    assert_eq!(
        rate_token_usage(&pricing, &usage).unwrap().total_cost,
        Decimal::TEN
    );
    pricing.rules = serde_json::json!([
        {"when":{"input_tokens":{"operator":"gte","value":80}},"overrides":{"input_token_unit_price":"5"}},
        {"when":{"input_tokens":{"operator":"gt","value":80}},"overrides":{"input_token_unit_price":"7","output_token_unit_price":"9"}},
        {"when":{"input_tokens":{"operator":"gte","value":80}},"overrides":{"cache_hit_token_unit_price":"6"}}
    ]);
    let at_boundary = rate_token_usage(&pricing, &usage).unwrap();
    assert_eq!(at_boundary.total_cost, Decimal::from(17));
    assert_eq!(at_boundary.matched_rule_indices, vec![0, 2]);
    let above = rate_token_usage(
        &pricing,
        &TokenUsage {
            input_tokens: 81,
            ..usage
        },
    )
    .unwrap();
    assert_eq!(above.applied_rates.input.unit_price, Decimal::from(7));
    assert_eq!(above.applied_rates.output.unit_price, Decimal::from(9));
    assert_eq!(above.applied_rates.cache_write.unit_size, 40);
    assert_eq!(above.matched_rule_indices, vec![0, 1, 2]);
}

// AC2: TTL evidence is mandatory, unmatched TTL keeps the fixed default.
#[test]
fn ttl_buckets_sparse_order_and_invalid_usage() {
    let mut pricing = rule();
    pricing.cache_write_token_unit_size = 100;
    pricing.cache_write_token_unit_price = Decimal::ONE;
    pricing.rules = serde_json::json!([
        {"when":{"cache_write_ttl_seconds":300},"overrides":{"cache_write_token_unit_price":"2","cache_write_token_unit_size":50}},
        {"when":{"input_tokens":{"operator":"gte","value":0}},"overrides":{"cache_write_token_unit_price":"3"}}
    ]);
    let usage = TokenUsage {
        input_tokens: 200,
        cache_write_tokens: 200,
        cache_write_by_ttl_seconds: Some([("300".into(), 100), ("999".into(), 100)].into()),
        ..Default::default()
    };
    let rated = rate_token_usage(&pricing, &usage).unwrap();
    assert_eq!(rated.cache_write_cost, Decimal::from(9));
    assert_eq!(rated.ordinary_input_tokens, 0);
    assert_eq!(
        rate_token_usage(
            &pricing,
            &TokenUsage {
                cache_write_by_ttl_seconds: None,
                ..usage.clone()
            }
        )
        .unwrap_err()
        .to_string(),
        "cache_write_ttl_required"
    );
    for bad in [
        TokenUsage {
            input_tokens: 199,
            ..usage.clone()
        },
        TokenUsage {
            input_cache_miss_tokens: Some(1),
            ..usage.clone()
        },
        TokenUsage {
            cache_write_tokens: 199,
            ..usage.clone()
        },
        TokenUsage {
            cache_write_by_ttl_seconds: Some([("0300".into(), 200)].into()),
            ..usage.clone()
        },
        TokenUsage {
            cache_write_by_ttl_seconds: Some([("300".into(), -1), ("999".into(), 201)].into()),
            ..usage.clone()
        },
    ] {
        assert!(rate_token_usage(&pricing, &bad).is_err());
    }
    pricing.rules = serde_json::json!([]);
    pricing.cache_write_token_unit_size = pricing.input_token_unit_size;
    pricing.cache_write_token_unit_price = pricing.input_token_unit_price;
    // Migrated legacy input-priced writes retain their original total, without double charging.
    assert_eq!(
        rate_token_usage(&pricing, &usage).unwrap().total_cost,
        Decimal::from(200) * pricing.input_token_unit_price
            / Decimal::from(pricing.input_token_unit_size)
    );
    pricing.input_token_unit_price = Decimal::MAX;
    assert!(rate_token_usage(
        &pricing,
        &TokenUsage {
            input_tokens: i64::MAX,
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn explicit_rating_time_applies_date_and_utc_window() {
    let mut pricing = rule();
    pricing.rules = serde_json::json!([{ "when": {
        "effective_from":"2026-09-10T00:00:00Z", "effective_to":"2026-09-12T00:00:00Z",
        "timezone":"UTC", "weekday_mask":8, "local_time_start":"23:00:00", "local_time_end":"01:00:00"
    }, "overrides":{"output_token_unit_price":"10"}}]);
    let usage = TokenUsage {
        output_tokens: 1_000_000,
        ..Default::default()
    };
    for (at, price) in [
        (datetime!(2026-09-10 23:00 UTC), 10),
        (datetime!(2026-09-10 00:30 UTC), 10),
        (datetime!(2026-09-10 01:00 UTC), 5),
        (datetime!(2026-09-11 00:30 UTC), 5),
        (datetime!(2026-09-12 00:00 UTC), 5),
    ] {
        assert_eq!(
            crate::billing::rate_token_usage_at(&pricing, &usage, at)
                .unwrap()
                .total_cost,
            Decimal::from(price)
        );
    }
}
