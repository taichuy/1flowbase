use super::*;

impl InMemoryOrchestrationRuntimeRepository {
    pub(super) fn memory_append_run_event(
        &self,
        input: &AppendRunEventInput,
    ) -> Result<domain::RunEventRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let events = inner
            .events_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default();
        let event = domain::RunEventRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            sequence: (events.len() + 1) as i64,
            event_type: input.event_type.clone(),
            payload: input.payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        events.push(event.clone());
        Ok(event)
    }

    pub(super) fn memory_append_runtime_span(
        &self,
        input: &AppendRuntimeSpanInput,
    ) -> Result<domain::RuntimeSpanRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let span = domain::RuntimeSpanRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            parent_span_id: input.parent_span_id,
            kind: input.kind,
            name: input.name.clone(),
            status: input.status,
            capability_id: input.capability_id.clone(),
            input_ref: input.input_ref.clone(),
            output_ref: input.output_ref.clone(),
            error_payload: input.error_payload.clone(),
            metadata: input.metadata.clone(),
            started_at: input.started_at,
            finished_at: input.finished_at,
        };
        inner
            .runtime_spans_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(span.clone());
        Ok(span)
    }

    pub(super) fn memory_claim_runtime_event_deliveries(
        &self,
        input: &crate::ports::ClaimRuntimeEventDeliveriesInput,
    ) -> Result<Vec<crate::ports::RuntimeEventDeliveryClaim>> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        let events = inner
            .runtime_events_by_flow_run_id
            .get(&input.flow_run_id)
            .cloned()
            .unwrap_or_default();
        let mut deliveries = Vec::new();
        for event in events {
            if deliveries.len() == input.limit
                || inner.runtime_delivery_acked.contains(&event.id)
                || event
                    .payload
                    .get("committed_delivery")
                    .and_then(Value::as_bool)
                    != Some(true)
            {
                continue;
            }
            let previous_generation = match inner.runtime_delivery_claims.get(&event.id) {
                Some((_, generation, lease_expires_at)) if *lease_expires_at > now => continue,
                Some((_, generation, _)) => *generation,
                None => 0,
            };
            let claim_token = Uuid::now_v7();
            let generation = previous_generation + 1;
            let lease_expires_at = now + time::Duration::seconds(input.lease_seconds);
            inner
                .runtime_delivery_claims
                .insert(event.id, (claim_token, generation, lease_expires_at));
            deliveries.push(crate::ports::RuntimeEventDeliveryClaim {
                event,
                claim_token,
                generation,
                lease_expires_at,
            });
        }
        Ok(deliveries)
    }

    pub(super) fn memory_ack_runtime_event_delivery(
        &self,
        input: &crate::ports::AckRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let owned = inner
            .runtime_delivery_claims
            .get(&input.event_id)
            .is_some_and(|(token, generation, _)| {
                *token == input.claim_token && *generation == input.expected_generation
            });
        if !owned {
            return Err(crate::errors::ControlPlaneError::Conflict(
                "runtime_event_delivery_claim_not_owned",
            )
            .into());
        }
        let event = inner
            .runtime_events_by_flow_run_id
            .values()
            .flatten()
            .find(|event| event.id == input.event_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("runtime delivery event not found"))?;
        inner.runtime_delivery_claims.remove(&input.event_id);
        inner.runtime_delivery_acked.insert(input.event_id);
        Ok(event)
    }

    pub(super) fn memory_release_runtime_event_delivery(
        &self,
        input: &crate::ports::ReleaseRuntimeEventDeliveryInput,
    ) -> Result<domain::RuntimeEventRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let owned = inner
            .runtime_delivery_claims
            .get(&input.event_id)
            .is_some_and(|(token, generation, _)| {
                *token == input.claim_token && *generation == input.expected_generation
            });
        if !owned {
            return Err(crate::errors::ControlPlaneError::Conflict(
                "runtime_event_delivery_claim_not_owned",
            )
            .into());
        }
        let event = inner
            .runtime_events_by_flow_run_id
            .values()
            .flatten()
            .find(|event| event.id == input.event_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("runtime delivery event not found"))?;
        inner.runtime_delivery_claims.remove(&input.event_id);
        Ok(event)
    }

    pub(super) fn memory_append_runtime_item(
        &self,
        input: &AppendRuntimeItemInput,
    ) -> Result<domain::RuntimeItemRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let now = OffsetDateTime::now_utc();
        let item = domain::RuntimeItemRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            span_id: input.span_id,
            kind: input.kind,
            status: input.status,
            source_event_id: input.source_event_id,
            input_ref: input.input_ref.clone(),
            output_ref: input.output_ref.clone(),
            usage_ledger_id: input.usage_ledger_id,
            trust_level: input.trust_level,
            created_at: now,
            updated_at: now,
        };
        inner
            .runtime_items_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(item.clone());
        Ok(item)
    }

    pub(super) fn memory_append_context_projection(
        &self,
        input: &AppendContextProjectionInput,
    ) -> Result<domain::ContextProjectionRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::ContextProjectionRecord {
            id: Uuid::now_v7(),
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            llm_turn_span_id: input.llm_turn_span_id,
            projection_kind: input.projection_kind.clone(),
            merge_stage_ref: input.merge_stage_ref.clone(),
            source_transcript_ref: input.source_transcript_ref.clone(),
            source_item_refs: input.source_item_refs.clone(),
            compaction_event_id: input.compaction_event_id,
            summary_version: input.summary_version.clone(),
            model_input_ref: input.model_input_ref.clone(),
            model_input_hash: input.model_input_hash.clone(),
            compacted_summary_ref: input.compacted_summary_ref.clone(),
            previous_projection_id: input.previous_projection_id,
            token_estimate: input.token_estimate,
            provider_continuation_metadata: input.provider_continuation_metadata.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .context_projections_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default()
            .push(record.clone());
        Ok(record)
    }

    pub(super) fn memory_put_canonical_runtime_content(
        &self,
        input: &crate::ports::PutCanonicalRuntimeContentInput,
    ) -> Result<domain::CanonicalRuntimeContentRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let serialized = serde_json::to_string(&input.content)?;
        let key = (input.application_id, serialized.clone());
        if let Some(id) = inner.canonical_runtime_content_ids_by_value.get(&key) {
            return Ok(inner
                .canonical_runtime_contents_by_id
                .get(id)
                .expect("canonical runtime content index must resolve")
                .clone());
        }
        let record = domain::CanonicalRuntimeContentRecord {
            id: Uuid::now_v7(),
            scope_id: input.scope_id,
            application_id: input.application_id,
            content_hash: format!("sha256:{:064x}", serialized.len()),
            content: input.content.clone(),
            byte_size: i64::try_from(serialized.len())?,
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .canonical_runtime_content_ids_by_value
            .insert(key, record.id);
        inner
            .canonical_runtime_contents_by_id
            .insert(record.id, record.clone());
        Ok(record)
    }

    pub(super) fn memory_append_context_version(
        &self,
        input: &crate::ports::AppendContextVersionInput,
    ) -> Result<domain::ContextVersionRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let record = domain::ContextVersionRecord {
            id: Uuid::now_v7(),
            scope_id: input.scope_id,
            application_id: input.application_id,
            flow_run_id: input.flow_run_id,
            parent_context_version_id: input.parent_context_version_id,
            sequence: input.sequence,
            transition_kind: input.transition_kind,
            transition_actor: input.transition_actor,
            declared_compaction_provenance: input.declared_compaction_provenance.clone(),
            actual_content_id: input.actual_content_id,
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .context_versions_by_id
            .insert(record.id, record.clone());
        Ok(record)
    }

    pub(super) fn memory_bind_invocation_context(
        &self,
        input: &crate::ports::BindInvocationContextInput,
    ) -> Result<domain::InvocationContextBindingRecord> {
        Ok(domain::InvocationContextBindingRecord {
            invocation_span_id: input.invocation_span_id,
            scope_id: input.scope_id,
            application_id: input.application_id,
            flow_run_id: input.flow_run_id,
            context_version_id: input.context_version_id,
            created_at: OffsetDateTime::now_utc(),
        })
    }

    pub(super) fn memory_append_provider_invocation_context(
        &self,
        input: &crate::ports::AppendProviderInvocationContextInput,
    ) -> Result<domain::ContextVersionRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        if let Some(id) = inner
            .invocation_context_versions_by_span
            .get(&input.invocation_span_id)
        {
            return Ok(inner
                .context_versions_by_id
                .get(id)
                .expect("invocation context version index must resolve")
                .clone());
        }
        let parent_id = inner
            .latest_invocation_context_version_by_run
            .get(&input.flow_run_id)
            .copied();
        let previous_content = parent_id
            .and_then(|id| inner.context_versions_by_id.get(&id))
            .and_then(|version| {
                inner
                    .canonical_runtime_contents_by_id
                    .get(&version.actual_content_id)
            })
            .map(|content| content.content.clone());
        let explicit = input
            .context_epoch
            .get("declaration")
            .and_then(Value::as_str)
            == Some("explicit");
        let observed_replacement = !explicit
            && previous_content.as_ref().is_some_and(|previous| {
                let old = previous.get("provider_messages").and_then(Value::as_array);
                let new = input
                    .actual_context
                    .get("provider_messages")
                    .and_then(Value::as_array);
                matches!((old, new), (Some(old), Some(new)) if !new.starts_with(old))
            });
        let serialized = serde_json::to_string(&input.actual_context)?;
        let content_key = (input.application_id, serialized.clone());
        let content_id = inner
            .canonical_runtime_content_ids_by_value
            .get(&content_key)
            .copied()
            .unwrap_or_else(|| {
                let content = domain::CanonicalRuntimeContentRecord {
                    id: Uuid::now_v7(),
                    scope_id: input.scope_id,
                    application_id: input.application_id,
                    content_hash: format!("sha256:{:064x}", serialized.len()),
                    content: input.actual_context.clone(),
                    byte_size: i64::try_from(serialized.len()).unwrap_or(i64::MAX),
                    created_at: OffsetDateTime::now_utc(),
                };
                inner
                    .canonical_runtime_content_ids_by_value
                    .insert(content_key, content.id);
                inner
                    .canonical_runtime_contents_by_id
                    .insert(content.id, content.clone());
                content.id
            });
        let version = domain::ContextVersionRecord {
            id: Uuid::now_v7(),
            scope_id: input.scope_id,
            application_id: input.application_id,
            flow_run_id: input.flow_run_id,
            parent_context_version_id: parent_id,
            sequence: inner
                .context_versions_by_id
                .values()
                .filter(|version| version.flow_run_id == input.flow_run_id)
                .map(|version| version.sequence)
                .max()
                .unwrap_or(-1)
                + 1,
            transition_kind: if explicit {
                domain::ContextTransitionKind::DeclaredCompaction
            } else if observed_replacement {
                domain::ContextTransitionKind::ObservedReplacement
            } else if parent_id.is_some() {
                domain::ContextTransitionKind::Append
            } else {
                domain::ContextTransitionKind::Initial
            },
            transition_actor: if explicit {
                domain::ContextTransitionActor::Client
            } else {
                domain::ContextTransitionActor::Host
            },
            declared_compaction_provenance: explicit.then(|| input.context_epoch.clone()),
            actual_content_id: content_id,
            created_at: OffsetDateTime::now_utc(),
        };
        inner
            .context_versions_by_id
            .insert(version.id, version.clone());
        inner
            .invocation_context_versions_by_span
            .insert(input.invocation_span_id, version.id);
        inner
            .latest_invocation_context_version_by_run
            .insert(input.flow_run_id, version.id);
        Ok(version)
    }

    pub(super) fn memory_append_recovery_history(
        &self,
        input: &crate::ports::AppendRecoveryHistoryInput,
    ) -> Result<domain::RecoveryHistoryRecord> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let records = inner
            .recovery_history_by_flow_run_id
            .entry(input.flow_run_id)
            .or_default();
        if let Some(existing) = records
            .iter()
            .find(|record| record.idempotency_key == input.idempotency_key)
        {
            return Ok(existing.clone());
        }
        let record = domain::RecoveryHistoryRecord {
            id: Uuid::now_v7(),
            scope_id: input.scope_id,
            application_id: input.application_id,
            flow_run_id: input.flow_run_id,
            node_run_id: input.node_run_id,
            sequence: input.sequence,
            state_code: input.state_code,
            coordinate: input.coordinate,
            context_version_id: input.context_version_id,
            recovery_content_id: input.recovery_content_id,
            idempotency_key: input.idempotency_key.clone(),
            created_at: OffsetDateTime::now_utc(),
        };
        records.push(record.clone());
        Ok(record)
    }

    pub(super) fn memory_load_runtime_context_content_lineage(
        &self,
        context_version_id: Uuid,
    ) -> Result<Vec<crate::ports::RuntimeContextContentVersion>> {
        let inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let mut current = Some(context_version_id);
        let mut lineage = Vec::new();
        while let Some(id) = current {
            let version = inner
                .context_versions_by_id
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("context version not found"))?;
            let content = inner
                .canonical_runtime_contents_by_id
                .get(&version.actual_content_id)
                .ok_or_else(|| anyhow::anyhow!("canonical runtime content not found"))?;
            lineage.push(crate::ports::RuntimeContextContentVersion {
                context_version_id: version.id,
                sequence: version.sequence,
                content: content.content.clone(),
            });
            current = version.parent_context_version_id;
        }
        lineage.reverse();
        Ok(lineage)
    }
}
