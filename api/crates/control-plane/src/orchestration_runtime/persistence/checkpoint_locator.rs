use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::orchestration_runtime) struct CheckpointLocatorPayload {
    node_id: String,
    next_node_index: usize,
    active_node_ids: Vec<String>,
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

        Ok(Self {
            node_id,
            next_node_index,
            active_node_ids,
        })
    }

    pub(in crate::orchestration_runtime) fn into_json(self) -> Value {
        json!({
            "node_id": self.node_id,
            "next_node_index": self.next_node_index,
            "active_node_ids": self.active_node_ids,
        })
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

pub(in crate::orchestration_runtime) fn checkpoint_snapshot_from_record(
    checkpoint: &domain::CheckpointRecord,
) -> Result<orchestration_runtime::execution_state::CheckpointSnapshot> {
    CheckpointLocatorPayload::from_record(checkpoint)?
        .into_checkpoint_snapshot(&checkpoint.variable_snapshot)
}

pub(in crate::orchestration_runtime) fn checkpoint_node_id(
    checkpoint: &domain::CheckpointRecord,
) -> Result<String> {
    Ok(CheckpointLocatorPayload::from_record(checkpoint)?.into_node_id())
}
