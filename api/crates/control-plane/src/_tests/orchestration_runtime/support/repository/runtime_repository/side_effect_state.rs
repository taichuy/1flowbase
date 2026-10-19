use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn memory_upsert_debug_variable_cache_entry(
        &self,
        input: &UpsertDebugVariableCacheEntryInput,
    ) -> Result<DebugVariableCacheEntry> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let entry = DebugVariableCacheEntry {
            node_id: input.node_id.clone(),
            variable_key: input.variable_key.clone(),
            value: input.value.clone(),
        };
        inner.debug_variable_cache_entries_by_key.insert(
            (
                input.application_id,
                input.draft_id,
                input.actor_user_id,
                input.node_id.clone(),
                input.variable_key.clone(),
            ),
            entry.clone(),
        );
        Ok(entry)
    }

    pub(super) fn memory_list_debug_variable_cache_entries(
        &self,
        application_id: Uuid,
        draft_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<Vec<DebugVariableCacheEntry>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .debug_variable_cache_entries_by_key
            .iter()
            .filter(
                |((cached_application_id, cached_draft_id, cached_actor_user_id, _, _), _)| {
                    *cached_application_id == application_id
                        && *cached_draft_id == draft_id
                        && *cached_actor_user_id == actor_user_id
                },
            )
            .map(|(_, entry)| entry.clone())
            .collect())
    }

    pub(super) fn memory_delete_debug_variable_cache_entries(
        &self,
        input: &DeleteDebugVariableCacheEntriesInput,
    ) -> Result<()> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        match &input.keys {
            Some(keys) => {
                for key in keys {
                    inner.debug_variable_cache_entries_by_key.remove(&(
                        input.application_id,
                        input.draft_id,
                        input.actor_user_id,
                        key.node_id.clone(),
                        key.variable_key.clone(),
                    ));
                }
            }
            None => {
                inner.debug_variable_cache_entries_by_key.retain(
                    |(application_id, draft_id, actor_user_id, _, _), _| {
                        *application_id != input.application_id
                            || *draft_id != input.draft_id
                            || *actor_user_id != input.actor_user_id
                    },
                );
            }
        }
        Ok(())
    }

    pub(super) fn memory_get_data_model_side_effect_receipt(
        &self,
        workspace_id: Uuid,
        idempotency_key: &str,
    ) -> Result<Option<domain::DataModelSideEffectReceiptRecord>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        Ok(inner
            .data_model_side_effect_receipts_by_idempotency
            .get(&(workspace_id, idempotency_key.to_string()))
            .cloned())
    }

    pub(super) fn memory_claim_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<DataModelSideEffectReceiptClaim> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner
            .data_model_side_effect_receipts_by_idempotency
            .get(&key)
        {
            return Ok(DataModelSideEffectReceiptClaim {
                record: record.clone(),
                claimed: false,
            });
        }

        let record = domain::DataModelSideEffectReceiptRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            draft_id: input.draft_id,
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            node_id: input.node_id.clone(),
            action: input.action.clone(),
            model_code: input.model_code.clone(),
            record_id: None,
            deleted_id: None,
            affected_count: 0,
            idempotency_key: input.idempotency_key.clone(),
            payload_hash: input.payload_hash.clone(),
            actor_user_id: input.actor_user_id,
            scope_id: input.scope_id,
            status: "pending".to_string(),
            output_payload: json!({}),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .data_model_side_effect_receipts_by_idempotency
            .insert(key, record.clone());

        Ok(DataModelSideEffectReceiptClaim {
            record,
            claimed: true,
        })
    }

    pub(super) fn memory_upsert_data_model_side_effect_receipt(
        &self,
        input: &UpsertDataModelSideEffectReceiptInput,
    ) -> Result<domain::DataModelSideEffectReceiptRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let key = (input.workspace_id, input.idempotency_key.clone());
        if let Some(record) = inner
            .data_model_side_effect_receipts_by_idempotency
            .get(&key)
        {
            if record.status != "pending" {
                return Ok(record.clone());
            }
        }

        let record = domain::DataModelSideEffectReceiptRecord {
            id: inner
                .data_model_side_effect_receipts_by_idempotency
                .get(&key)
                .map(|record| record.id)
                .unwrap_or_else(Uuid::now_v7),
            workspace_id: input.workspace_id,
            application_id: input.application_id,
            draft_id: input.draft_id,
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            node_id: input.node_id.clone(),
            action: input.action.clone(),
            model_code: input.model_code.clone(),
            record_id: input.record_id.clone(),
            deleted_id: input.deleted_id.clone(),
            affected_count: input.affected_count,
            idempotency_key: input.idempotency_key.clone(),
            payload_hash: input.payload_hash.clone(),
            actor_user_id: input.actor_user_id,
            scope_id: input.scope_id,
            status: input.status.clone(),
            output_payload: input.output_payload.clone(),
            created_at: inner
                .data_model_side_effect_receipts_by_idempotency
                .get(&key)
                .map(|record| record.created_at)
                .unwrap_or_else(OffsetDateTime::now_utc),
        };
        inner
            .data_model_side_effect_receipts_by_idempotency
            .insert(key, record.clone());

        Ok(record)
    }
}
