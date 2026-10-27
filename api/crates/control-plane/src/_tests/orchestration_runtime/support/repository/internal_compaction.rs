use super::*;
use control_plane_contracts::ports::{
    CommitInternalCompactionInput, InternalCompactionHead, ObserveInternalCompactionInput,
};

#[derive(Default)]
pub(super) struct InternalCompactionFixtureState {
    sessions:
        HashMap<(Uuid, Uuid, String), (ObserveInternalCompactionInput, InternalCompactionHead)>,
    observations: Vec<ObserveInternalCompactionInput>,
    candidates: Vec<CommitInternalCompactionInput>,
}

impl InMemoryOrchestrationRuntimeRepository {
    pub(crate) fn set_compaction_model_window(&self, instance_id: Uuid, window: u64) {
        let mut inner = self.inner.lock().unwrap();
        let instance = inner.instances_by_id.get_mut(&instance_id).unwrap();
        instance.configured_models[0].context_window_override_tokens = Some(window);
    }

    pub(crate) fn set_compaction_compatibility_mode(&self, run_id: Uuid, mode: Option<&str>) {
        self.inner
            .lock()
            .unwrap()
            .flow_runs_by_id
            .get_mut(&run_id)
            .unwrap()
            .compatibility_mode = mode.map(str::to_owned);
    }

    pub(crate) fn enable_internal_compaction_fixture(&self) {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .internal_compaction = Some(Default::default());
    }

    pub(super) fn memory_observe_internal_compaction(
        &self,
        input: &ObserveInternalCompactionInput,
    ) -> Result<InternalCompactionHead> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let state = inner
            .internal_compaction
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("internal compaction observation not implemented"))?;
        state.observations.push(input.clone());
        let (latest, head) = state
            .sessions
            .entry((
                input.scope_id,
                input.application_id,
                input.session_key.clone(),
            ))
            .or_insert_with(|| (input.clone(), InternalCompactionHead::default()));
        if latest.prefix_hash != input.prefix_hash
            || input.source_hashes.len() >= latest.source_hashes.len()
            || !latest.source_hashes.starts_with(&input.source_hashes)
        {
            *latest = input.clone();
        }
        Ok(head.clone())
    }

    pub(super) fn memory_commit_internal_compaction(
        &self,
        input: &CommitInternalCompactionInput,
    ) -> Result<Option<Uuid>> {
        let mut inner = self.inner.lock().expect("runtime repo mutex poisoned");
        let run_active = input.flow_run_id.is_none_or(|id| {
            inner.flow_runs_by_id.get(&id).is_some_and(|run| {
                run.application_id == input.application_id && !run.status.is_terminal()
            })
        });
        let state = inner
            .internal_compaction
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("internal compaction commit not implemented"))?;
        state.candidates.push(input.clone());
        let Some((latest, head)) = state.sessions.get_mut(&(
            input.scope_id,
            input.application_id,
            input.session_key.clone(),
        )) else {
            return Ok(None);
        };
        if !run_active
            || input.covered_source_hashes.is_empty()
            || head.checkpoint_id != input.expected_checkpoint_id
            || latest.prefix_hash != input.prefix_hash
            || !latest
                .source_hashes
                .starts_with(&input.covered_source_hashes)
        {
            return Ok(None);
        }
        let id = Uuid::now_v7();
        *head = InternalCompactionHead {
            checkpoint_id: Some(id),
            checkpoint: Some(input.checkpoint.clone()),
        };
        Ok(Some(id))
    }

    pub(crate) fn internal_compaction_observations(&self) -> Vec<ObserveInternalCompactionInput> {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .internal_compaction
            .as_ref()
            .map(|state| state.observations.clone())
            .unwrap_or_default()
    }

    pub(crate) fn internal_compaction_candidates(&self) -> Vec<CommitInternalCompactionInput> {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .internal_compaction
            .as_ref()
            .map(|state| state.candidates.clone())
            .unwrap_or_default()
    }

    pub(crate) fn internal_compaction_heads(&self) -> Vec<InternalCompactionHead> {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .internal_compaction
            .as_ref()
            .map(|state| {
                state
                    .sessions
                    .values()
                    .map(|(_, head)| head.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn internal_compaction_run_count(&self) -> usize {
        self.inner
            .lock()
            .expect("runtime repo mutex poisoned")
            .flow_runs_by_id
            .len()
    }
}
