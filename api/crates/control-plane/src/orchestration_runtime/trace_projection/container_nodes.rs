use super::*;
use std::collections::{BTreeMap, HashMap};

impl TraceProjectionBuilder {
    /// Preserve receipt identity and order while nesting executions under their actual round.
    pub(super) fn nest_container_nodes(&mut self, records: &[domain::NodeRunRecord]) {
        let by_receipt: HashMap<_, _> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.trace_node_id, index))
            .collect();
        let by_execution: HashMap<_, _> = records
            .iter()
            .filter_map(|record| {
                let identity = record.execution_identity.as_ref()?;
                let locator = format!("run:{}/node:{}", self.flow_run_id, record.id);
                let id = trace_node_id_for_locator(self.flow_run_id, &locator);
                by_receipt
                    .get(&id)
                    .map(|&index| (identity.execution_id, index))
            })
            .collect();
        let mut rounds: BTreeMap<(Uuid, u64), Vec<&domain::NodeRunRecord>> = BTreeMap::new();
        for record in records {
            let Some(identity) = record.execution_identity.as_ref() else {
                continue;
            };
            let Some(round) = identity.round_path.last() else {
                continue;
            };
            if by_execution.contains_key(&round.container_execution_id) {
                rounds
                    .entry((round.container_execution_id, round.round_index))
                    .or_default()
                    .push(record);
            }
        }
        for ((container_id, round_index), children) in rounds {
            let parent_index = by_execution[&container_id];
            let parent = &self.nodes[parent_index];
            let locator = format!("{}/round:{round_index}", parent.stable_locator);
            let id = trace_node_id_for_locator(self.flow_run_id, &locator);
            let mut group = parent.clone();
            group.trace_node_id = id;
            group.parent_trace_node_id = Some(parent.trace_node_id);
            group.stable_locator = locator;
            group.node_kind = "round_group".into();
            group.owner_kind = Some("container_round".into());
            group.owner_id = Some(format!("{container_id}:{round_index}"));
            group.order_key = format!("{}/round:{round_index:020}", parent.order_key);
            group.node_alias = format!("#{}", round_index + 1);
            group.node_type = Some("rounds".into());
            group.node_id = None;
            group.has_children = true;
            group.child_count = children.len() as i64;
            group.has_content = false;
            group.metrics_payload = serde_json::json!({});
            group.started_at = children.iter().map(|node| node.started_at).min().unwrap();
            group.finished_at = children
                .iter()
                .all(|node| node.finished_at.is_some())
                .then(|| children.iter().filter_map(|node| node.finished_at).max())
                .flatten();
            group.duration_ms = trace_node_duration_ms(group.started_at, group.finished_at);
            group.status = if children
                .iter()
                .any(|node| node.status == domain::NodeRunStatus::Failed)
            {
                "failed"
            } else if children.iter().any(|node| node.finished_at.is_none()) {
                "running"
            } else if children
                .iter()
                .all(|node| node.status == domain::NodeRunStatus::Succeeded)
            {
                "succeeded"
            } else {
                children.last().unwrap().status.as_str()
            }
            .to_owned();
            for child in children {
                if let Some(index) = child
                    .execution_identity
                    .as_ref()
                    .and_then(|identity| by_execution.get(&identity.execution_id))
                {
                    self.nodes[*index].parent_trace_node_id = Some(id);
                }
            }
            self.nodes[parent_index].has_children = true;
            self.nodes[parent_index].child_count += 1;
            self.nodes.push(group);
        }
    }
}
