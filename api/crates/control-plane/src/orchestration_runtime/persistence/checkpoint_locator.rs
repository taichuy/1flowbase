use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::orchestration_runtime) struct CheckpointLocatorPayload {
    node_id: String,
    next_node_index: usize,
    active_node_ids: Vec<String>,
    context_version_id: Option<Uuid>,
}

impl CheckpointLocatorPayload {
    pub(in crate::orchestration_runtime) fn from_snapshot(
        node_id: &str,
        snapshot: &orchestration_runtime::execution_state::CheckpointSnapshot,
    ) -> Self {
        Self {
            node_id: node_id.to_string(),
            next_node_index: snapshot.next_node_index,
            active_node_ids: snapshot.active_node_ids.clone(),
            context_version_id: None,
        }
    }

    #[cfg(test)]
    pub(in crate::orchestration_runtime) fn from_runtime_position(
        node_id: &str,
        next_node_index: usize,
        active_node_ids: Vec<String>,
    ) -> Self {
        Self {
            node_id: node_id.to_string(),
            next_node_index,
            active_node_ids,
            context_version_id: None,
        }
    }

    pub(in crate::orchestration_runtime) fn from_record(
        checkpoint: &domain::CheckpointRecord,
    ) -> Result<Self> {
        let node_id = checkpoint
            .locator_payload
            .get("node_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow!("checkpoint is missing node_id"))?;
        let next_node_index = checkpoint
            .locator_payload
            .get("next_node_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("checkpoint is missing next_node_index"))?;
        let next_node_index = usize::try_from(next_node_index)
            .map_err(|_| anyhow!("checkpoint next_node_index is too large"))?;
        let active_node_ids = checkpoint
            .locator_payload
            .get("active_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("checkpoint is missing active_node_ids"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| anyhow!("checkpoint active_node_ids must be strings"))
            })
            .collect::<Result<Vec<_>>>()?;
        let context_version_id = checkpoint
            .locator_payload
            .get("context_version_id")
            .and_then(Value::as_str)
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| anyhow!("checkpoint context_version_id must be a UUID"))?;

        Ok(Self {
            node_id,
            next_node_index,
            active_node_ids,
            context_version_id,
        })
    }

    pub(in crate::orchestration_runtime) fn into_json(self) -> Value {
        let mut locator = json!({
            "node_id": self.node_id,
            "next_node_index": self.next_node_index,
            "active_node_ids": self.active_node_ids,
        });
        if let Some(context_version_id) = self.context_version_id {
            locator["context_version_id"] = json!(context_version_id);
        }
        locator
    }

    pub(in crate::orchestration_runtime) fn into_checkpoint_snapshot(
        self,
        variable_snapshot: &Value,
    ) -> Result<orchestration_runtime::execution_state::CheckpointSnapshot> {
        Ok(orchestration_runtime::execution_state::CheckpointSnapshot {
            next_node_index: self.next_node_index,
            variable_pool: variable_snapshot
                .as_object()
                .cloned()
                .ok_or_else(|| anyhow!("checkpoint variable_snapshot must be an object"))?,
            active_node_ids: self.active_node_ids,
        })
    }

    pub(in crate::orchestration_runtime) fn into_node_id(self) -> String {
        self.node_id
    }
}

pub(super) const RECOVERY_CONTEXT_MARKER: &str = "__runtime_recovery_context";

pub(super) struct SparseCheckpointContent {
    pub(super) content: Value,
    pub(super) parent_context_version_id: Option<Uuid>,
    pub(super) variable_snapshot: Value,
}

pub(super) fn compact_checkpoint_content(
    snapshot: &orchestration_runtime::execution_state::CheckpointSnapshot,
    previous_variable_pool: Option<&Map<String, Value>>,
) -> Result<SparseCheckpointContent> {
    let marker = snapshot
        .variable_pool
        .get(RECOVERY_CONTEXT_MARKER)
        .and_then(Value::as_object);
    let parent_context_version_id = marker
        .and_then(|marker| marker.get("context_version_id"))
        .and_then(Value::as_str)
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| anyhow!("runtime recovery context marker must contain a UUID"))?;
    let sequence = marker
        .and_then(|marker| marker.get("sequence"))
        .and_then(Value::as_i64)
        .map(|sequence| sequence + 1)
        .unwrap_or(0);
    let mut current = snapshot.variable_pool.clone();
    current.remove(RECOVERY_CONTEXT_MARKER);
    let content = match previous_variable_pool {
        None => json!({ "format": "runtime_snapshot_v1", "variable_pool": current }),
        Some(previous) => checkpoint_delta_content(previous, &current),
    };
    Ok(SparseCheckpointContent {
        content,
        parent_context_version_id,
        variable_snapshot: json!({
            RECOVERY_CONTEXT_MARKER: {
                "parent_context_version_id": parent_context_version_id,
                "sequence": sequence,
            }
        }),
    })
}

fn checkpoint_delta_content(previous: &Map<String, Value>, current: &Map<String, Value>) -> Value {
    let mut set = Map::new();
    let mut llm_callback_appends = Vec::new();
    for (key, value) in current {
        if previous.get(key) != Some(value) {
            if let Some(append) = previous
                .get(key)
                .and_then(|previous| llm_callback_append(key, previous, value))
            {
                llm_callback_appends.push(append);
                continue;
            }
            set.insert(key.clone(), value.clone());
        }
    }
    let remove = previous
        .keys()
        .filter(|key| !current.contains_key(*key) && key.as_str() != RECOVERY_CONTEXT_MARKER)
        .cloned()
        .collect::<Vec<_>>();
    json!({
        "format": "runtime_delta_v1",
        "set": set,
        "remove": remove,
        "llm_callback_appends": llm_callback_appends,
    })
}

fn llm_callback_append(node_id: &str, previous: &Value, current: &Value) -> Option<Value> {
    let previous_state = previous.get("__llm_tool_callback")?.as_object()?;
    let current_state = current.get("__llm_tool_callback")?.as_object()?;
    let previous_history = previous_state.get("history")?.as_array()?;
    let current_history = current_state.get("history")?.as_array()?;
    if !current_history.starts_with(previous_history) {
        return None;
    }
    let mut node_overlay = current.as_object()?.clone();
    let mut state_overlay = node_overlay
        .remove("__llm_tool_callback")?
        .as_object()
        .cloned()?;
    state_overlay.remove("history");
    state_overlay.remove("system");
    Some(json!({
        "node_id": node_id,
        "node_overlay": node_overlay,
        "state_overlay": state_overlay,
        "system_changed": previous_state.get("system") != current_state.get("system"),
        "system_present": current_state.contains_key("system"),
        "system": current_state.get("system"),
        "history_append": current_history[previous_history.len()..],
    }))
}

pub(super) fn materialize_checkpoint_content(
    lineage: &[crate::ports::RuntimeContextContentVersion],
) -> Result<Map<String, Value>> {
    let mut variable_pool = Map::new();
    for version in lineage {
        match version.content.get("format").and_then(Value::as_str) {
            Some("runtime_snapshot_v1") => {
                variable_pool = version
                    .content
                    .get("variable_pool")
                    .and_then(Value::as_object)
                    .cloned()
                    .ok_or_else(|| {
                        anyhow!("runtime snapshot content must contain variable_pool")
                    })?;
            }
            Some("runtime_delta_v1") => {
                let set = version
                    .content
                    .get("set")
                    .and_then(Value::as_object)
                    .ok_or_else(|| anyhow!("runtime delta content must contain set"))?;
                for (key, value) in set {
                    variable_pool.insert(key.clone(), value.clone());
                }
                for key in version
                    .content
                    .get("remove")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    variable_pool.remove(key);
                }
                for append in version
                    .content
                    .get("llm_callback_appends")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    apply_llm_callback_append(&mut variable_pool, append)?;
                }
            }
            _ => return Err(anyhow!("unknown runtime checkpoint content format")),
        }
    }
    if let Some(latest) = lineage.last() {
        variable_pool.insert(
            RECOVERY_CONTEXT_MARKER.to_string(),
            json!({
                "context_version_id": latest.context_version_id,
                "sequence": latest.sequence,
            }),
        );
    }
    Ok(variable_pool)
}

fn apply_llm_callback_append(variable_pool: &mut Map<String, Value>, append: &Value) -> Result<()> {
    let node_id = append
        .get("node_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("LLM callback append is missing node_id"))?;
    let previous_node = variable_pool
        .get(node_id)
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("LLM callback node state must be an object"))?;
    let previous_state = previous_node
        .get("__llm_tool_callback")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("LLM callback state must be an object"))?;
    let mut node = append
        .get("node_overlay")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| anyhow!("LLM callback append must contain node_overlay"))?;
    let mut state = append
        .get("state_overlay")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| anyhow!("LLM callback append must contain state_overlay"))?;
    if append
        .get("system_changed")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("LLM callback append must contain system_changed"))?
    {
        if append
            .get("system_present")
            .and_then(Value::as_bool)
            .ok_or_else(|| anyhow!("LLM callback append must contain system_present"))?
        {
            state.insert(
                "system".to_string(),
                append
                    .get("system")
                    .cloned()
                    .ok_or_else(|| anyhow!("LLM callback append must contain system"))?,
            );
        }
    } else if let Some(system) = previous_state.get("system") {
        state.insert("system".to_string(), system.clone());
    }
    let mut history = previous_state
        .get("history")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("LLM callback history must be an array"))?;
    history.extend(
        append
            .get("history_append")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| anyhow!("LLM callback append must contain history_append"))?,
    );
    state.insert("history".to_string(), Value::Array(history));
    node.insert("__llm_tool_callback".to_string(), Value::Object(state));
    variable_pool.insert(node_id.to_string(), Value::Object(node));
    Ok(())
}

#[cfg(test)]
pub(in crate::orchestration_runtime) fn checkpoint_snapshot_from_record(
    checkpoint: &domain::CheckpointRecord,
) -> Result<orchestration_runtime::execution_state::CheckpointSnapshot> {
    CheckpointLocatorPayload::from_record(checkpoint)?
        .into_checkpoint_snapshot(&checkpoint.variable_snapshot)
}

pub(in crate::orchestration_runtime) async fn checkpoint_snapshot_from_record_with_context<R>(
    repository: &R,
    checkpoint: &domain::CheckpointRecord,
) -> Result<orchestration_runtime::execution_state::CheckpointSnapshot>
where
    R: crate::ports::OrchestrationRuntimeRepository,
{
    let locator = CheckpointLocatorPayload::from_record(checkpoint)?;
    let Some(context_version_id) = locator.context_version_id else {
        return locator.into_checkpoint_snapshot(&checkpoint.variable_snapshot);
    };
    let lineage = repository
        .load_runtime_context_content_lineage(context_version_id)
        .await?;
    let variable_pool = materialize_checkpoint_content(&lineage)?;
    Ok(orchestration_runtime::execution_state::CheckpointSnapshot {
        next_node_index: locator.next_node_index,
        variable_pool,
        active_node_ids: locator.active_node_ids,
    })
}

pub(in crate::orchestration_runtime) fn checkpoint_node_id(
    checkpoint: &domain::CheckpointRecord,
) -> Result<String> {
    Ok(CheckpointLocatorPayload::from_record(checkpoint)?.into_node_id())
}
