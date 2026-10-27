//! Synchronous AI Native LocalSummary. Provider credentials, metering, deadlines and
//! trajectory stay on the ordinary Generate path; no new run or tool executor is created.
use super::*;
use anyhow::{ensure, Context};
use control_plane_contracts::ports::{
    CommitInternalCompactionInput, ObserveInternalCompactionInput,
};
use orchestration_runtime::execution_engine::internal_compaction::{
    fingerprint, select_prefix, summary_request, validate_summary, InputBudget, WorkingCheckpoint,
    POLICY_KEY, SUMMARY_KEY,
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
        let count = self
            .count_tokens(
                runtime,
                ProviderCountTokensInput::from_invocation(input.clone()),
            )
            .await?;
        Ok((InputBudget::count(&count)?, serde_json::to_value(count)?))
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
        let explicit = input
            .run_context
            .get(POLICY_KEY)
            .and_then(|v| v.get("enabled"))
            .and_then(Value::as_bool);
        let Some(context) = self.flow_execution_context.as_ref() else {
            return Ok(None);
        };
        let Some(run_id) = self.flow_run_id else {
            return Ok(None);
        };
        let application_id = context.data_model.application_id;
        let run = self
            .repository
            .get_flow_run(application_id, run_id)
            .await?
            .context("compaction run is unavailable")?;
        let assistant = run.compatibility_mode.as_deref() == Some("embedded_assistant");
        // Client protocol history and native continuations remain owned by their client.
        let external = input.client_protocol_envelope.is_some() && !assistant;
        if !orchestration_runtime::execution_engine::internal_compaction::policy_enabled(
            assistant, explicit, external,
        ) {
            return Ok(None);
        }
        let reserve = output_reserve(input)?;
        // The reserved output must also be the actual limit sent upstream. A provider's
        // unknown default cannot silently exceed the host's independent safety budget.
        input
            .model_parameters
            .insert("max_output_tokens".into(), json!(reserve));
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
        // Exactly one candidate attempt per invocation; failure cannot loop over unchanged source.
        let attempt:Result<(WorkingCheckpoint, Vec<plugin_framework::provider_contract::ProviderMessage>,u64,Value)>=async {
            let selection=select_prefix(&source)?;
            let mut raw=input.clone(); raw.messages=source.clone();
            let mut summary=summary_request(&raw,&selection,old.as_ref())?;
            summary.trace_context.insert("node_id".into(),node.clone());
            let summary_budget=InputBudget::new(budget.window,4096)?;
            let (summary_tokens,summary_count)=self.complete_input_count(runtime,&summary).await?;
            ensure!(summary_budget.fits(summary_tokens),"summary request cannot fit its own model budget");
            self.ensure_internal_compaction_run_active(application_id,run_id).await?;
            let summary_output=self.invoke_resolved_llm(runtime,route.clone(),summary).await?;
            ensure!(summary_output.result.tool_calls.is_empty() && summary_output.result.mcp_calls.is_empty(),"summary returned tool calls");
            let body=validate_summary(summary_output.result.final_content.as_deref(),summary_output.result.finish_reason.as_ref())?;
            self.ensure_internal_compaction_run_active(application_id,run_id).await?;
            let checkpoint=WorkingCheckpoint {schema_version:1,prefix_hash:prefix_hash.clone(),
                covered_source_hashes:source_hashes[..selection.covered_count].to_vec(),
                retained_item_refs:selection.retained_item_refs,summary:body};
            let view=checkpoint.view(&source,&prefix_hash)?;
            let mut candidate_input=input.clone(); candidate_input.messages=view.clone();
            let (after,after_count)=self.complete_input_count(runtime,&candidate_input).await?;
            ensure!(budget.fits(after) && after < budget.threshold && after <= before.saturating_sub((before/10).max(1)),"summary did not release enough context growth space");
            Ok((checkpoint,view,after,json!({"input_tokens":summary_tokens,"count":summary_count,"usage":summary_output.result.usage,"after_count":after_count})))
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
        .or_else(|| input.model_parameters.get("max_tokens"));
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
