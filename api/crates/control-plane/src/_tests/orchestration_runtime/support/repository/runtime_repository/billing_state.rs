use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn memory_append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> Result<domain::UsageLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::UsageLedgerRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            span_id: input.span_id,
            failover_attempt_id: input.failover_attempt_id,
            provider_instance_id: input.provider_instance_id,
            gateway_route_id: input.gateway_route_id,
            model_id: input.model_id.clone(),
            upstream_model_id: input.upstream_model_id.clone(),
            upstream_request_id: input.upstream_request_id.clone(),
            input_tokens: input.input_tokens,
            cached_input_tokens: input.cached_input_tokens,
            output_tokens: input.output_tokens,
            reasoning_output_tokens: input.reasoning_output_tokens,
            total_tokens: input.total_tokens,
            input_cache_hit_tokens: input.input_cache_hit_tokens,
            input_cache_miss_tokens: input.input_cache_miss_tokens,
            cache_read_tokens: input.cache_read_tokens,
            cache_write_tokens: input.cache_write_tokens,
            price_snapshot: input.price_snapshot.clone(),
            cost_snapshot: input.cost_snapshot.clone(),
            usage_status: input.usage_status,
            raw_usage: input.raw_usage.clone(),
            normalized_usage: input.normalized_usage.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .usage_ledger_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    pub(super) fn memory_append_cost_ledger(
        &self,
        input: &AppendCostLedgerInput,
    ) -> Result<domain::CostLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::CostLedgerRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            usage_ledger_id: input.usage_ledger_id,
            billing_session_id: input.billing_session_id,
            workspace_id: input.workspace_id,
            provider_instance_id: input.provider_instance_id,
            provider_account_id: input.provider_account_id,
            gateway_route_id: input.gateway_route_id,
            model_id: input.model_id.clone(),
            upstream_model_id: input.upstream_model_id.clone(),
            price_snapshot: input.price_snapshot.clone(),
            raw_cost: input.raw_cost.clone(),
            normalized_cost: input.normalized_cost.clone(),
            settlement_currency: input.settlement_currency.clone(),
            cost_source: input.cost_source.clone(),
            cost_status: input.cost_status.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        if let Some(flow_run_id) = record.flow_run_id {
            inner
                .cost_ledger_by_flow_run_id
                .entry(flow_run_id)
                .or_default()
                .push(record.clone());
        }
        Ok(record)
    }

    pub(super) fn memory_append_credit_ledger(
        &self,
        input: &AppendCreditLedgerInput,
    ) -> Result<domain::CreditLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner.credit_ledger_by_idempotency.get(&key) {
            return Ok(record.clone());
        }
        let record = domain::CreditLedgerRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            user_id: input.user_id,
            application_id: input.application_id,
            agent_id: input.agent_id,
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            cost_ledger_id: input.cost_ledger_id,
            transaction_type: input.transaction_type.clone(),
            amount: input.amount.clone(),
            balance_after: input.balance_after.clone(),
            credit_unit: input.credit_unit.clone(),
            reason: input.reason.clone(),
            idempotency_key: input.idempotency_key.clone(),
            status: input.status.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .credit_ledger_by_idempotency
            .insert(key, record.clone());
        Ok(record)
    }

    pub(super) fn memory_append_billing_session(
        &self,
        input: &AppendBillingSessionInput,
    ) -> Result<domain::BillingSessionRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner.billing_sessions_by_idempotency.get(&key) {
            return Ok(record.clone());
        }
        let now = OffsetDateTime::now_utc();
        let record = domain::BillingSessionRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            flow_run_id: input.flow_run_id,
            client_request_id: input.client_request_id.clone(),
            idempotency_key: input.idempotency_key.clone(),
            route_id: input.route_id,
            provider_account_id: input.provider_account_id,
            status: input.status,
            reserved_credit_ledger_id: input.reserved_credit_ledger_id,
            settled_credit_ledger_id: input.settled_credit_ledger_id,
            refund_credit_ledger_id: input.refund_credit_ledger_id,
            metadata: input.metadata.clone(),
            created_at: now,
            updated_at: now,
        };
        inner
            .billing_sessions_by_idempotency
            .insert(key, record.clone());
        Ok(record)
    }

    pub(super) fn memory_model_billing_enabled_at(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Option<OffsetDateTime>> {
        Ok(self.model_billing_enabled_at_value())
    }

    pub(super) fn memory_model_billing_match_pricing_rules(
        &self,
        provider_code: &str,
        upstream_model_id: &str,
        _at: OffsetDateTime,
    ) -> Result<Vec<crate::billing::PricingRule>> {
        if self.model_billing_enabled_at_value().is_none() {
            return Ok(Vec::new());
        }
        Ok(vec![test_model_pricing_rule(
            provider_code,
            upstream_model_id,
        )])
    }

    pub(super) fn memory_model_billing_reserve_credit(
        &self,
        input: &crate::ports::ReserveCreditInput,
    ) -> Result<crate::ports::CreditReservation> {
        self.record_model_billing_reservation();
        Ok(crate::ports::CreditReservation {
            billing_session_id: Uuid::now_v7(),
            account_id: input.user_id,
            reserved_amount: input.amount.clone(),
            charge_skipped: !input.charge_enabled_default,
            charge_skip_reason: (!input.charge_enabled_default)
                .then(|| "charge_disabled".to_string()),
        })
    }

    pub(super) fn memory_model_billing_release_credit(
        &self,
        billing_session_id: Uuid,
        reason: &str,
    ) -> Result<Option<crate::ports::CreditTransactionRecord>> {
        self.record_model_billing_release(billing_session_id, reason);
        Ok(None)
    }

    pub(super) fn memory_model_billing_heartbeat_credit_reservation(
        &self,
        _billing_session_id: Uuid,
        _reservation_expires_at: OffsetDateTime,
    ) -> Result<bool> {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .model_billing_heartbeat_count += 1;
        Ok(true)
    }

    pub(super) fn memory_append_audit_hash(
        &self,
        flow_run_id: Uuid,
        fact_table: &str,
        fact_id: Uuid,
        payload: serde_json::Value,
    ) -> Result<domain::AuditHashRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let hashes = inner
            .audit_hashes_by_flow_run_id
            .entry(flow_run_id)
            .or_default();
        let prev_hash = hashes.last().map(|record| record.row_hash.as_str());
        let record = domain::AuditHashRecord {
            id: Uuid::now_v7(),
            flow_run_id,
            fact_table: fact_table.to_string(),
            fact_id,
            prev_hash: prev_hash.map(ToString::to_string),
            row_hash: crate::runtime_observability::audit_row_hash(
                prev_hash, fact_table, fact_id, &payload,
            ),
            created_at: OffsetDateTime::now_utc(),
        };
        hashes.push(record.clone());
        Ok(record)
    }

    pub(super) fn memory_append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::ModelFailoverAttemptLedgerRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            llm_turn_span_id: input.llm_turn_span_id,
            queue_snapshot_id: input.queue_snapshot_id,
            attempt_index: input.attempt_index,
            provider_instance_id: input.provider_instance_id,
            provider_code: input.provider_code.clone(),
            upstream_model_id: input.upstream_model_id.clone(),
            protocol: input.protocol.clone(),
            request_ref: input.request_ref.clone(),
            request_hash: input.request_hash.clone(),
            started_at: input.started_at,
            first_token_at: input.first_token_at,
            finished_at: input.finished_at,
            status: input.status.clone(),
            failed_after_first_token: input.failed_after_first_token,
            upstream_request_id: input.upstream_request_id.clone(),
            error_code: input.error_code.clone(),
            error_message_ref: input.error_message_ref.clone(),
            usage_ledger_id: input.usage_ledger_id,
            cost_ledger_id: input.cost_ledger_id,
            response_ref: input.response_ref.clone(),
        };
        inner
            .model_failover_attempts_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    pub(super) fn memory_link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let attempt = inner
            .model_failover_attempts_by_flow_run_id
            .values_mut()
            .flat_map(|attempts| attempts.iter_mut())
            .find(|attempt| attempt.id == input.failover_attempt_id)
            .ok_or_else(|| anyhow::anyhow!("model failover attempt not found"))?;
        attempt.usage_ledger_id = Some(input.usage_ledger_id);
        Ok(attempt.clone())
    }

    pub(super) fn memory_append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> Result<domain::CapabilityInvocationRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::CapabilityInvocationRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            capability_id: input.capability_id.clone(),
            requested_by_span_id: input.requested_by_span_id,
            requester_kind: input.requester_kind.clone(),
            arguments_ref: input.arguments_ref.clone(),
            authorization_status: input.authorization_status.clone(),
            authorization_reason: input.authorization_reason.clone(),
            result_ref: input.result_ref.clone(),
            normalized_result: input.normalized_result.clone(),
            started_at: input.started_at,
            finished_at: input.finished_at,
            error_payload: input.error_payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .capability_invocations_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }
}
