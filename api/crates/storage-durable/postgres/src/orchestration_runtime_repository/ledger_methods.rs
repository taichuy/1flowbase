impl PgControlPlaneStore {
    async fn append_usage_ledger(
        &self,
        input: &AppendUsageLedgerInput,
    ) -> Result<domain::UsageLedgerRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_usage_ledger (
                id,
                flow_run_id,
                node_run_id,
                span_id,
                failover_attempt_id,
                provider_instance_id,
                gateway_route_id,
                model_id,
                upstream_model_id,
                upstream_request_id,
                input_tokens,
                cached_input_tokens,
                output_tokens,
                reasoning_output_tokens,
                total_tokens,
                input_cache_hit_tokens,
                input_cache_miss_tokens,
                cache_read_tokens,
                cache_write_tokens,
                price_snapshot,
                cost_snapshot,
                usage_status,
                raw_usage,
                normalized_usage
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                      $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                      $21, $22, $23, $24)
            returning
                id,
                flow_run_id,
                node_run_id,
                span_id,
                failover_attempt_id,
                provider_instance_id,
                gateway_route_id,
                model_id,
                upstream_model_id,
                upstream_request_id,
                input_tokens,
                cached_input_tokens,
                output_tokens,
                reasoning_output_tokens,
                total_tokens,
                input_cache_hit_tokens,
                input_cache_miss_tokens,
                cache_read_tokens,
                cache_write_tokens,
                price_snapshot,
                cost_snapshot,
                usage_status,
                raw_usage,
                normalized_usage,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.span_id)
        .bind(input.failover_attempt_id)
        .bind(input.provider_instance_id)
        .bind(input.gateway_route_id)
        .bind(input.model_id.as_deref())
        .bind(input.upstream_model_id.as_deref())
        .bind(input.upstream_request_id.as_deref())
        .bind(input.input_tokens)
        .bind(input.cached_input_tokens)
        .bind(input.output_tokens)
        .bind(input.reasoning_output_tokens)
        .bind(input.total_tokens)
        .bind(input.input_cache_hit_tokens)
        .bind(input.input_cache_miss_tokens)
        .bind(input.cache_read_tokens)
        .bind(input.cache_write_tokens)
        .bind(&input.price_snapshot)
        .bind(&input.cost_snapshot)
        .bind(input.usage_status.as_str())
        .bind(&input.raw_usage)
        .bind(&input.normalized_usage)
        .fetch_one(self.pool())
        .await?;

        map_usage_ledger_record(row)
    }

    async fn append_cost_ledger(
        &self,
        input: &AppendCostLedgerInput,
    ) -> Result<domain::CostLedgerRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_cost_ledger (
                id,
                flow_run_id,
                span_id,
                usage_ledger_id,
                billing_session_id,
                workspace_id,
                provider_instance_id,
                provider_account_id,
                gateway_route_id,
                model_id,
                upstream_model_id,
                price_snapshot,
                raw_cost,
                normalized_cost,
                settlement_currency,
                cost_source,
                cost_status
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                      $12, $13::numeric, $14::numeric, $15, $16, $17)
            returning
                id,
                flow_run_id,
                span_id,
                usage_ledger_id,
                billing_session_id,
                workspace_id,
                provider_instance_id,
                provider_account_id,
                gateway_route_id,
                model_id,
                upstream_model_id,
                price_snapshot,
                raw_cost::text as raw_cost,
                normalized_cost::text as normalized_cost,
                settlement_currency,
                cost_source,
                cost_status,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.span_id)
        .bind(input.usage_ledger_id)
        .bind(input.billing_session_id)
        .bind(input.workspace_id)
        .bind(input.provider_instance_id)
        .bind(input.provider_account_id)
        .bind(input.gateway_route_id)
        .bind(input.model_id.as_deref())
        .bind(input.upstream_model_id.as_deref())
        .bind(&input.price_snapshot)
        .bind(input.raw_cost.as_deref())
        .bind(input.normalized_cost.as_deref())
        .bind(input.settlement_currency.as_deref())
        .bind(&input.cost_source)
        .bind(&input.cost_status)
        .fetch_one(self.pool())
        .await?;

        Ok(map_cost_ledger_record(row))
    }

    async fn append_credit_ledger(
        &self,
        input: &AppendCreditLedgerInput,
    ) -> Result<domain::CreditLedgerRecord> {
        let row = sqlx::query(
            r#"
            insert into runtime_credit_ledger (
                id,
                workspace_id,
                user_id,
                application_id,
                agent_id,
                flow_run_id,
                span_id,
                cost_ledger_id,
                transaction_type,
                amount,
                balance_after,
                credit_unit,
                reason,
                idempotency_key,
                status
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::numeric,
                      $11::numeric, $12, $13, $14, $15)
            on conflict (workspace_id, idempotency_key) do update
            set idempotency_key = excluded.idempotency_key
            returning
                id,
                workspace_id,
                user_id,
                application_id,
                agent_id,
                flow_run_id,
                span_id,
                cost_ledger_id,
                transaction_type,
                amount::text as amount,
                balance_after::text as balance_after,
                credit_unit,
                reason,
                idempotency_key,
                status,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.user_id)
        .bind(input.application_id)
        .bind(input.agent_id)
        .bind(input.flow_run_id)
        .bind(input.span_id)
        .bind(input.cost_ledger_id)
        .bind(&input.transaction_type)
        .bind(&input.amount)
        .bind(input.balance_after.as_deref())
        .bind(&input.credit_unit)
        .bind(&input.reason)
        .bind(&input.idempotency_key)
        .bind(&input.status)
        .fetch_one(self.pool())
        .await?;

        Ok(map_credit_ledger_record(row))
    }

    async fn append_billing_session(
        &self,
        input: &AppendBillingSessionInput,
    ) -> Result<domain::BillingSessionRecord> {
        let row = sqlx::query(
            r#"
            insert into billing_sessions (
                id,
                workspace_id,
                flow_run_id,
                client_request_id,
                idempotency_key,
                route_id,
                provider_account_id,
                status,
                reserved_credit_ledger_id,
                settled_credit_ledger_id,
                refund_credit_ledger_id,
                metadata
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            on conflict (workspace_id, idempotency_key) do update
            set idempotency_key = excluded.idempotency_key
            returning
                id,
                workspace_id,
                flow_run_id,
                client_request_id,
                idempotency_key,
                route_id,
                provider_account_id,
                status,
                reserved_credit_ledger_id,
                settled_credit_ledger_id,
                refund_credit_ledger_id,
                metadata,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.flow_run_id)
        .bind(&input.client_request_id)
        .bind(&input.idempotency_key)
        .bind(input.route_id)
        .bind(input.provider_account_id)
        .bind(input.status.as_str())
        .bind(input.reserved_credit_ledger_id)
        .bind(input.settled_credit_ledger_id)
        .bind(input.refund_credit_ledger_id)
        .bind(&input.metadata)
        .fetch_one(self.pool())
        .await?;

        map_billing_session_record(row)
    }

    async fn append_audit_hash(
        &self,
        flow_run_id: Uuid,
        fact_table: &str,
        fact_id: Uuid,
        payload: serde_json::Value,
    ) -> Result<domain::AuditHashRecord> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("lock table runtime_audit_hashes in share row exclusive mode")
            .execute(&mut *tx)
            .await?;

        let prev_hash = sqlx::query_scalar::<_, String>(
            r#"
            select row_hash
            from runtime_audit_hashes
            where flow_run_id = $1
            order by created_at desc, id desc
            limit 1
            "#,
        )
        .bind(flow_run_id)
        .fetch_optional(&mut *tx)
        .await?;
        let row_hash = control_plane::runtime_observability::audit_row_hash(
            prev_hash.as_deref(),
            fact_table,
            fact_id,
            &payload,
        );
        let row = sqlx::query(
            r#"
            insert into runtime_audit_hashes (
                id,
                flow_run_id,
                fact_table,
                fact_id,
                prev_hash,
                row_hash
            ) values ($1, $2, $3, $4, $5, $6)
            returning
                id,
                flow_run_id,
                fact_table,
                fact_id,
                prev_hash,
                row_hash,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(flow_run_id)
        .bind(fact_table)
        .bind(fact_id)
        .bind(prev_hash.as_deref())
        .bind(&row_hash)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(map_audit_hash_record(row))
    }

    async fn append_model_failover_attempt_ledger(
        &self,
        input: &AppendModelFailoverAttemptLedgerInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        let row = sqlx::query(
            r#"
            insert into model_failover_attempt_ledger (
                id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                queue_snapshot_id,
                attempt_index,
                provider_instance_id,
                provider_code,
                upstream_model_id,
                protocol,
                request_ref,
                request_hash,
                started_at,
                first_token_at,
                finished_at,
                status,
                failed_after_first_token,
                upstream_request_id,
                error_code,
                error_message_ref,
                usage_ledger_id,
                cost_ledger_id,
                response_ref
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                      $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                      $21, $22, $23)
            returning
                id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                queue_snapshot_id,
                attempt_index,
                provider_instance_id,
                provider_code,
                upstream_model_id,
                protocol,
                request_ref,
                request_hash,
                started_at,
                first_token_at,
                finished_at,
                status,
                failed_after_first_token,
                upstream_request_id,
                error_code,
                error_message_ref,
                usage_ledger_id,
                cost_ledger_id,
                response_ref
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.llm_turn_span_id)
        .bind(input.queue_snapshot_id)
        .bind(input.attempt_index)
        .bind(input.provider_instance_id)
        .bind(&input.provider_code)
        .bind(&input.upstream_model_id)
        .bind(&input.protocol)
        .bind(input.request_ref.as_deref())
        .bind(input.request_hash.as_deref())
        .bind(input.started_at)
        .bind(input.first_token_at)
        .bind(input.finished_at)
        .bind(&input.status)
        .bind(input.failed_after_first_token)
        .bind(input.upstream_request_id.as_deref())
        .bind(input.error_code.as_deref())
        .bind(input.error_message_ref.as_deref())
        .bind(input.usage_ledger_id)
        .bind(input.cost_ledger_id)
        .bind(input.response_ref.as_deref())
        .fetch_one(self.pool())
        .await?;

        Ok(map_model_failover_attempt_ledger_record(row))
    }

    async fn link_usage_ledger_to_model_failover_attempt(
        &self,
        input: &LinkUsageLedgerToModelFailoverAttemptInput,
    ) -> Result<domain::ModelFailoverAttemptLedgerRecord> {
        let row = sqlx::query(
            r#"
            update model_failover_attempt_ledger
            set usage_ledger_id = $2
            where id = $1
            returning
                id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                queue_snapshot_id,
                attempt_index,
                provider_instance_id,
                provider_code,
                upstream_model_id,
                protocol,
                request_ref,
                request_hash,
                started_at,
                first_token_at,
                finished_at,
                status,
                failed_after_first_token,
                upstream_request_id,
                error_code,
                error_message_ref,
                usage_ledger_id,
                cost_ledger_id,
                response_ref
            "#,
        )
        .bind(input.failover_attempt_id)
        .bind(input.usage_ledger_id)
        .fetch_one(self.pool())
        .await?;

        Ok(map_model_failover_attempt_ledger_record(row))
    }

    async fn append_capability_invocation(
        &self,
        input: &AppendCapabilityInvocationInput,
    ) -> Result<domain::CapabilityInvocationRecord> {
        let row = sqlx::query(
            r#"
            insert into capability_invocations (
                id,
                flow_run_id,
                span_id,
                capability_id,
                requested_by_span_id,
                requester_kind,
                arguments_ref,
                authorization_status,
                authorization_reason,
                result_ref,
                normalized_result,
                started_at,
                finished_at,
                error_payload
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            returning
                id,
                flow_run_id,
                span_id,
                capability_id,
                requested_by_span_id,
                requester_kind,
                arguments_ref,
                authorization_status,
                authorization_reason,
                result_ref,
                normalized_result,
                started_at,
                finished_at,
                error_payload,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.flow_run_id)
        .bind(input.span_id)
        .bind(&input.capability_id)
        .bind(input.requested_by_span_id)
        .bind(&input.requester_kind)
        .bind(input.arguments_ref.as_deref())
        .bind(&input.authorization_status)
        .bind(input.authorization_reason.as_deref())
        .bind(input.result_ref.as_deref())
        .bind(&input.normalized_result)
        .bind(input.started_at)
        .bind(input.finished_at)
        .bind(&input.error_payload)
        .fetch_one(self.pool())
        .await?;

        Ok(map_capability_invocation_record(row))
    }


}
