impl PgControlPlaneStore {
    async fn claim_runtime_event_deliveries(
        &self,
        input: &ClaimRuntimeEventDeliveriesInput,
    ) -> Result<Vec<RuntimeEventDeliveryClaim>> {
        if input.limit == 0 {
            return Ok(Vec::new());
        }
        if input.lease_seconds <= 0 {
            return Err(anyhow!("runtime event delivery lease must be positive"));
        }
        let limit = i64::try_from(input.limit).unwrap_or(i64::MAX);
        let rows = sqlx::query(
            r#"
            with candidates as (
                select id
                  from runtime_events
                 where flow_run_id = $1
                   and (
                       delivery_status = 'pending'
                       or (delivery_status = 'claimed' and delivery_lease_expires_at <= now())
                   )
                 order by sequence asc, id asc
                 for update skip locked
                 limit $2
            )
            update runtime_events event
               set delivery_status = 'claimed',
                   delivery_claim_token = gen_random_uuid(),
                   delivery_generation = delivery_generation + 1,
                   delivery_lease_expires_at = now() + $3::bigint * interval '1 second',
                   delivery_acked_at = null
              from candidates
             where event.id = candidates.id
            returning event.id, event.flow_run_id, event.node_run_id, event.span_id,
                      event.parent_span_id, event.sequence, event.event_type, event.layer,
                      event.source, event.trust_level, event.item_id, event.ledger_ref,
                      event.payload, event.visibility, event.durability, event.created_at,
                      event.delivery_claim_token, event.delivery_generation,
                      event.delivery_lease_expires_at
            "#,
        )
        .bind(input.flow_run_id)
        .bind(limit)
        .bind(input.lease_seconds)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                let claim_token = row.get("delivery_claim_token");
                let generation = row.get("delivery_generation");
                let lease_expires_at = row.get("delivery_lease_expires_at");
                Ok(RuntimeEventDeliveryClaim {
                    event: map_runtime_event_record(row)?,
                    claim_token,
                    generation,
                    lease_expires_at,
                })
            })
            .collect()
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
                      event_type, layer, source, trust_level, item_id, ledger_ref, payload,
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
                      event_type, layer, source, trust_level, item_id, ledger_ref, payload,
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
}
