//! Synchronous AI Native LocalSummary. Provider credentials, metering, deadlines and
//! trajectory stay on the ordinary Generate path; no new run or tool executor is created.
use super::*;
use anyhow::{ensure, Context};
use control_plane_contracts::ports::{
    CommitInternalCompactionInput, ObserveInternalCompactionInput,
};
use orchestration_runtime::execution_engine::internal_compaction::{
    closed_prefix_ends, fingerprint, select_prefix, summary_request, validate_summary,
    CompactionSelection, InputBudget, WorkingCheckpoint, POLICY_KEY, SUMMARY_KEY,
};
use orchestration_runtime::execution_engine::ProviderInvoker;
use plugin_framework::provider_contract::{ProviderCountTokensInput, ProviderWireOperation};

impl<R, H> RuntimeProviderInvoker<R, H>
where
    R: ModelProviderRepository
        + OrchestrationRuntimeRepository
        + PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: ProviderRuntimePort + Clone + Send + Sync,
{
    async fn complete_input_count(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        input: &ProviderInvocationInput,
    ) -> Result<(u64, Value)> {
        let provider_count = self
            .count_tokens(
                runtime,
                ProviderCountTokensInput::from_invocation(input.clone()),
            )
            .await?;
        use plugin_framework::provider_contract::{
            ProviderCountTokensCoverage, ProviderCountTokensFallbackReason,
        };
        // A tokenizer may not model visible tool definitions or structured output.
        // That limitation is different from opaque content. Use the existing host
        // estimator only when it covers the whole readable input, and retain both
        // receipts. Coverage is not a claim of tokenizer accuracy or an upper bound.
        if provider_count.coverage == ProviderCountTokensCoverage::Partial
            && closed_prefix_ends(&input.messages).is_ok()
        {
            let mut estimate = estimate_provider_count_tokens(input)?;
            if estimate.coverage == ProviderCountTokensCoverage::Complete
                && estimate.unknown_block_count == 0
            {
                estimate.fallback_reason =
                    Some(ProviderCountTokensFallbackReason::CapabilityUnavailable);
                let tokens = InputBudget::count(&estimate)?;
                let mut receipt = serde_json::to_value(estimate)?;
                receipt["fallback_context"] = json!("provider_count_partial_visible_input");
                receipt["provider_count"] = serde_json::to_value(provider_count)?;
                return Ok((tokens, receipt));
            }
        }
        Ok((
            InputBudget::count(&provider_count)?,
            serde_json::to_value(provider_count)?,
        ))
    }

    pub(super) async fn apply_internal_compaction(
        &self,
        runtime: &orchestration_runtime::compiled_plan::CompiledLlmRuntime,
        route: &orchestration_runtime::execution_engine::ResolvedProviderRoute,
        input: &mut ProviderInvocationInput,
        effective_window: Option<u64>,
    ) -> Result<Option<Value>> {
        if input.operation != ProviderWireOperation::Generate {
            return Ok(None);
        }
        let is_summary = input.run_context.get(SUMMARY_KEY).and_then(Value::as_bool) == Some(true);
        if is_summary {
            let reserve = output_reserve(input)?;
            let budget = InputBudget::new(
                effective_window.context("summary context window unavailable")?,
                reserve,
            )?;
            let (tokens, _) = self.complete_input_count(runtime, input).await?;
            ensure!(
                budget.fits(tokens),
                "summary input plus output reserve exceeds model window"
            );
            ensure!(
                input.tools.is_empty() && input.mcp_bindings.is_empty(),
                "summary must not execute tools"
            );
            return Ok(None);
        }
        let enabled = orchestration_runtime::execution_engine::internal_compaction::policy_enabled(
            input
                .run_context
                .get(POLICY_KEY)
                .and_then(|v| v.get("enabled")),
        );
        // Disabled means no compaction work, including repository reads or budget
        // validation. The ordinary provider, usage and error paths remain owners.
        if !enabled {
            return Ok(None);
        }
        let Some(context) = self.flow_execution_context.as_ref() else {
            return Ok(None);
        };
        let Some(run_id) = self.flow_run_id else {
            return Ok(None);
        };
        let application_id = context.data_model.application_id;
        let reserve = output_reserve(input)?;
        // The reserved output must also be the actual limit sent upstream. A provider's
        // unknown default cannot silently exceed the host's independent safety budget.
        input
            .model_parameters
            .insert("max_output_tokens".into(), json!(reserve));
        input.model_parameters.remove("max_tokens");
        input.model_parameters.remove("max_completion_tokens");
        ensure!(
            input.previous_response_id.is_none() && input.native_transport.is_none(),
            "internal compaction requires complete canonical history"
        );
        let budget = InputBudget::new(
            effective_window.context("model context window unavailable")?,
            reserve,
        )?;
        let conversation = self
            .repository
            .get_flow_run_assistant_conversation_id(run_id)
            .await?;
        let session_identity = conversation
            .map(|id| id.to_string())
            // The existing scope is authenticated and absent for stateless calls.
            .or_else(|| self.provider_session_scope.clone())
            .unwrap_or_else(|| run_id.to_string());
        let node = input
            .trace_context
            .get("node_id")
            .context("compaction node identity unavailable")?;
        let session_key = fingerprint(&json!(["internal-compaction/v1", session_identity, node]))?;
        let prefix_hash = fingerprint(&json!([
            input.system,
            input.tools,
            input.mcp_bindings,
            input.response_format,
            input.request_context
        ]))?;
        let source = input.messages.clone();
        let source_hashes = source.iter().map(fingerprint).collect::<Result<Vec<_>>>()?;
        let head = self
            .repository
            .observe_internal_compaction(&ObserveInternalCompactionInput {
                scope_id: self.workspace_id,
                application_id,
                session_key: session_key.clone(),
                source: serde_json::to_value(&source)?,
                source_hashes: source_hashes.clone(),
                prefix_hash: prefix_hash.clone(),
            })
            .await?;
        let stored = head
            .checkpoint
            .clone()
            .map(serde_json::from_value::<WorkingCheckpoint>)
            .transpose()?;
        // A deliberate source/prefix edit rebuilds from raw history. Keep the old
        // durable head until a new candidate passes both validation and source CAS.
        let old = stored.filter(|cp| cp.view(&source, &prefix_hash).is_ok());
        if let Some(cp) = &old {
            input.messages = cp.view(&source, &prefix_hash)?;
        }
        let (before, count_receipt) = self.complete_input_count(runtime, input).await?;
        let mut receipt = json!({"enabled":true,"model":runtime.model,"provider_instance_id":runtime.provider_instance_id,
            "window":budget.window,"threshold":budget.threshold,"input_tokens_before":before,
            "output_reserve":budget.output_reserve,"safety_margin":budget.safety_margin,
            "count":count_receipt,"checkpoint_id":head.checkpoint_id,"status":if old.is_some(){"reused"}else{"below_threshold"},
            "cache_read_tokens":"unknown","cache_write_tokens":"unknown"});
        if !budget.should_compact(before) {
            receipt["input_tokens_after"] = json!(before);
            self.persist_internal_compaction_event(&receipt).await?;
            return Ok(Some(receipt));
        }
        let started = std::time::Instant::now();
        // One candidate publication. Each Generate consumes a new, closed source
        // prefix; shrinking an unsent batch is planning, not a provider retry.
        let attempt: Result<(WorkingCheckpoint, Vec<plugin_framework::provider_contract::ProviderMessage>, u64, Value)> = async {
            let selection = select_prefix(&source)?;
            let cuts = closed_prefix_ends(&source)?;
            let mut raw = input.clone();
            raw.messages = source.clone();
            let summary_budget = InputBudget::new(budget.window, 4096)?;
            let mut checkpoint = old.clone();
            let mut cursor = checkpoint.as_ref().map_or(0, |cp| cp.covered_source_hashes.len());
            ensure!(cursor < selection.covered_count, "compaction made no source progress");
            let mut batches = Vec::new();
            while cursor < selection.covered_count {
                let ends = cuts.iter().copied().filter(|end| *end > cursor && *end <= selection.covered_count).collect::<Vec<_>>();
                ensure!(!ends.is_empty(), "no closed history unit available for summary");
                let mut choice = ends.len() - 1;
                let (summary, summary_tokens, summary_count, end) = loop {
                    let end = ends[choice];
                    let batch = CompactionSelection {
                        covered_count: end,
                        retained_item_refs: selection.retained_item_refs.iter().copied().filter(|index| *index < end).collect(),
                    };
                    let mut summary = summary_request(&raw, &batch, checkpoint.as_ref())?;
                    summary.trace_context.insert("node_id".into(), node.clone());
                    let (tokens, count) = self.complete_input_count(runtime, &summary).await?;
                    if summary_budget.fits(tokens) {
                        break (summary, tokens, count, end);
                    }
                    ensure!(choice > 0, "one closed history unit plus checkpoint cannot fit the summary model budget");
                    // No token-monotonicity assumption: only a measured fitting
                    // request is ever submitted. The finite candidate set shrinks.
                    choice /= 2;
                };
                self.ensure_internal_compaction_run_active(application_id, run_id).await?;
                let summary_output = self.invoke_resolved_llm(runtime, route.clone(), summary).await?;
                ensure!(summary_output.result.tool_calls.is_empty() && summary_output.result.mcp_calls.is_empty(), "summary returned tool calls");
                let body = validate_summary(summary_output.result.final_content.as_deref(), summary_output.result.finish_reason.as_ref())?;
                self.ensure_internal_compaction_run_active(application_id, run_id).await?;
                batches.push(json!({"source_start":cursor,"source_end":end,
                    "input_tokens":summary_tokens,"count":summary_count,
                    "output_reserve":summary_budget.output_reserve,"safety_margin":summary_budget.safety_margin,
                    "usage":summary_output.result.usage}));
                checkpoint = Some(WorkingCheckpoint {
                    schema_version: 1, prefix_hash: prefix_hash.clone(),
                    covered_source_hashes: source_hashes[..end].to_vec(),
                    retained_item_refs: selection.retained_item_refs.iter().copied().filter(|index| *index < end).collect(),
                    summary: body,
                });
                cursor = end;
            }
            let checkpoint = checkpoint.context("no summary candidate produced")?;
            let view = checkpoint.view(&source, &prefix_hash)?;
            let mut candidate_input = input.clone();
            candidate_input.messages = view.clone();
            let (after, after_count) = self.complete_input_count(runtime, &candidate_input).await?;
            ensure!(budget.fits(after) && after < budget.threshold && after <= before.saturating_sub((before / 10).max(1)),
                "summary did not release enough context growth space");
            Ok((checkpoint, view, after, json!({"batches":batches,"after_count":after_count})))
        }.await;
        match attempt {
            Ok((checkpoint, view, after, summary_usage)) => {
                self.ensure_internal_compaction_run_active(application_id, run_id)
                    .await?;
                let checkpoint_id = self
                    .repository
                    .commit_internal_compaction(&CommitInternalCompactionInput {
                        scope_id: self.workspace_id,
                        application_id,
                        flow_run_id: Some(run_id),
                        session_key,
                        expected_checkpoint_id: head.checkpoint_id,
                        prefix_hash,
                        covered_source_hashes: checkpoint.covered_source_hashes.clone(),
                        checkpoint: serde_json::to_value(&checkpoint)?,
                    })
                    .await?
                    .context("checkpoint source or active pointer changed concurrently")?;
                input.messages = view;
                receipt["checkpoint_id"] = json!(checkpoint_id);
                receipt["status"] = json!("activated");
                receipt["input_tokens_after"] = json!(after);
                receipt["covered_message_count"] = json!(checkpoint.covered_source_hashes.len());
                receipt["retained_item_refs"] = json!(checkpoint.retained_item_refs);
                receipt["summary"] = summary_usage;
            }
            Err(error) => {
                receipt["status"] = json!("failed_preserved");
                receipt["failure"] = json!(error.to_string());
                receipt["duration_ms"] = json!(started.elapsed().as_millis() as u64);
                self.persist_internal_compaction_event(&receipt).await?;
                // Preserve the old view and checkpoint. Never send an unsafe request after failure.
                self.ensure_internal_compaction_run_active(application_id, run_id)
                    .await?;
                ensure!(
                    budget.fits(before),
                    "compaction failed and existing model input is unsafe: {error}"
                );
                return Ok(Some(receipt));
            }
        }
        receipt["duration_ms"] = json!(started.elapsed().as_millis() as u64);
        self.persist_internal_compaction_event(&receipt).await?;
        Ok(Some(receipt))
    }

    async fn ensure_internal_compaction_run_active(
        &self,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<()> {
        let run = self
            .repository
            .get_flow_run(application_id, run_id)
            .await?
            .context("compaction run disappeared")?;
        ensure!(
            !run.status.is_terminal(),
            "compaction run is no longer active"
        );
        Ok(())
    }
    async fn persist_internal_compaction_event(&self, receipt: &Value) -> Result<()> {
        let Some(run_id) = self.flow_run_id else {
            return Ok(());
        };
        let payload = crate::ports::RuntimeEventPayload {
            event_type: "internal_compaction".into(),
            source: crate::ports::RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: false,
            payload: receipt.clone(),
        };
        runtime_event_persister::persist_runtime_event_payload_for_generation(
            &self.repository,
            run_id,
            &payload,
            None,
        )
        .await
    }
}

fn output_reserve(input: &ProviderInvocationInput) -> Result<u64> {
    let configured_reserve = input
        .model_parameters
        .get("max_output_tokens")
        .or_else(|| input.model_parameters.get("max_tokens"))
        .or_else(|| input.model_parameters.get("max_completion_tokens"));
    let reserve = configured_reserve
        .map(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
                .filter(|value| *value > 0)
                .context("output reserve is not a positive integer")
        })
        .transpose()?
        .unwrap_or(4096);
    Ok(reserve)
}

#[cfg(test)]
#[path = "_tests/internal_compaction.rs"]
mod tests;
