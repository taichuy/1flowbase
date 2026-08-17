use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    billing::PricingRule,
    errors::ControlPlaneError,
    ports::{
        BillingRepository, CreditAccountRecord, CreditCommandInput, CreditOutboxEvent,
        CreditReservation, CreditTransactionRecord, ListCreditLedgerInput, ListPricingRulesInput,
        PricingRulesPage, ReserveCreditInput, SettleCreditInput, UpsertPricingRuleInput,
    },
};
use sqlx::{Postgres, QueryBuilder, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn pricing_rule(row: sqlx::postgres::PgRow) -> Result<PricingRule> {
    Ok(PricingRule {
        id: row.try_get("id")?,
        provider_code: row.try_get("provider_code")?,
        upstream_model_id: row.try_get("upstream_model_id")?,
        input_token_unit_size: row.try_get("input_token_unit_size")?,
        input_token_unit_price: row
            .try_get::<String, _>("input_token_unit_price")?
            .parse()?,
        output_token_unit_size: row.try_get("output_token_unit_size")?,
        output_token_unit_price: row
            .try_get::<String, _>("output_token_unit_price")?
            .parse()?,
        cache_hit_token_unit_size: row.try_get("cache_hit_token_unit_size")?,
        cache_hit_token_unit_price: row
            .try_get::<String, _>("cache_hit_token_unit_price")?
            .parse()?,
        currency_code: row.try_get("currency_code")?,
        effective_from: row.try_get("effective_from")?,
        effective_to: row.try_get("effective_to")?,
        timezone: row.try_get("timezone")?,
        weekday_mask: row.try_get("weekday_mask")?,
        local_time_start: row.try_get("local_time_start")?,
        local_time_end: row.try_get("local_time_end")?,
        priority: row.try_get("priority")?,
        enabled: row.try_get("enabled")?,
        source_kind: row.try_get("source_kind")?,
        source_catalog_id: row.try_get("source_catalog_id")?,
        source_version: row.try_get("source_version")?,
        source_checksum: row.try_get("source_checksum")?,
        extensions: row.try_get("extensions")?,
        created_by: row.try_get("created_by")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

const PRICING_SELECT: &str = r#"
    select id, provider_code, upstream_model_id,
           input_token_unit_size, input_token_unit_price::text as input_token_unit_price,
           output_token_unit_size, output_token_unit_price::text as output_token_unit_price,
           cache_hit_token_unit_size, cache_hit_token_unit_price::text as cache_hit_token_unit_price,
           currency_code, effective_from, effective_to, timezone, weekday_mask,
           local_time_start, local_time_end, priority, enabled, source_kind,
           source_catalog_id, source_version, source_checksum, extensions,
           created_by, created_at, updated_at
    from model_pricing_rules
"#;

fn account_record(row: &sqlx::postgres::PgRow) -> Result<CreditAccountRecord> {
    Ok(CreditAccountRecord {
        id: row.try_get("id")?,
        workspace_id: row.try_get("workspace_id")?,
        user_id: row.try_get("user_id")?,
        credit_unit: row.try_get("credit_unit")?,
        charge_enabled: row.try_get("charge_enabled")?,
        current_balance: row.try_get("current_balance")?,
        reserved_amount: row.try_get("reserved_amount")?,
        available_balance: row.try_get("available_balance")?,
        credit_insufficient: row.try_get("credit_insufficient")?,
        revision: row.try_get("revision")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn transaction_record(row: &sqlx::postgres::PgRow) -> Result<CreditTransactionRecord> {
    Ok(CreditTransactionRecord {
        id: row.try_get("id")?,
        transaction_id: row.try_get("transaction_id")?,
        account_id: row.try_get("account_id")?,
        workspace_id: row.try_get("workspace_id")?,
        user_id: row.try_get("user_id")?,
        billing_session_id: row.try_get("billing_session_id")?,
        actor_user_id: row.try_get("actor_user_id")?,
        actor_plugin_id: row.try_get("actor_plugin_id")?,
        transaction_type: row.try_get("transaction_type")?,
        amount: row.try_get("amount")?,
        balance_after: row.try_get("balance_after")?,
        reserved_after: row.try_get("reserved_after")?,
        credit_unit: row.try_get("credit_unit")?,
        reason: row.try_get("reason")?,
        source_type: row.try_get("source_type")?,
        source_id: row.try_get("source_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        status: row.try_get("status")?,
        metadata: row.try_get("metadata")?,
        created_at: row.try_get("created_at")?,
    })
}

const CREDIT_LEDGER_COLUMNS: &str = r#"id, transaction_id, account_id, workspace_id, user_id,
           billing_session_id, actor_user_id, actor_plugin_id, transaction_type,
           amount::text as amount, coalesce(balance_after, 0)::text as balance_after,
           coalesce(reserved_after, 0)::text as reserved_after, credit_unit, reason,
           source_type, source_id, idempotency_key, status, metadata, created_at"#;

const CREDIT_LEDGER_SELECT: &str = r#"select id, transaction_id, account_id, workspace_id, user_id,
           billing_session_id, actor_user_id, actor_plugin_id, transaction_type,
           amount::text as amount, coalesce(balance_after, 0)::text as balance_after,
           coalesce(reserved_after, 0)::text as reserved_after, credit_unit, reason,
           source_type, source_id, idempotency_key, status, metadata, created_at
    from runtime_credit_ledger"#;

async fn ensure_account<'a>(
    tx: &mut Transaction<'a, Postgres>,
    workspace_id: Uuid,
    user_id: Uuid,
    charge_enabled_default: bool,
) -> Result<CreditAccountRecord> {
    sqlx::query(
        r#"insert into user_credit_accounts
           (id, workspace_id, user_id, credit_unit, charge_enabled)
           values ($1, $2, $3, 'USD', $4)
           on conflict (workspace_id, user_id, credit_unit) do nothing"#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(user_id)
    .bind(charge_enabled_default)
    .execute(&mut **tx)
    .await?;
    let row = sqlx::query(
        r#"select id, workspace_id, user_id, credit_unit, charge_enabled,
                  current_balance::text as current_balance,
                  reserved_amount::text as reserved_amount,
                  (current_balance-reserved_amount)::text as available_balance,
                  (current_balance-reserved_amount)<0 as credit_insufficient,
                  revision, created_at, updated_at
           from user_credit_accounts
           where workspace_id = $1 and user_id = $2 and credit_unit = 'USD'
           for update"#,
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;
    account_record(&row)
}

async fn insert_outbox(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    account_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<()> {
    sqlx::query(
        "insert into credit_event_outbox (event_id, workspace_id, account_id, event_type, payload) values ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(account_id)
    .bind(event_type)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[async_trait]
impl BillingRepository for PgControlPlaneStore {
    async fn list_pricing_rules(&self, input: &ListPricingRulesInput) -> Result<PricingRulesPage> {
        let mut query = QueryBuilder::<Postgres>::new(PRICING_SELECT);
        query.push(" where true");
        if let Some(provider_code) = &input.provider_code {
            query
                .push(" and provider_code ilike ")
                .push_bind(format!("%{provider_code}%"));
        }
        if let Some(model) = &input.upstream_model_id {
            query
                .push(" and upstream_model_id ilike ")
                .push_bind(format!("%{model}%"));
        }
        if let Some(enabled) = input.enabled {
            query.push(" and enabled = ").push_bind(enabled);
        }
        if let Some(source_kind) = &input.source_kind {
            query.push(" and source_kind = ").push_bind(source_kind);
        }
        query.push(
            " order by provider_code, upstream_model_id, priority desc, effective_from desc, id",
        );
        query
            .push(" limit ")
            .push_bind(input.page_size.clamp(1, 500));
        query.push(" offset ").push_bind(input.offset.max(0));
        let items = query
            .build()
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(pricing_rule)
            .collect::<Result<Vec<_>>>()?;

        let mut count = QueryBuilder::<Postgres>::new(
            "select count(*)::bigint from model_pricing_rules where true",
        );
        if let Some(provider_code) = &input.provider_code {
            count
                .push(" and provider_code ilike ")
                .push_bind(format!("%{provider_code}%"));
        }
        if let Some(model) = &input.upstream_model_id {
            count
                .push(" and upstream_model_id ilike ")
                .push_bind(format!("%{model}%"));
        }
        if let Some(enabled) = input.enabled {
            count.push(" and enabled = ").push_bind(enabled);
        }
        if let Some(source_kind) = &input.source_kind {
            count.push(" and source_kind = ").push_bind(source_kind);
        }
        let total_count = count.build_query_scalar().fetch_one(self.pool()).await?;
        Ok(PricingRulesPage { items, total_count })
    }

    async fn get_pricing_rule(&self, id: Uuid) -> Result<Option<PricingRule>> {
        let row = sqlx::query(&(PRICING_SELECT.to_owned() + " where id = $1"))
            .bind(id)
            .fetch_optional(self.pool())
            .await?;
        row.map(pricing_rule).transpose()
    }

    async fn match_pricing_rules(
        &self,
        provider_code: &str,
        upstream_model_id: &str,
        at: OffsetDateTime,
    ) -> Result<Vec<PricingRule>> {
        let rows = sqlx::query(
            &(PRICING_SELECT.to_owned()
                + r#"
            where (
                    (provider_code = $1 and upstream_model_id = $2)
                    or (provider_code = 'zero' and upstream_model_id = 'any')
                  )
              and enabled = true
              and effective_from <= $3 and (effective_to is null or effective_to > $3)
            order by priority desc, effective_from desc, id"#),
        )
        .bind(provider_code)
        .bind(upstream_model_id)
        .bind(at)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(pricing_rule).collect()
    }

    async fn upsert_pricing_rule(&self, input: &UpsertPricingRuleInput) -> Result<PricingRule> {
        input.rule.validate()?;
        let rule = &input.rule;
        let overlaps: bool = sqlx::query_scalar(
            r#"select exists (
                select 1 from model_pricing_rules existing
                where existing.id <> $1
                  and existing.enabled and $2
                  and existing.provider_code = $3
                  and existing.upstream_model_id = $4
                  and existing.priority = $5
                  and existing.effective_from < coalesce($7, 'infinity'::timestamptz)
                  and $6 < coalesce(existing.effective_to, 'infinity'::timestamptz)
                  and (existing.weekday_mask & $8) <> 0
                  and (
                    existing.local_time_start is null or $9::time is null or
                    (existing.local_time_start < $10::time and $9::time < existing.local_time_end)
                  )
            )"#,
        )
        .bind(rule.id)
        .bind(rule.enabled)
        .bind(&rule.provider_code)
        .bind(&rule.upstream_model_id)
        .bind(rule.priority)
        .bind(rule.effective_from)
        .bind(rule.effective_to)
        .bind(rule.weekday_mask)
        .bind(rule.local_time_start)
        .bind(rule.local_time_end)
        .fetch_one(self.pool())
        .await?;
        if overlaps {
            return Err(ControlPlaneError::Conflict("pricing_rule_conflict").into());
        }
        let row = sqlx::query(r#"
            insert into model_pricing_rules (
                id, provider_code, upstream_model_id,
                input_token_unit_size, input_token_unit_price,
                output_token_unit_size, output_token_unit_price,
                cache_hit_token_unit_size, cache_hit_token_unit_price,
                currency_code, effective_from, effective_to, timezone, weekday_mask,
                local_time_start, local_time_end, priority, enabled, source_kind,
                source_catalog_id, source_version, source_checksum, extensions, created_by
            ) values ($1,$2,$3,$4,$5::numeric,$6,$7::numeric,$8,$9::numeric,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24)
            on conflict (id) do update set
                provider_code=excluded.provider_code, upstream_model_id=excluded.upstream_model_id,
                input_token_unit_size=excluded.input_token_unit_size, input_token_unit_price=excluded.input_token_unit_price,
                output_token_unit_size=excluded.output_token_unit_size, output_token_unit_price=excluded.output_token_unit_price,
                cache_hit_token_unit_size=excluded.cache_hit_token_unit_size, cache_hit_token_unit_price=excluded.cache_hit_token_unit_price,
                currency_code=excluded.currency_code, effective_from=excluded.effective_from, effective_to=excluded.effective_to,
                timezone=excluded.timezone, weekday_mask=excluded.weekday_mask, local_time_start=excluded.local_time_start,
                local_time_end=excluded.local_time_end, priority=excluded.priority, enabled=excluded.enabled,
                source_kind=excluded.source_kind, source_catalog_id=excluded.source_catalog_id,
                source_version=excluded.source_version, source_checksum=excluded.source_checksum,
                extensions=excluded.extensions, updated_at=now()
            returning id, provider_code, upstream_model_id,
                input_token_unit_size, input_token_unit_price::text as input_token_unit_price,
                output_token_unit_size, output_token_unit_price::text as output_token_unit_price,
                cache_hit_token_unit_size, cache_hit_token_unit_price::text as cache_hit_token_unit_price,
                currency_code, effective_from, effective_to, timezone, weekday_mask,
                local_time_start, local_time_end, priority, enabled, source_kind,
                source_catalog_id, source_version, source_checksum, extensions, created_by, created_at, updated_at
        "#)
        .bind(rule.id).bind(&rule.provider_code).bind(&rule.upstream_model_id)
        .bind(rule.input_token_unit_size).bind(rule.input_token_unit_price.to_string())
        .bind(rule.output_token_unit_size).bind(rule.output_token_unit_price.to_string())
        .bind(rule.cache_hit_token_unit_size).bind(rule.cache_hit_token_unit_price.to_string())
        .bind(&rule.currency_code).bind(rule.effective_from).bind(rule.effective_to)
        .bind(&rule.timezone).bind(rule.weekday_mask).bind(rule.local_time_start).bind(rule.local_time_end)
        .bind(rule.priority).bind(rule.enabled).bind(&rule.source_kind).bind(&rule.source_catalog_id)
        .bind(&rule.source_version).bind(&rule.source_checksum).bind(&rule.extensions).bind(rule.created_by)
        .fetch_one(self.pool()).await?;
        pricing_rule(row)
    }

    async fn delete_pricing_rule(&self, id: Uuid) -> Result<bool> {
        Ok(sqlx::query("delete from model_pricing_rules where id = $1")
            .bind(id)
            .execute(self.pool())
            .await?
            .rows_affected()
            > 0)
    }

    async fn billing_enabled_at(&self, workspace_id: Uuid) -> Result<Option<OffsetDateTime>> {
        Ok(sqlx::query_scalar(
            "select billing_enabled_at from workspace_billing_settings where workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?)
    }

    async fn list_credit_accounts(
        &self,
        workspace_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CreditAccountRecord>> {
        let rows = sqlx::query(
            r#"select id, workspace_id, user_id, credit_unit, charge_enabled,
            current_balance::text as current_balance, reserved_amount::text as reserved_amount,
            (current_balance-reserved_amount)::text as available_balance,
            (current_balance-reserved_amount)<0 as credit_insufficient,
            revision, created_at, updated_at from user_credit_accounts
            where workspace_id=$1 order by updated_at desc, id desc limit $2 offset $3"#,
        )
        .bind(workspace_id)
        .bind(limit.clamp(1, 500))
        .bind(offset.max(0))
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(account_record).collect()
    }

    async fn get_credit_account(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<CreditAccountRecord>> {
        let row = sqlx::query(
            r#"select id, workspace_id, user_id, credit_unit, charge_enabled,
            current_balance::text as current_balance, reserved_amount::text as reserved_amount,
            (current_balance-reserved_amount)::text as available_balance,
            (current_balance-reserved_amount)<0 as credit_insufficient,
            revision, created_at, updated_at from user_credit_accounts
            where workspace_id=$1 and user_id=$2 and credit_unit='USD'"#,
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await?;
        row.as_ref().map(account_record).transpose()
    }

    async fn credit_target_is_root(&self, user_id: Uuid) -> Result<bool> {
        crate::repositories::is_root_user(self.pool(), user_id).await
    }

    async fn billing_session_scope(
        &self,
        billing_session_id: Uuid,
    ) -> Result<Option<(Uuid, Uuid)>> {
        Ok(sqlx::query_as(
            "select workspace_id,user_id from billing_sessions where id=$1 and user_id is not null",
        )
        .bind(billing_session_id)
        .fetch_optional(self.pool())
        .await?)
    }

    async fn execute_credit_command(
        &self,
        input: &CreditCommandInput,
    ) -> Result<CreditTransactionRecord> {
        if input.credit_unit != "USD" {
            return Err(ControlPlaneError::InvalidInput("credit_unit").into());
        }
        let amount: rust_decimal::Decimal = input
            .amount
            .parse()
            .map_err(|_| ControlPlaneError::InvalidInput("amount"))?;
        let signed = match input.command.as_str() {
            "grant" | "refund" if amount > rust_decimal::Decimal::ZERO => amount,
            "charge" if amount > rust_decimal::Decimal::ZERO => -amount,
            "adjustment" => amount,
            "enable_charge" | "disable_charge" if amount == rust_decimal::Decimal::ZERO => amount,
            _ => return Err(ControlPlaneError::InvalidInput("credit_command").into()),
        };
        let charge_enabled_default =
            !crate::repositories::is_root_user(self.pool(), input.user_id).await?;
        let mut tx = self.pool().begin().await?;
        let account = ensure_account(
            &mut tx,
            input.workspace_id,
            input.user_id,
            charge_enabled_default,
        )
        .await?;
        if let Some(row) = sqlx::query(
            &(CREDIT_LEDGER_SELECT.to_owned() + " where account_id=$1 and idempotency_key=$2"),
        )
        .bind(account.id)
        .bind(&input.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            let existing = transaction_record(&row)?;
            let existing_amount: rust_decimal::Decimal = existing.amount.parse()?;
            if existing.transaction_type != input.command
                || existing_amount != signed
                || existing.reason != input.reason
                || existing.source_type != input.source_type
                || existing.source_id != input.source_id
                || existing.actor_user_id != input.actor_user_id
                || existing.actor_plugin_id != input.actor_plugin_id
                || existing.metadata != input.metadata
            {
                return Err(
                    ControlPlaneError::Conflict("credit_idempotency_payload_mismatch").into(),
                );
            }
            tx.commit().await?;
            return Ok(existing);
        }
        let charge_enabled = match input.command.as_str() {
            "enable_charge" => true,
            "disable_charge" => false,
            _ => account.charge_enabled,
        };
        let row = sqlx::query(r#"update user_credit_accounts set
              current_balance=current_balance+$2::numeric, charge_enabled=$3,
              revision=revision+1, updated_at=now() where id=$1
              returning current_balance::text as current_balance, reserved_amount::text as reserved_amount"#)
            .bind(account.id).bind(signed.to_string()).bind(charge_enabled).fetch_one(&mut *tx).await?;
        let balance: String = row.try_get("current_balance")?;
        let reserved: String = row.try_get("reserved_amount")?;
        let ledger_id = Uuid::now_v7();
        let transaction_id = Uuid::now_v7();
        let ledger = sqlx::query(&(format!(r#"insert into runtime_credit_ledger
            (id,transaction_id,account_id,workspace_id,user_id,actor_user_id,actor_plugin_id,
             transaction_type,amount,balance_after,reserved_after,credit_unit,reason,source_type,
             source_id,idempotency_key,status,metadata)
            values ($1,$2,$3,$4,$5,$6,$7,$8,$9::numeric,$10::numeric,$11::numeric,'USD',$12,$13,$14,$15,'completed',$16)
            returning {}"#, CREDIT_LEDGER_COLUMNS)))
            .bind(ledger_id).bind(transaction_id).bind(account.id).bind(input.workspace_id).bind(input.user_id)
            .bind(input.actor_user_id).bind(&input.actor_plugin_id).bind(&input.command).bind(signed.to_string())
            .bind(&balance).bind(&reserved).bind(&input.reason).bind(&input.source_type).bind(&input.source_id)
            .bind(&input.idempotency_key).bind(&input.metadata).fetch_one(&mut *tx).await?;
        insert_outbox(&mut tx, input.workspace_id, account.id, match input.command.as_str() {
            "grant" => "CreditGranted", "charge" => "CreditCharged", "refund" => "CreditRefunded",
            "adjustment" => "CreditAdjusted", _ => "CreditChargeSettingChanged",
        }, serde_json::json!({"ledger_id": ledger_id, "transaction_id": transaction_id, "user_id": input.user_id,
            "amount": signed.to_string(), "balance_after": balance, "credit_unit": "USD"})).await?;
        tx.commit().await?;
        transaction_record(&ledger)
    }

    async fn record_credit_command_rejected(
        &self,
        workspace_id: Uuid,
        actor_plugin_id: &str,
        command: &str,
        reason: &str,
        idempotency_key: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"insert into credit_event_outbox
            (event_id,workspace_id,event_type,payload)
            values ($1,$2,'CreditCommandRejected',$3)"#,
        )
        .bind(Uuid::now_v7())
        .bind(workspace_id)
        .bind(serde_json::json!({
            "actor_plugin_id":actor_plugin_id,"command":command,"reason":reason,
            "idempotency_key":idempotency_key
        }))
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn reserve_credit(&self, input: &ReserveCreditInput) -> Result<CreditReservation> {
        let amount: rust_decimal::Decimal = input
            .amount
            .parse()
            .map_err(|_| ControlPlaneError::InvalidInput("amount"))?;
        if amount.is_sign_negative() {
            return Err(ControlPlaneError::InvalidInput("amount").into());
        }
        let mut tx = self.pool().begin().await?;
        let account = ensure_account(
            &mut tx,
            input.workspace_id,
            input.user_id,
            input.charge_enabled_default,
        )
        .await?;
        let key = format!("reserve:{}", input.provider_invocation_id);
        let effective_amount = if account.charge_enabled {
            amount
        } else {
            rust_decimal::Decimal::ZERO
        };
        if let Some(row) = sqlx::query("select id, account_id, user_id, flow_run_id, pricing_rule_id, reserved_amount::text as reserved_amount, metadata from billing_sessions where workspace_id=$1 and idempotency_key=$2")
            .bind(input.workspace_id).bind(&key).fetch_optional(&mut *tx).await? {
            let metadata: serde_json::Value = row.try_get("metadata")?;
            let existing_amount: rust_decimal::Decimal = row.try_get::<String, _>("reserved_amount")?.parse()?;
            if row.try_get::<Uuid, _>("user_id")? != input.user_id
                || row.try_get::<Option<Uuid>, _>("flow_run_id")? != input.flow_run_id
                || row.try_get::<Uuid, _>("pricing_rule_id")? != input.pricing_rule_id
                || existing_amount != effective_amount
            {
                return Err(ControlPlaneError::Conflict("credit_idempotency_payload_mismatch").into());
            }
            tx.commit().await?;
            return Ok(CreditReservation { billing_session_id: row.try_get("id")?, account_id: row.try_get("account_id")?,
                reserved_amount: existing_amount.to_string(), charge_skipped: metadata.get("charge_skipped").and_then(|v| v.as_bool()).unwrap_or(false),
                charge_skip_reason: metadata.get("charge_skip_reason").and_then(|v| v.as_str()).map(str::to_string) });
        }
        let balance: rust_decimal::Decimal = account.current_balance.parse()?;
        let reserved: rust_decimal::Decimal = account.reserved_amount.parse()?;
        if account.charge_enabled && balance - reserved < rust_decimal::Decimal::ZERO {
            return Err(ControlPlaneError::Conflict("credit_insufficient").into());
        }
        let session_id = Uuid::now_v7();
        let skip_reason = (!account.charge_enabled).then_some("charge_disabled");
        let metadata = serde_json::json!({"charge_skipped": !account.charge_enabled, "charge_skip_reason": skip_reason,
            "provider_invocation_id": input.provider_invocation_id});
        sqlx::query(
            r#"insert into billing_sessions
            (id,workspace_id,flow_run_id,idempotency_key,status,user_id,account_id,pricing_rule_id,
             reserved_amount,reservation_expires_at,last_heartbeat_at,metadata)
            values ($1,$2,$3,$4,'reserved',$5,$6,$7,$8::numeric,$9,now(),$10)"#,
        )
        .bind(session_id)
        .bind(input.workspace_id)
        .bind(input.flow_run_id)
        .bind(&key)
        .bind(input.user_id)
        .bind(account.id)
        .bind(input.pricing_rule_id)
        .bind(effective_amount.to_string())
        .bind(input.reservation_expires_at)
        .bind(&metadata)
        .execute(&mut *tx)
        .await?;
        let updated = sqlx::query("update user_credit_accounts set reserved_amount=reserved_amount+$2::numeric,revision=revision+1,updated_at=now() where id=$1 returning current_balance::text as balance,reserved_amount::text as reserved")
            .bind(account.id).bind(effective_amount.to_string()).fetch_one(&mut *tx).await?;
        let balance_after: String = updated.try_get("balance")?;
        let reserved_after: String = updated.try_get("reserved")?;
        let ledger_id = Uuid::now_v7();
        sqlx::query(r#"insert into runtime_credit_ledger
            (id,transaction_id,account_id,workspace_id,user_id,flow_run_id,billing_session_id,transaction_type,
             amount,balance_after,reserved_after,credit_unit,reason,idempotency_key,status,metadata)
            values ($1,$2,$3,$4,$5,$6,$7,'reserve',0,$8::numeric,$9::numeric,'USD','model_invocation',$10,'completed',$11)"#)
            .bind(ledger_id).bind(Uuid::now_v7()).bind(account.id).bind(input.workspace_id).bind(input.user_id)
            .bind(input.flow_run_id).bind(session_id).bind(balance_after).bind(reserved_after).bind(&key).bind(&metadata)
            .execute(&mut *tx).await?;
        sqlx::query("update billing_sessions set reserved_credit_ledger_id=$2 where id=$1")
            .bind(session_id)
            .bind(ledger_id)
            .execute(&mut *tx)
            .await?;
        insert_outbox(&mut tx,input.workspace_id,account.id,"CreditReserved",serde_json::json!({"billing_session_id":session_id,"amount":effective_amount.to_string()})).await?;
        tx.commit().await?;
        Ok(CreditReservation {
            billing_session_id: session_id,
            account_id: account.id,
            reserved_amount: effective_amount.to_string(),
            charge_skipped: !account.charge_enabled,
            charge_skip_reason: skip_reason.map(str::to_string),
        })
    }

    async fn settle_credit(&self, input: &SettleCreditInput) -> Result<CreditTransactionRecord> {
        let actual: rust_decimal::Decimal = input
            .actual_amount
            .parse()
            .map_err(|_| ControlPlaneError::InvalidInput("actual_amount"))?;
        if actual.is_sign_negative() {
            return Err(ControlPlaneError::InvalidInput("actual_amount").into());
        }
        let mut tx = self.pool().begin().await?;
        let session = sqlx::query("select workspace_id,user_id,account_id,reserved_amount::text as reserved_amount,actual_amount::text as actual_amount,status,metadata from billing_sessions where id=$1 for update")
            .bind(input.billing_session_id).fetch_one(&mut *tx).await?;
        let account_id: Uuid = session.try_get("account_id")?;
        if session.try_get::<String, _>("status")? == "settled" {
            let row = sqlx::query(
                &(CREDIT_LEDGER_SELECT.to_owned()
                    + " where billing_session_id=$1 and transaction_type='settle'"),
            )
            .bind(input.billing_session_id)
            .fetch_one(&mut *tx)
            .await?;
            let existing = transaction_record(&row)?;
            let existing_actual: rust_decimal::Decimal = session
                .try_get::<Option<String>, _>("actual_amount")?
                .ok_or_else(|| ControlPlaneError::Conflict("billing_session_settlement_invalid"))?
                .parse()?;
            if existing_actual != actual
                || existing.metadata.get("price_snapshot") != Some(&input.price_snapshot)
                || existing.metadata.get("usage_snapshot") != Some(&input.usage_snapshot)
            {
                return Err(
                    ControlPlaneError::Conflict("credit_idempotency_payload_mismatch").into(),
                );
            }
            tx.commit().await?;
            return Ok(existing);
        }
        if session.try_get::<String, _>("status")? != "reserved" {
            return Err(ControlPlaneError::Conflict("billing_session_not_reserved").into());
        }
        let metadata: serde_json::Value = session.try_get("metadata")?;
        let charge_skipped = metadata
            .get("charge_skipped")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let charged = if charge_skipped {
            rust_decimal::Decimal::ZERO
        } else {
            actual
        };
        let reserved: rust_decimal::Decimal =
            session.try_get::<String, _>("reserved_amount")?.parse()?;
        let account = sqlx::query("update user_credit_accounts set current_balance=current_balance-$2::numeric,reserved_amount=greatest(reserved_amount-$3::numeric,0),revision=revision+1,updated_at=now() where id=$1 returning workspace_id,user_id,current_balance::text as balance,reserved_amount::text as reserved")
            .bind(account_id).bind(charged.to_string()).bind(reserved.to_string()).fetch_one(&mut *tx).await?;
        let workspace_id: Uuid = account.try_get("workspace_id")?;
        let user_id: Uuid = account.try_get("user_id")?;
        let balance: String = account.try_get("balance")?;
        let reserved_after: String = account.try_get("reserved")?;
        let ledger_id = Uuid::now_v7();
        let key = format!("settle:{}", input.billing_session_id);
        let ledger=sqlx::query(&(format!(r#"insert into runtime_credit_ledger
            (id,transaction_id,account_id,workspace_id,user_id,billing_session_id,cost_ledger_id,transaction_type,
             amount,balance_after,reserved_after,credit_unit,reason,idempotency_key,status,metadata)
            values ($1,$2,$3,$4,$5,$6,$7,'settle',$8::numeric,$9::numeric,$10::numeric,'USD','model_token_usage',$11,'completed',$12)
            returning {}"#, CREDIT_LEDGER_COLUMNS)))
            .bind(ledger_id).bind(Uuid::now_v7()).bind(account_id).bind(workspace_id).bind(user_id).bind(input.billing_session_id)
            .bind(input.cost_ledger_id).bind((-charged).to_string()).bind(&balance).bind(&reserved_after).bind(&key)
            .bind(serde_json::json!({"price_snapshot":input.price_snapshot,"usage_snapshot":input.usage_snapshot,"charge_skipped":charge_skipped}))
            .fetch_one(&mut *tx).await?;
        sqlx::query("update billing_sessions set status='settled',actual_amount=$2::numeric,settled_credit_ledger_id=$3,updated_at=now() where id=$1")
            .bind(input.billing_session_id).bind(actual.to_string()).bind(ledger_id).execute(&mut *tx).await?;
        insert_outbox(&mut tx,workspace_id,account_id,"CreditSettled",serde_json::json!({"billing_session_id":input.billing_session_id,"actual_amount":actual.to_string(),"charged_amount":charged.to_string(),"balance_after":balance})).await?;
        tx.commit().await?;
        transaction_record(&ledger)
    }

    async fn release_credit(
        &self,
        billing_session_id: Uuid,
        reason: &str,
    ) -> Result<Option<CreditTransactionRecord>> {
        let mut tx = self.pool().begin().await?;
        let Some(session)=sqlx::query("select workspace_id,user_id,account_id,reserved_amount::text as reserved_amount,status from billing_sessions where id=$1 for update")
            .bind(billing_session_id).fetch_optional(&mut *tx).await? else { return Ok(None); };
        if session.try_get::<String, _>("status")? != "reserved" {
            tx.commit().await?;
            return Ok(None);
        }
        let account_id: Uuid = session.try_get("account_id")?;
        let reserved: String = session.try_get("reserved_amount")?;
        let account=sqlx::query("update user_credit_accounts set reserved_amount=greatest(reserved_amount-$2::numeric,0),revision=revision+1,updated_at=now() where id=$1 returning workspace_id,user_id,current_balance::text as balance,reserved_amount::text as reserved")
            .bind(account_id).bind(&reserved).fetch_one(&mut *tx).await?;
        let workspace_id: Uuid = account.try_get("workspace_id")?;
        let user_id: Uuid = account.try_get("user_id")?;
        let balance: String = account.try_get("balance")?;
        let reserved_after: String = account.try_get("reserved")?;
        let ledger_id = Uuid::now_v7();
        let key = format!("release:{billing_session_id}");
        let ledger=sqlx::query(&(format!(r#"insert into runtime_credit_ledger
            (id,transaction_id,account_id,workspace_id,user_id,billing_session_id,transaction_type,amount,balance_after,reserved_after,credit_unit,reason,idempotency_key,status)
            values ($1,$2,$3,$4,$5,$6,'release',0,$7::numeric,$8::numeric,'USD',$9,$10,'completed') returning {}"#,CREDIT_LEDGER_COLUMNS)))
            .bind(ledger_id).bind(Uuid::now_v7()).bind(account_id).bind(workspace_id).bind(user_id).bind(billing_session_id)
            .bind(&balance).bind(&reserved_after).bind(reason).bind(&key).fetch_one(&mut *tx).await?;
        sqlx::query("update billing_sessions set status='released',refund_credit_ledger_id=$2,updated_at=now() where id=$1")
            .bind(billing_session_id).bind(ledger_id).execute(&mut *tx).await?;
        insert_outbox(
            &mut tx,
            workspace_id,
            account_id,
            "CreditReleased",
            serde_json::json!({"billing_session_id":billing_session_id,"released_amount":reserved}),
        )
        .await?;
        tx.commit().await?;
        Ok(Some(transaction_record(&ledger)?))
    }

    async fn heartbeat_credit_reservation(
        &self,
        billing_session_id: Uuid,
        reservation_expires_at: OffsetDateTime,
    ) -> Result<bool> {
        Ok(sqlx::query(
            "update billing_sessions set last_heartbeat_at=now(),reservation_expires_at=$2,updated_at=now() where id=$1 and status='reserved'",
        )
        .bind(billing_session_id)
        .bind(reservation_expires_at)
        .execute(self.pool())
        .await?
        .rows_affected()
            == 1)
    }

    async fn list_credit_ledger(
        &self,
        input: &ListCreditLedgerInput,
    ) -> Result<Vec<CreditTransactionRecord>> {
        let mut query = QueryBuilder::<Postgres>::new(CREDIT_LEDGER_SELECT);
        query
            .push(" where workspace_id=")
            .push_bind(input.workspace_id);
        if let Some(user_id) = input.user_id {
            query.push(" and user_id=").push_bind(user_id);
        }
        if let (Some(at), Some(id)) = (input.before_created_at, input.before_id) {
            query
                .push(" and (created_at,id) < (")
                .push_bind(at)
                .push(",")
                .push_bind(id)
                .push(")");
        }
        query
            .push(" order by created_at desc,id desc limit ")
            .push_bind(input.limit.clamp(1, 200));
        let rows = query.build().fetch_all(self.pool()).await?;
        rows.iter().map(transaction_record).collect()
    }

    async fn claim_credit_outbox_events(
        &self,
        worker_id: &str,
        limit: i64,
        locked_until: OffsetDateTime,
    ) -> Result<Vec<CreditOutboxEvent>> {
        let rows = sqlx::query(
            r#"with candidates as (
                select event_id from credit_event_outbox
                where published_at is null and (locked_until is null or locked_until < now())
                order by created_at, event_id
                for update skip locked
                limit $1
            )
            update credit_event_outbox events
            set locked_by=$2, locked_until=$3, delivery_attempts=delivery_attempts+1
            from candidates where events.event_id=candidates.event_id
            returning events.event_id,events.workspace_id,events.account_id,events.event_type,
                      events.payload,events.created_at,events.delivery_attempts"#,
        )
        .bind(limit.clamp(1, 100))
        .bind(worker_id)
        .bind(locked_until)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(CreditOutboxEvent {
                    event_id: row.try_get("event_id")?,
                    workspace_id: row.try_get("workspace_id")?,
                    account_id: row.try_get("account_id")?,
                    event_type: row.try_get("event_type")?,
                    payload: row.try_get("payload")?,
                    created_at: row.try_get("created_at")?,
                    delivery_attempts: row.try_get("delivery_attempts")?,
                })
            })
            .collect()
    }

    async fn complete_credit_outbox_event(&self, event_id: Uuid, worker_id: &str) -> Result<bool> {
        Ok(sqlx::query("update credit_event_outbox set published_at=now(),locked_by=null,locked_until=null,last_error=null where event_id=$1 and locked_by=$2 and published_at is null")
            .bind(event_id).bind(worker_id).execute(self.pool()).await?.rows_affected()==1)
    }

    async fn fail_credit_outbox_event(
        &self,
        event_id: Uuid,
        worker_id: &str,
        error: &str,
    ) -> Result<bool> {
        Ok(sqlx::query("update credit_event_outbox set locked_by=null,locked_until=null,last_error=$3 where event_id=$1 and locked_by=$2 and published_at is null")
            .bind(event_id).bind(worker_id).bind(error).execute(self.pool()).await?.rows_affected()==1)
    }

    async fn recover_expired_credit_reservations(
        &self,
        now: OffsetDateTime,
        limit: i64,
    ) -> Result<usize> {
        let rows = sqlx::query(
            r#"select sessions.id, costs.id as cost_ledger_id,
                      costs.usage_ledger_id, costs.normalized_cost::text as normalized_cost,
                      costs.price_snapshot, usage.normalized_usage as usage_snapshot
               from billing_sessions sessions
               left join runtime_cost_ledger costs on costs.billing_session_id=sessions.id
               left join runtime_usage_ledger usage on usage.id=costs.usage_ledger_id
               where sessions.status='reserved' and sessions.reservation_expires_at < $1
               order by sessions.reservation_expires_at,sessions.id limit $2"#,
        )
        .bind(now)
        .bind(limit.clamp(1, 100))
        .fetch_all(self.pool())
        .await?;
        let mut recovered = 0;
        for row in rows {
            let billing_session_id: Uuid = row.try_get("id")?;
            let cost_ledger_id: Option<Uuid> = row.try_get("cost_ledger_id")?;
            if let Some(cost_ledger_id) = cost_ledger_id {
                let actual_amount = row
                    .try_get::<Option<String>, _>("normalized_cost")?
                    .ok_or(ControlPlaneError::Conflict("billing_cost_unrated"))?;
                self.settle_credit(&SettleCreditInput {
                    billing_session_id,
                    actual_amount,
                    cost_ledger_id: Some(cost_ledger_id),
                    usage_ledger_id: row.try_get("usage_ledger_id")?,
                    price_snapshot: row.try_get("price_snapshot")?,
                    usage_snapshot: row
                        .try_get::<Option<serde_json::Value>, _>("usage_snapshot")?
                        .unwrap_or_else(|| serde_json::json!({})),
                })
                .await?;
                recovered += 1;
            } else if self
                .release_credit(billing_session_id, "reservation_expired_without_cost")
                .await?
                .is_some()
            {
                recovered += 1;
            }
        }
        Ok(recovered)
    }
}
