//! Host-owned in-process Privacy Masking Kernel: policy cache, fail-closed masking, and memory-only session vault.
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use control_plane_contracts::ports::{
    AppendPrivacyMaskingLogInput, PrivacyMaskingInvalidationObserver, PrivacyMaskingRepository,
    UpsertPrivacyMaskingRuntimePolicyInput,
};
use extension_contracts::provider_contract::{
    NativePromptBlock, ProviderInvocationInput, ProviderInvocationResult, ProviderStreamEvent,
};
use extension_contracts::{
    PrivacyMaskingPreviewHitDto, PrivacyMaskingSubjectKind, PrivacyMaskingWorkspacePolicyDto,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use super::engine::{
    derive_hmac_subkey, mask_json_value, mask_text_with_rules, CompiledWorkspacePolicy,
    PrivacyPolicyCompileError, StageMaskState,
};
use super::stream_restorer::{ProviderStreamEventRestorer, RestorationStack, RestorationStage};

const SESSION_VAULT_MAX_ENTRIES: usize = 2048;
const SESSION_VAULT_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone)]
pub struct PrivacyInvocationContext {
    pub workspace_id: Uuid,
    pub flow_run_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub provider_instance_id: Uuid,
    pub provider_instance_name: String,
    pub model_id: String,
    pub protocol: String,
}

#[derive(Debug, Clone)]
pub struct SyncPolicySummary {
    pub policy_version: i64,
    pub enabled_rule_count: u32,
    pub active_subject_count: u32,
}

#[derive(Debug, Clone)]
pub struct PreviewMaskingOutput {
    pub masked_text: String,
    pub restored_text: Option<String>,
    pub hits: Vec<PrivacyMaskingPreviewHitDto>,
    pub total_hits: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum PrivacyPolicySyncError {
    #[error(transparent)]
    Compile(#[from] PrivacyPolicyCompileError),
    #[error(transparent)]
    Storage(#[from] anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct ExecutedMaskingStage {
    pub stage_index: usize,
    pub installation_id: Option<Uuid>,
    pub subject_kind: PrivacyMaskingSubjectKind,
    pub subject_id: String,
    pub subject_name: String,
    pub restore_output: bool,
    pub state: StageMaskState,
}

pub struct PreparedPrivacyMasking {
    pub context: PrivacyInvocationContext,
    pub stages: Vec<ExecutedMaskingStage>,
    pub restoration_stack: RestorationStack,
    pub all_stages_restore: bool,
    /// When `all_stages_restore == false`, holds the input representation that should be persisted to PG
    /// (stages with `restore_output == false` stay masked; stages with `restore_output == true` are restored).
    pub persisted_input_override: Option<ProviderInvocationInput>,
    /// Masked native wire body when `all_stages_restore == false` and `native_transport` is present.
    pub persisted_native_wire_body: Option<Value>,
}

impl PreparedPrivacyMasking {
    pub fn has_hits(&self) -> bool {
        self.stages.iter().any(|stage| stage.state.total_hits > 0)
    }

    pub fn should_restore_output(&self) -> bool {
        !self.restoration_stack.is_empty()
    }

    pub fn new_stream_restorer(&self) -> ProviderStreamEventRestorer {
        ProviderStreamEventRestorer::new(self.restoration_stack.clone())
    }

    pub fn restore_completed_output(
        &self,
        events: &mut [ProviderStreamEvent],
        result: &mut ProviderInvocationResult,
    ) {
        if self.restoration_stack.is_empty() {
            return;
        }
        // Events in `invocation_output.events` may contain chunked deltas or committed items.
        // Running a fresh stream restorer over the final event list restores text/reasoning/tool
        // boundaries without double-counting into the stage counters that live streaming already recorded.
        let non_counting_stack = self.restoration_stack.clone_without_shared_counters();
        let mut restorer = ProviderStreamEventRestorer::new(non_counting_stack.clone());
        let mut restored_events = Vec::with_capacity(events.len());
        for event in events.iter().cloned() {
            restored_events.extend(restorer.process_event(event));
        }
        restored_events.extend(restorer.flush_all());
        // Preserve event slice length if caller expects in-place mutation; replace each item if 1:1,
        // otherwise restore directly on each event item.
        if restored_events.len() == events.len() {
            for (dst, src) in events.iter_mut().zip(restored_events) {
                *dst = src;
            }
        } else {
            for event in events.iter_mut() {
                restore_event_in_place(&non_counting_stack, event);
            }
        }
        non_counting_stack.restore_invocation_result(result);
    }

    /// Restores completed output while recording restoration hits when no live stream task ran.
    pub fn restore_completed_output_counting(
        &self,
        events: &mut Vec<ProviderStreamEvent>,
        result: &mut ProviderInvocationResult,
    ) {
        if self.restoration_stack.is_empty() {
            return;
        }
        let mut restorer = ProviderStreamEventRestorer::new(self.restoration_stack.clone());
        let mut restored_events = Vec::with_capacity(events.len());
        for event in events.drain(..) {
            restored_events.extend(restorer.process_event(event));
        }
        restored_events.extend(restorer.flush_all());
        *events = restored_events;
        // Restore final_content / tool_calls without double-counting if events already counted them.
        let non_counting = self.restoration_stack.clone_without_shared_counters();
        non_counting.restore_invocation_result(result);
    }
}

fn restore_event_in_place(stack: &RestorationStack, event: &mut ProviderStreamEvent) {
    match event {
        ProviderStreamEvent::TextDelta { delta }
        | ProviderStreamEvent::ReasoningDelta { delta } => {
            if delta.contains("{{") {
                *delta = stack.restore_str(delta);
            }
        }
        ProviderStreamEvent::ToolCallDelta { delta, .. }
        | ProviderStreamEvent::McpCallDelta { delta, .. } => {
            stack.restore_json(delta);
        }
        ProviderStreamEvent::ToolCallCommit { call } => stack.restore_tool_call(call),
        ProviderStreamEvent::McpCallCommit { call } => stack.restore_mcp_call(call),
        ProviderStreamEvent::OutputItem { item, .. } => stack.restore_json(item),
        ProviderStreamEvent::ResponsesOutputDelta { event }
        | ProviderStreamEvent::NativeEvent { event, .. } => stack.restore_json(event),
        _ => {}
    }
}

#[derive(Default)]
struct SessionRestorationVault {
    entries: HashMap<(Uuid, String), (Instant, HashMap<String, String>)>,
}

impl SessionRestorationVault {
    fn get(&mut self, workspace_id: Uuid, response_id: &str) -> HashMap<String, String> {
        let now = Instant::now();
        self.evict_expired(now);
        self.entries
            .get(&(workspace_id, response_id.to_string()))
            .map(|(_, map)| map.clone())
            .unwrap_or_default()
    }

    fn insert(
        &mut self,
        workspace_id: Uuid,
        response_id: String,
        map: HashMap<String, String>,
    ) {
        if map.is_empty() || response_id.trim().is_empty() {
            return;
        }
        let now = Instant::now();
        self.evict_expired(now);
        if self.entries.len() >= SESSION_VAULT_MAX_ENTRIES {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, (ts, _))| *ts)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&oldest_key);
            }
        }
        self.entries.insert((workspace_id, response_id), (now, map));
    }

    fn evict_expired(&mut self, now: Instant) {
        self.entries
            .retain(|_, (inserted_at, _)| now.saturating_duration_since(*inserted_at) <= SESSION_VAULT_TTL);
    }
}

pub struct PrivacyMaskingKernel {
    repository: Arc<dyn PrivacyMaskingRepository>,
    hmac_subkey: [u8; 32],
    policy_cache: RwLock<HashMap<Uuid, Arc<Vec<CompiledWorkspacePolicy>>>>,
    session_vault: Mutex<SessionRestorationVault>,
}

impl PrivacyMaskingKernel {
    pub fn new(
        repository: Arc<dyn PrivacyMaskingRepository>,
        master_key: &str,
    ) -> anyhow::Result<Self> {
        let hmac_subkey = derive_hmac_subkey(master_key)?;
        Ok(Self {
            repository,
            hmac_subkey,
            policy_cache: RwLock::new(HashMap::new()),
            session_vault: Mutex::new(SessionRestorationVault::default()),
        })
    }

    pub async fn sync_workspace_policy(
        &self,
        installation_id: Uuid,
        workspace_id: Uuid,
        updated_by: Option<Uuid>,
        policy: &PrivacyMaskingWorkspacePolicyDto,
    ) -> Result<SyncPolicySummary, PrivacyPolicySyncError> {
        let compiled = CompiledWorkspacePolicy::compile(Some(installation_id), 0, policy)?;
        let record = self
            .repository
            .upsert_privacy_masking_policy(&UpsertPrivacyMaskingRuntimePolicyInput {
                installation_id,
                workspace_id,
                policy: policy.clone(),
                updated_by,
            })
            .await?;
        self.invalidate_workspace(workspace_id);
        Ok(SyncPolicySummary {
            policy_version: record.policy_version,
            enabled_rule_count: compiled.enabled_rule_count(),
            active_subject_count: compiled.active_subject_count(),
        })
    }

    pub async fn preview_masking(
        &self,
        workspace_id: Uuid,
        text: &str,
        override_policy: Option<&PrivacyMaskingWorkspacePolicyDto>,
        restore_output: bool,
    ) -> Result<PreviewMaskingOutput, PrivacyPolicySyncError> {
        let policies: Vec<CompiledWorkspacePolicy> = if let Some(dto) = override_policy {
            vec![CompiledWorkspacePolicy::compile(None, 0, dto)?]
        } else {
            self.load_workspace_policies(workspace_id)
                .await?
                .as_ref()
                .clone()
        };

        let mut current = text.to_string();
        let mut combined_hits = Vec::new();
        let mut total_hits = 0u32;
        let mut stages = Vec::new();

        for (stage_index, policy) in policies.iter().enumerate() {
            let rules = policy.all_enabled_rules();
            if rules.is_empty() {
                continue;
            }
            let mut state = StageMaskState::default();
            current = mask_text_with_rules(
                &current,
                workspace_id,
                &self.hmac_subkey,
                &policy.allowlist_lower,
                &rules,
                &mut state,
            );
            total_hits = total_hits.saturating_add(state.total_hits);
            combined_hits.extend(state.to_preview_hits());
            if restore_output && !state.restore_map.is_empty() {
                stages.push(RestorationStage {
                    stage_index,
                    map: state.restore_map,
                });
            }
        }

        let restored_text = if restore_output {
            let stack = RestorationStack::new(stages, HashMap::new());
            Some(stack.restore_str(&current))
        } else {
            None
        };

        Ok(PreviewMaskingOutput {
            masked_text: current,
            restored_text,
            hits: combined_hits,
            total_hits,
        })
    }

    pub async fn has_active_policies(&self, workspace_id: Uuid) -> anyhow::Result<bool> {
        Ok(!self.load_workspace_policies(workspace_id).await?.is_empty())
    }

    pub async fn load_workspace_policies(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Arc<Vec<CompiledWorkspacePolicy>>> {
        if let Ok(guard) = self.policy_cache.read() {
            if let Some(cached) = guard.get(&workspace_id) {
                return Ok(Arc::clone(cached));
            }
        }
        let records = self
            .repository
            .list_active_privacy_masking_policies(workspace_id)
            .await?;
        let mut compiled = Vec::with_capacity(records.len());
        for record in records {
            let policy = CompiledWorkspacePolicy::compile(
                Some(record.installation_id),
                record.policy_version,
                &record.policy,
            )
            .map_err(|err| anyhow::anyhow!("privacy_masking_policy_compile_failed: {err}"))?;
            compiled.push(policy);
        }
        let compiled = Arc::new(compiled);
        if let Ok(mut guard) = self.policy_cache.write() {
            guard.insert(workspace_id, Arc::clone(&compiled));
        }
        Ok(compiled)
    }

    /// Evaluates matched subjects in sequential routing-chain order:
    /// Stage 1: `agent_flow` binding -> Stage 2: `model_provider_instance` binding.
    /// Mutates `input` in place with masked content and recomputes `native_transport` digest/size.
    /// Fail-closed: returns `Err` on any policy load, compile, or digest failure.
    pub async fn prepare_invocation(
        &self,
        ctx: &PrivacyInvocationContext,
        input: &mut ProviderInvocationInput,
    ) -> anyhow::Result<Option<PreparedPrivacyMasking>> {
        let policies = self.load_workspace_policies(ctx.workspace_id).await?;
        if policies.is_empty() {
            return Ok(None);
        }

        let mut stages: Vec<ExecutedMaskingStage> = Vec::new();
        let mut any_matched_binding = false;
        let mut all_stages_restore = true;

        // Stage 1: AgentFlow subject bindings
        if let Some(application_id) = ctx.application_id {
            let app_id_str = application_id.to_string();
            for policy in policies.iter() {
                let Some(binding) =
                    policy.matching_binding(PrivacyMaskingSubjectKind::AgentFlow, &app_id_str)
                else {
                    continue;
                };
                any_matched_binding = true;
                if !binding.restore_output {
                    all_stages_restore = false;
                }
                let rules = policy.rules_for_binding(binding);
                let mut state = StageMaskState::default();
                if !rules.is_empty() {
                    mask_provider_invocation_input(
                        input,
                        ctx.workspace_id,
                        &self.hmac_subkey,
                        &policy.allowlist_lower,
                        &rules,
                        &mut state,
                    )?;
                }
                let stage_index = stages.len();
                stages.push(ExecutedMaskingStage {
                    stage_index,
                    installation_id: policy.installation_id,
                    subject_kind: PrivacyMaskingSubjectKind::AgentFlow,
                    subject_id: app_id_str.clone(),
                    subject_name: if binding.subject_name.is_empty() {
                        app_id_str.clone()
                    } else {
                        binding.subject_name.clone()
                    },
                    restore_output: binding.restore_output,
                    state,
                });
            }
        }

        // Stage 2: ModelProviderInstance subject bindings (sequential chain over Stage 1 output)
        let instance_id_str = ctx.provider_instance_id.to_string();
        for policy in policies.iter() {
            let Some(binding) = policy.matching_binding(
                PrivacyMaskingSubjectKind::ModelProviderInstance,
                &instance_id_str,
            ) else {
                continue;
            };
            any_matched_binding = true;
            if !binding.restore_output {
                all_stages_restore = false;
            }
            let rules = policy.rules_for_binding(binding);
            let mut state = StageMaskState::default();
            if !rules.is_empty() {
                mask_provider_invocation_input(
                    input,
                    ctx.workspace_id,
                    &self.hmac_subkey,
                    &policy.allowlist_lower,
                    &rules,
                    &mut state,
                )?;
            }
            let stage_index = stages.len();
            stages.push(ExecutedMaskingStage {
                stage_index,
                installation_id: policy.installation_id,
                subject_kind: PrivacyMaskingSubjectKind::ModelProviderInstance,
                subject_id: instance_id_str.clone(),
                subject_name: if binding.subject_name.is_empty() {
                    ctx.provider_instance_name.clone()
                } else {
                    binding.subject_name.clone()
                },
                restore_output: binding.restore_output,
                state,
            });
        }

        if !any_matched_binding {
            return Ok(None);
        }

        // Scrub earlier stages' `masked_snippet` against all subsequent stages' masked secrets
        // so cross-stage neighboring sensitive values never leak into an earlier stage's audit snippet.
        scrub_cross_stage_snippets(&mut stages);

        let restoration_stages: Vec<RestorationStage> = stages
            .iter()
            .filter(|stage| stage.restore_output && !stage.state.restore_map.is_empty())
            .map(|stage| RestorationStage {
                stage_index: stage.stage_index,
                map: stage.state.restore_map.clone(),
            })
            .collect();

        // Session vault fallback only applies when output restoration is enabled.
        let session_fallback = if stages.iter().any(|s| s.restore_output) {
            let prev_id = input
                .previous_response_id
                .as_deref()
                .or_else(|| {
                    input
                        .native_transport
                        .as_ref()
                        .and_then(|t| t.wire_body.get("previous_response_id"))
                        .and_then(Value::as_str)
                })
                .filter(|s| !s.trim().is_empty());
            if let Some(prev_id) = prev_id {
                self.session_vault
                    .lock()
                    .map(|mut vault| vault.get(ctx.workspace_id, prev_id))
                    .unwrap_or_default()
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        let restoration_stack = RestorationStack::new(restoration_stages, session_fallback);

        // When any stage disables restoration (`restore_output == false`), compute the exact
        // persisted input view: stages with `restore_output == false` stay masked, while any
        // stage with `restore_output == true` is unwound back to plaintext.
        let (persisted_input_override, persisted_native_wire_body) = if !all_stages_restore {
            let mut persisted = input.clone();
            if !restoration_stack.is_empty() {
                let non_counting = restoration_stack.clone_without_shared_counters();
                restore_provider_invocation_input(&non_counting, &mut persisted)?;
            }
            let wire_body = persisted
                .native_transport
                .as_ref()
                .map(|t| t.wire_body.clone());
            (Some(persisted), wire_body)
        } else {
            (None, None)
        };

        Ok(Some(PreparedPrivacyMasking {
            context: ctx.clone(),
            stages,
            restoration_stack,
            all_stages_restore,
            persisted_input_override,
            persisted_native_wire_body,
        }))
    }

    /// Records the memory-only restoration map for `previous_response_id` continuations
    /// and appends structured zero-plaintext audit logs for every stage that had hits.
    pub async fn finalize_invocation(
        &self,
        prepared: &PreparedPrivacyMasking,
        response_id: Option<&str>,
    ) {
        if let Some(response_id) = response_id.filter(|s| !s.trim().is_empty()) {
            let merged = prepared.restoration_stack.merged_restore_map();
            if !merged.is_empty() {
                if let Ok(mut vault) = self.session_vault.lock() {
                    vault.insert(
                        prepared.context.workspace_id,
                        response_id.to_string(),
                        merged,
                    );
                }
            }
        }

        let now = OffsetDateTime::now_utc();
        for stage in &prepared.stages {
            if stage.state.total_hits == 0 {
                continue;
            }
            let restored_counts = prepared
                .restoration_stack
                .restored_counts_for_stage(stage.stage_index);
            let restored_count: u32 = restored_counts.values().copied().sum();
            let matched_rules = stage.state.to_log_matches(&restored_counts);
            let log_input = AppendPrivacyMaskingLogInput {
                id: Uuid::now_v7(),
                workspace_id: prepared.context.workspace_id,
                installation_id: stage.installation_id,
                flow_run_id: prepared.context.flow_run_id,
                application_id: prepared.context.application_id,
                provider_instance_id: Some(prepared.context.provider_instance_id),
                subject_kind: stage.subject_kind,
                subject_id: stage.subject_id.clone(),
                subject_name: stage.subject_name.clone(),
                model_id: prepared.context.model_id.clone(),
                protocol: prepared.context.protocol.clone(),
                restore_enabled: stage.restore_output,
                hit_count: stage.state.total_hits,
                restored_count,
                matched_rules,
                created_at: now,
            };
            if let Err(error) = self.repository.append_privacy_masking_log(&log_input).await {
                tracing::warn!(
                    workspace_id = %prepared.context.workspace_id,
                    subject_kind = %stage.subject_kind.as_str(),
                    subject_id = %stage.subject_id,
                    error = %error,
                    "failed to append privacy masking audit log"
                );
            }
        }
    }
}

impl PrivacyMaskingInvalidationObserver for PrivacyMaskingKernel {
    fn invalidate_installation(&self, _installation_id: Uuid) {
        if let Ok(mut guard) = self.policy_cache.write() {
            guard.clear();
        }
    }

    fn invalidate_workspace(&self, workspace_id: Uuid) {
        if let Ok(mut guard) = self.policy_cache.write() {
            guard.remove(&workspace_id);
        }
    }
}

fn scrub_cross_stage_snippets(stages: &mut [ExecutedMaskingStage]) {
    if stages.len() < 2 {
        return;
    }
    let stage_maps: Vec<Vec<(String, String)>> = stages
        .iter()
        .map(|stage| {
            let mut pairs: Vec<(String, String)> = stage
                .state
                .restore_map
                .iter()
                .map(|(placeholder, raw)| (placeholder.clone(), raw.clone()))
                .collect();
            // Replace longest raw strings first for deterministic scrubbing.
            pairs.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
            pairs
        })
        .collect();

    for (idx, stage) in stages.iter_mut().enumerate() {
        for later_pairs in &stage_maps[(idx + 1)..] {
            for entry in stage.state.matches.values_mut() {
                for (later_placeholder, later_raw) in later_pairs {
                    if !later_raw.is_empty()
                        && !later_raw.contains(&entry.placeholder)
                        && entry.masked_snippet.contains(later_raw.as_str())
                    {
                        entry.masked_snippet = entry
                            .masked_snippet
                            .replace(later_raw.as_str(), later_placeholder);
                    }
                }
            }
        }
    }
}

fn mask_provider_invocation_input(
    input: &mut ProviderInvocationInput,
    workspace_id: Uuid,
    hmac_subkey: &[u8; 32],
    allowlist_lower: &std::collections::HashSet<String>,
    rules: &[&super::engine::CompiledRule],
    state: &mut StageMaskState,
) -> anyhow::Result<()> {
    for message in &mut input.messages {
        if !message.content.is_empty() {
            message.content = mask_text_with_rules(
                &message.content,
                workspace_id,
                hmac_subkey,
                allowlist_lower,
                rules,
                state,
            );
        }
        if let Some(content_blocks) = message.content_blocks.as_mut() {
            mask_json_value(
                content_blocks,
                workspace_id,
                hmac_subkey,
                allowlist_lower,
                rules,
                state,
            );
        }
        if let Some(tool_calls) = message.tool_calls.as_mut() {
            mask_json_value(
                tool_calls,
                workspace_id,
                hmac_subkey,
                allowlist_lower,
                rules,
                state,
            );
        }
    }

    for block in &mut input.system {
        match block {
            NativePromptBlock::Text { text, .. } => {
                if !text.is_empty() {
                    *text = mask_text_with_rules(
                        text,
                        workspace_id,
                        hmac_subkey,
                        allowlist_lower,
                        rules,
                        state,
                    );
                }
            }
        }
    }

    if let Some(envelope) = input.client_protocol_envelope.as_mut() {
        mask_protocol_body_map(
            &mut envelope.body,
            workspace_id,
            hmac_subkey,
            allowlist_lower,
            rules,
            state,
        );
        if let Some(source_request) = envelope.source_request.as_mut() {
            if let Some(body) = source_request.body.as_mut() {
                mask_json_value(
                    body,
                    workspace_id,
                    hmac_subkey,
                    allowlist_lower,
                    rules,
                    state,
                );
            }
        }
    }

    if let Some(transport) = input.native_transport.as_mut() {
        let before_hits = state.total_hits;
        mask_json_value(
            &mut transport.wire_body,
            workspace_id,
            hmac_subkey,
            allowlist_lower,
            rules,
            state,
        );
        if state.total_hits > before_hits {
            let (digest, size_bytes) = canonical_json_digest_and_size(&transport.wire_body)?;
            transport.digest = digest;
            transport.size_bytes = size_bytes as u64;
        }
    }

    Ok(())
}

fn mask_protocol_body_map(
    body: &mut BTreeMap<String, Value>,
    workspace_id: Uuid,
    hmac_subkey: &[u8; 32],
    allowlist_lower: &std::collections::HashSet<String>,
    rules: &[&super::engine::CompiledRule],
    state: &mut StageMaskState,
) {
    for (key, value) in body.iter_mut() {
        if matches!(
            key.as_str(),
            "model" | "previous_response_id" | "conversation" | "stream" | "stream_options"
        ) {
            continue;
        }
        mask_json_value(
            value,
            workspace_id,
            hmac_subkey,
            allowlist_lower,
            rules,
            state,
        );
    }
}

fn restore_provider_invocation_input(
    stack: &RestorationStack,
    input: &mut ProviderInvocationInput,
) -> anyhow::Result<()> {
    for message in &mut input.messages {
        if message.content.contains("{{") {
            message.content = stack.restore_str(&message.content);
        }
        if let Some(blocks) = message.content_blocks.as_mut() {
            stack.restore_json(blocks);
        }
        if let Some(tool_calls) = message.tool_calls.as_mut() {
            stack.restore_json(tool_calls);
        }
    }
    for block in &mut input.system {
        match block {
            NativePromptBlock::Text { text, .. } => {
                if text.contains("{{") {
                    *text = stack.restore_str(text);
                }
            }
        }
    }
    if let Some(envelope) = input.client_protocol_envelope.as_mut() {
        for value in envelope.body.values_mut() {
            stack.restore_json(value);
        }
        if let Some(source_request) = envelope.source_request.as_mut() {
            if let Some(body) = source_request.body.as_mut() {
                stack.restore_json(body);
            }
        }
    }
    if let Some(transport) = input.native_transport.as_mut() {
        stack.restore_json(&mut transport.wire_body);
        let (digest, size_bytes) = canonical_json_digest_and_size(&transport.wire_body)?;
        transport.digest = digest;
        transport.size_bytes = size_bytes as u64;
    }
    Ok(())
}

/// Canonical JSON SHA-256 digest and serialized byte size matching `ProviderTransportPayload`.
pub(crate) fn canonical_json_digest_and_size(value: &Value) -> anyhow::Result<(String, usize)> {
    struct DigestWriter {
        hash: Sha256,
        bytes: usize,
    }
    impl Write for DigestWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes = self
                .bytes
                .checked_add(bytes.len())
                .ok_or_else(|| io::Error::other("privacy_native_transport_size_overflow"))?;
            self.hash.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn write_canonical(value: &Value, out: &mut impl Write) -> anyhow::Result<()> {
        match value {
            Value::Object(object) => {
                out.write_all(b"{")?;
                let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
                keys.sort_unstable();
                for (index, key) in keys.into_iter().enumerate() {
                    if index > 0 {
                        out.write_all(b",")?;
                    }
                    serde_json::to_writer(&mut *out, key)?;
                    out.write_all(b":")?;
                    write_canonical(&object[key], out)?;
                }
                out.write_all(b"}")?;
                Ok(())
            }
            Value::Array(values) => {
                out.write_all(b"[")?;
                for (index, item) in values.iter().enumerate() {
                    if index > 0 {
                        out.write_all(b",")?;
                    }
                    write_canonical(item, out)?;
                }
                out.write_all(b"]")?;
                Ok(())
            }
            _ => Ok(serde_json::to_writer(out, value)?),
        }
    }
    let mut writer = DigestWriter {
        hash: Sha256::new(),
        bytes: 0,
    };
    write_canonical(value, &mut writer)?;
    Ok((
        format!("sha256:{:x}", base16ct::HexDisplay(&writer.hash.finalize())),
        writer.bytes,
    ))
}
