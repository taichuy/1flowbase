impl PgControlPlaneStore {
    async fn claim_runtime_event_deliveries(
        &self,
        input: &ClaimRuntimeEventDeliveriesInput,
    ) -> Result<RuntimeEventDeliveryBatch> {
        if input.lease_seconds <= 0 {
            return Err(anyhow!("runtime event delivery lease must be positive"));
        }
        let (kind, callback) = match input.scope {
            RuntimeEventDeliveryScope::Start => ("start", None),
            RuntimeEventDeliveryScope::Resume { callback_task_id } => {
                ("resume", Some(callback_task_id))
            }
            RuntimeEventDeliveryScope::Unresolved => {
                return Err(
                    ControlPlaneError::Conflict("runtime_event_delivery_scope_unresolved").into(),
                );
            }
        };
        let limit = i64::try_from(input.limit).unwrap_or(i64::MAX);
        // Recovery order is the original business transaction order. Diagnostic waiting
        // copies and stream markers cannot move either boundary. Every claim resolves
        // the next wait again: an absent upper boundary is never cached as open.
        // include_membership=false gates the entire eligible CTE, not just Rust
        // insertion. The independent claimed UNION still returns up to the existing
        // limit, and original JSON is restored only for the resulting fact IDs.
        let rows = sqlx::query(
            r#"
            with anchor as (
                select 0::bigint as event_sequence, -1::bigint as recovery_sequence
                 where $4::text = 'start'
                union all
                select h.event_sequence, h.sequence
                  from flow_run_recovery_history h
                 where $4::text = 'resume' and h.flow_run_id = $1
                   and h.idempotency_key = 'waiting_callback:' || $5::uuid::text
                   and h.state_code = 'waiting_callback'
            ), bounds as (
                select a.event_sequence as lower_sequence, a.recovery_sequence as lower_recovery_sequence,
                       n.event_sequence as upper_sequence, n.sequence as upper_recovery_sequence, n.id as upper_recovery_id
                  from anchor a
                  left join lateral (
                      select h.id, h.sequence, h.event_sequence
                        from flow_run_recovery_history h
                       where h.flow_run_id = $1 and h.sequence > a.recovery_sequence
                         and h.state_code in ('waiting_callback', 'waiting_human')
                       order by h.sequence limit 1
                  ) n on true
            ), eligible as materialized (
                select e.id from runtime_events e cross join bounds b
                 where $6::boolean and e.flow_run_id = $1 and e.sequence > b.lower_sequence
                   and (b.upper_recovery_sequence is null or e.sequence < b.upper_sequence)
                   and e.delivery_status in ('pending', 'claimed', 'acked', 'uncertain')
            ), candidates as (
                select e.id from runtime_events e cross join bounds b
                 where e.flow_run_id = $1 and e.sequence > b.lower_sequence
                   and (b.upper_recovery_sequence is null or e.sequence < b.upper_sequence)
                   and (e.delivery_status = 'pending'
                        or (e.delivery_status = 'claimed' and e.delivery_lease_expires_at <= now()))
                 order by e.sequence, e.id
                 for update of e skip locked limit $2
            ), claimed as (
                update runtime_events e
                   set delivery_status = 'claimed', delivery_claim_token = gen_random_uuid(),
                       delivery_generation = e.delivery_generation + 1,
                       delivery_lease_expires_at = now() + $3::bigint * interval '1 second',
                       delivery_acked_at = null
                  from candidates c where e.id = c.id
                returning e.id, e.delivery_claim_token, e.delivery_generation, e.delivery_lease_expires_at
            ), facts as (
                select id, true as proof_fact from eligible
                union all
                select id, false as proof_fact from claimed where not $6::boolean
            )
            select b.lower_sequence, b.lower_recovery_sequence, b.upper_sequence,
                   b.upper_recovery_sequence, b.upper_recovery_id, f.proof_fact,
                   e.id, e.flow_run_id, e.node_run_id, e.span_id, e.parent_span_id, e.sequence,
                   e.event_type, e.layer, e.source, e.trust_level, e.item_id, e.ledger_ref,
                   runtime_original_json(e.payload, e.raw_json_payloads, 'payload') as payload,
                   e.visibility, e.durability, e.created_at,
                   c.delivery_claim_token, c.delivery_generation, c.delivery_lease_expires_at
              from (select 1) root left join bounds b on true
              left join facts f on true left join runtime_events e on e.id = f.id
              left join claimed c on c.id = e.id
             order by e.sequence, e.id
            "#,
        )
        .bind(input.flow_run_id)
        .bind(limit)
        .bind(input.lease_seconds)
        .bind(kind)
        .bind(callback)
        .bind(input.include_membership)
        .fetch_all(self.pool())
        .await?;
        let mut batch = RuntimeEventDeliveryBatch {
            claims: Vec::new(),
            events: Vec::new(),
            closed_proof: None,
        };
        for row in rows {
            // Legacy identity without an original anchor cannot authorize executable
            // replay. Keep supported anchored legacy recovery, never infer Start.
            if row.get::<Option<i64>, _>("lower_sequence").is_none()
                || (row
                    .get::<Option<i64>, _>("upper_recovery_sequence")
                    .is_some()
                    && row.get::<Option<i64>, _>("upper_sequence").is_none())
            {
                return Err(
                    ControlPlaneError::Conflict("runtime_event_delivery_scope_unresolved").into(),
                );
            }
            if let Some(upper_id) = row.get::<Option<Uuid>, _>("upper_recovery_id") {
                batch.closed_proof = Some(RuntimeEventDeliveryClosedProof {
                    flow_run_id: input.flow_run_id,
                    scope: input.scope,
                    lower_sequence: row.get("lower_sequence"),
                    lower_recovery_sequence: row.get("lower_recovery_sequence"),
                    upper: RuntimeEventDeliveryBoundary {
                        recovery_id: upper_id,
                        recovery_sequence: row.get("upper_recovery_sequence"),
                        event_sequence: row.get("upper_sequence"),
                    },
                });
            }
            if row.get::<Option<Uuid>, _>("id").is_none() {
                continue;
            }
            let claim_token: Option<Uuid> = row.get("delivery_claim_token");
            let generation: Option<i64> = row.get("delivery_generation");
            let lease_expires_at: Option<time::OffsetDateTime> =
                row.get("delivery_lease_expires_at");
            let proof_fact = row.get::<Option<bool>, _>("proof_fact") == Some(true);
            let event = map_runtime_event_record(row)?;
            if let (Some(claim_token), Some(generation), Some(lease_expires_at)) =
                (claim_token, generation, lease_expires_at)
            {
                batch.claims.push(RuntimeEventDeliveryClaim {
                    event: event.clone(),
                    claim_token,
                    generation,
                    lease_expires_at,
                });
            }
            if proof_fact {
                batch.events.push(event);
            }
        }
        Ok(batch)
    }

    async fn ack_runtime_event_delivery(
        &self,
        input: &AckRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        let row = sqlx::query(
            r#"
            update runtime_events
               set delivery_status = 'acked', delivery_acked_at = $4
             where id = $1
               and delivery_status = 'claimed'
               and delivery_claim_token = $2
               and delivery_generation = $3
            returning id, flow_run_id, node_run_id, span_id, parent_span_id, sequence,
                      event_type, layer, source, trust_level, item_id, ledger_ref, runtime_original_json(payload, runtime_events.raw_json_payloads, 'payload') as payload,
                      visibility, durability, created_at
            "#,
        )
        .bind(input.event_id)
        .bind(input.claim_token)
        .bind(input.expected_generation)
        .bind(input.acknowledged_at)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::Conflict("runtime_event_delivery_claim_not_owned"))?;
        map_runtime_event_record(row)
    }

    async fn release_runtime_event_delivery(
        &self,
        input: &ReleaseRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        let row = sqlx::query(
            r#"
            update runtime_events
               set delivery_status = 'pending', delivery_claim_token = null,
                   delivery_lease_expires_at = null
             where id = $1
               and delivery_status = 'claimed'
               and delivery_claim_token = $2
               and delivery_generation = $3
            returning id, flow_run_id, node_run_id, span_id, parent_span_id, sequence,
                      event_type, layer, source, trust_level, item_id, ledger_ref, runtime_original_json(payload, runtime_events.raw_json_payloads, 'payload') as payload,
                      visibility, durability, created_at
            "#,
        )
        .bind(input.event_id)
        .bind(input.claim_token)
        .bind(input.expected_generation)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::Conflict("runtime_event_delivery_claim_not_owned"))?;
        map_runtime_event_record(row)
    }

    async fn mark_runtime_event_delivery_uncertain(
        &self,
        input: &MarkRuntimeEventDeliveryUncertainInput,
    ) -> Result<domain::RuntimeEventRecord> {
        let row = sqlx::query(
            r#"
            update runtime_events
               set delivery_status = 'uncertain', delivery_uncertain_at = $4
             where id = $1
               and delivery_status = 'claimed'
               and delivery_claim_token = $2
               and delivery_generation = $3
            returning id, flow_run_id, node_run_id, span_id, parent_span_id, sequence,
                      event_type, layer, source, trust_level, item_id, ledger_ref, runtime_original_json(payload, runtime_events.raw_json_payloads, 'payload') as payload,
                      visibility, durability, created_at
            "#,
        )
        .bind(input.event_id)
        .bind(input.claim_token)
        .bind(input.expected_generation)
        .bind(input.marked_at)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::Conflict("runtime_event_delivery_claim_not_owned"))?;
        map_runtime_event_record(row)
    }
}
