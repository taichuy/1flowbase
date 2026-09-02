use super::*;

#[async_trait]
impl crate::ports::BillingRepository for InMemoryOrchestrationRuntimeRepository {
    async fn list_pricing_rules(
        &self,
        _input: &crate::ports::ListPricingRulesInput,
    ) -> Result<crate::ports::PricingRulesPage> {
        Ok(crate::ports::PricingRulesPage {
            items: Vec::new(),
            total_count: 0,
        })
    }

    async fn get_pricing_rule(&self, _id: Uuid) -> Result<Option<crate::billing::PricingRule>> {
        Ok(None)
    }

    async fn match_pricing_rules(
        &self,
        _provider_code: &str,
        _upstream_model_id: &str,
        _at: OffsetDateTime,
    ) -> Result<Vec<crate::billing::PricingRule>> {
        Ok(Vec::new())
    }

    async fn upsert_pricing_rule(
        &self,
        _input: &crate::ports::UpsertPricingRuleInput,
    ) -> Result<crate::billing::PricingRule> {
        anyhow::bail!("pricing rule fixture is not implemented")
    }

    async fn insert_pricing_rule_if_absent(
        &self,
        _input: &crate::ports::UpsertPricingRuleInput,
    ) -> Result<Option<crate::billing::PricingRule>> {
        anyhow::bail!("pricing rule fixture is not implemented")
    }

    async fn delete_pricing_rule(&self, _id: Uuid) -> Result<bool> {
        Ok(false)
    }

    async fn billing_enabled_at(&self, _workspace_id: Uuid) -> Result<Option<OffsetDateTime>> {
        Ok(self.model_billing_enabled_at_value())
    }

    async fn list_credit_accounts(
        &self,
        _workspace_id: Uuid,
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<crate::ports::CreditAccountRecord>> {
        Ok(Vec::new())
    }

    async fn get_credit_account(
        &self,
        _workspace_id: Uuid,
        _user_id: Uuid,
    ) -> Result<Option<crate::ports::CreditAccountRecord>> {
        Ok(None)
    }

    async fn credit_target_is_root(&self, _user_id: Uuid) -> Result<bool> {
        Ok(false)
    }

    async fn billing_session_scope(
        &self,
        _billing_session_id: Uuid,
    ) -> Result<Option<(Uuid, Uuid)>> {
        Ok(None)
    }

    async fn execute_credit_command(
        &self,
        input: &crate::ports::CreditCommandInput,
    ) -> Result<crate::ports::CreditTransactionRecord> {
        let key = (input.workspace_id, input.idempotency_key.clone());
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        if let Some(existing) = inner.plugin_credit_transactions_by_idempotency.get(&key) {
            return Ok(existing.clone());
        }
        let now = OffsetDateTime::now_utc();
        let transaction = crate::ports::CreditTransactionRecord {
            id: Uuid::now_v7(),
            transaction_id: Uuid::now_v7(),
            account_id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            user_id: input.user_id,
            billing_session_id: None,
            actor_user_id: input.actor_user_id,
            actor_plugin_id: input.actor_plugin_id.clone(),
            transaction_type: input.command.clone(),
            amount: input.amount.clone(),
            balance_after: input.amount.clone(),
            reserved_after: "0".to_string(),
            credit_unit: input.credit_unit.clone(),
            reason: input.reason.clone(),
            source_type: input.source_type.clone(),
            source_id: input.source_id.clone(),
            idempotency_key: input.idempotency_key.clone(),
            status: "posted".to_string(),
            metadata: input.metadata.clone(),
            created_at: now,
        };
        inner
            .plugin_credit_transactions_by_idempotency
            .insert(key, transaction.clone());
        Ok(transaction)
    }

    async fn record_credit_command_rejected(
        &self,
        workspace_id: Uuid,
        actor_plugin_id: &str,
        command: &str,
        reason: &str,
        idempotency_key: &str,
    ) -> Result<()> {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .plugin_credit_rejections
            .push((
                workspace_id,
                actor_plugin_id.to_string(),
                command.to_string(),
                reason.to_string(),
                idempotency_key.to_string(),
            ));
        Ok(())
    }

    async fn reserve_credit(
        &self,
        _input: &crate::ports::ReserveCreditInput,
    ) -> Result<crate::ports::CreditReservation> {
        anyhow::bail!("credit reservation fixture is not implemented")
    }

    async fn settle_credit(
        &self,
        _input: &crate::ports::SettleCreditInput,
    ) -> Result<crate::ports::CreditTransactionRecord> {
        anyhow::bail!("credit settlement fixture is not implemented")
    }

    async fn release_credit(
        &self,
        _billing_session_id: Uuid,
        _reason: &str,
    ) -> Result<Option<crate::ports::CreditTransactionRecord>> {
        Ok(None)
    }

    async fn heartbeat_credit_reservation(
        &self,
        _billing_session_id: Uuid,
        _reservation_expires_at: OffsetDateTime,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn list_credit_ledger(
        &self,
        _input: &crate::ports::ListCreditLedgerInput,
    ) -> Result<Vec<crate::ports::CreditTransactionRecord>> {
        Ok(Vec::new())
    }

    async fn claim_credit_outbox_events(
        &self,
        _worker_id: &str,
        _limit: i64,
        _locked_until: OffsetDateTime,
    ) -> Result<Vec<crate::ports::CreditOutboxEvent>> {
        Ok(Vec::new())
    }

    async fn complete_credit_outbox_event(
        &self,
        _event_id: Uuid,
        _worker_id: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn fail_credit_outbox_event(
        &self,
        _event_id: Uuid,
        _worker_id: &str,
        _error: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    async fn recover_expired_credit_reservations(
        &self,
        _now: OffsetDateTime,
        _limit: i64,
    ) -> Result<usize> {
        Ok(0)
    }
}
