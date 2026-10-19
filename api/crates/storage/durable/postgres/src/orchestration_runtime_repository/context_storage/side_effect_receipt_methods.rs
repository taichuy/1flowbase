impl PgControlPlaneStore {
    pub async fn get_data_model_side_effect_receipt(
        &self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<domain::DataModelSideEffectReceiptRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                application_id,
                draft_id,
                flow_run_id,
                node_run_id,
                node_id,
                action,
                model_code,
                record_id,
                deleted_id,
                affected_count,
                idempotency_key,
                payload_hash,
                actor_user_id,
                scope_id,
                status,
                output_payload,
                created_at
            from data_model_side_effect_receipts
            where workspace_id = $1 and idempotency_key = $2
            "#,
        )
        .bind(workspace_id)
        .bind(idempotency_key)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_data_model_side_effect_receipt_record))
    }

    pub async fn upsert_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<domain::DataModelSideEffectReceiptRecord> {
        let row = sqlx::query(
            r#"
            insert into data_model_side_effect_receipts (
                id,
                workspace_id,
                application_id,
                draft_id,
                flow_run_id,
                node_run_id,
                node_id,
                action,
                model_code,
                record_id,
                deleted_id,
                affected_count,
                idempotency_key,
                payload_hash,
                actor_user_id,
                scope_id,
                status,
                output_payload
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            on conflict (workspace_id, idempotency_key) do update
            set
                record_id = case
                    when data_model_side_effect_receipts.status = 'pending'
                    then excluded.record_id
                    else data_model_side_effect_receipts.record_id
                end,
                deleted_id = case
                    when data_model_side_effect_receipts.status = 'pending'
                    then excluded.deleted_id
                    else data_model_side_effect_receipts.deleted_id
                end,
                affected_count = case
                    when data_model_side_effect_receipts.status = 'pending'
                    then excluded.affected_count
                    else data_model_side_effect_receipts.affected_count
                end,
                status = case
                    when data_model_side_effect_receipts.status = 'pending'
                    then excluded.status
                    else data_model_side_effect_receipts.status
                end,
                output_payload = case
                    when data_model_side_effect_receipts.status = 'pending'
                    then excluded.output_payload
                    else data_model_side_effect_receipts.output_payload
                end
            returning
                id,
                workspace_id,
                application_id,
                draft_id,
                flow_run_id,
                node_run_id,
                node_id,
                action,
                model_code,
                record_id,
                deleted_id,
                affected_count,
                idempotency_key,
                payload_hash,
                actor_user_id,
                scope_id,
                status,
                output_payload,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.draft_id)
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.node_id.clone())
        .bind(input.action.clone())
        .bind(input.model_code.clone())
        .bind(input.record_id.clone())
        .bind(input.deleted_id.clone())
        .bind(input.affected_count)
        .bind(input.idempotency_key.clone())
        .bind(input.payload_hash.clone())
        .bind(input.actor_user_id)
        .bind(input.scope_id)
        .bind(input.status.clone())
        .bind(input.output_payload.clone())
        .fetch_one(self.pool())
        .await?;

        Ok(map_data_model_side_effect_receipt_record(row))
    }

    pub async fn claim_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<DataModelSideEffectReceiptClaim> {
        let row = sqlx::query(
            r#"
            insert into data_model_side_effect_receipts (
                id,
                workspace_id,
                application_id,
                draft_id,
                flow_run_id,
                node_run_id,
                node_id,
                action,
                model_code,
                record_id,
                deleted_id,
                affected_count,
                idempotency_key,
                payload_hash,
                actor_user_id,
                scope_id,
                status,
                output_payload
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, null, null, 0, $10, $11, $12, $13, 'pending', '{}'::jsonb)
            on conflict (workspace_id, idempotency_key) do nothing
            returning
                id,
                workspace_id,
                application_id,
                draft_id,
                flow_run_id,
                node_run_id,
                node_id,
                action,
                model_code,
                record_id,
                deleted_id,
                affected_count,
                idempotency_key,
                payload_hash,
                actor_user_id,
                scope_id,
                status,
                output_payload,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.draft_id)
        .bind(input.flow_run_id)
        .bind(input.node_run_id)
        .bind(input.node_id.clone())
        .bind(input.action.clone())
        .bind(input.model_code.clone())
        .bind(input.idempotency_key.clone())
        .bind(input.payload_hash.clone())
        .bind(input.actor_user_id)
        .bind(input.scope_id)
        .fetch_optional(self.pool())
        .await?;

        if let Some(row) = row {
            return Ok(DataModelSideEffectReceiptClaim {
                record: map_data_model_side_effect_receipt_record(row),
                claimed: true,
            });
        }

        let record = self
            .get_data_model_side_effect_receipt(input.workspace_id, &input.idempotency_key)
            .await?
            .ok_or_else(|| anyhow!("data_model side-effect receipt claim disappeared"))?;

        Ok(DataModelSideEffectReceiptClaim {
            record,
            claimed: false,
        })
    }
}

fn map_data_model_side_effect_receipt_record(
    row: sqlx::postgres::PgRow,
) -> domain::DataModelSideEffectReceiptRecord {
    domain::DataModelSideEffectReceiptRecord {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        application_id: row.get("application_id"),
        draft_id: row.get("draft_id"),
        flow_run_id: row.get("flow_run_id"),
        node_run_id: row.get("node_run_id"),
        node_id: row.get("node_id"),
        action: row.get("action"),
        model_code: row.get("model_code"),
        record_id: row.get("record_id"),
        deleted_id: row.get("deleted_id"),
        affected_count: row.get("affected_count"),
        idempotency_key: row.get("idempotency_key"),
        payload_hash: row.get("payload_hash"),
        actor_user_id: row.get("actor_user_id"),
        scope_id: row.get("scope_id"),
        status: row.get("status"),
        output_payload: row.get("output_payload"),
        created_at: row.get("created_at"),
    }
}
