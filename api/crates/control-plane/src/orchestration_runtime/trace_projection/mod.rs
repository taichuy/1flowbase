use std::collections::{HashMap, HashSet};

use anyhow::Result;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::ports::{
    ApplicationRunTraceNodeContentProjectionInput, ApplicationRunTraceNodeProjectionInput,
    ReplaceApplicationRunTraceProjectionInput,
};

pub const APPLICATION_RUN_TRACE_PROJECTION_VERSION: i32 = 12;

pub fn trace_node_id_for_locator(flow_run_id: Uuid, stable_locator: &str) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"1flowbase.application_run_trace_node.v1");
    hasher.update(flow_run_id.as_bytes());
    hasher.update(stable_locator.as_bytes());

    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

pub fn legacy_locator_component(
    source_path: &str,
    order_key: &str,
    source_payload: &serde_json::Value,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"1flowbase.trace_legacy_locator.v1");
    hasher.update(source_path.as_bytes());
    hasher.update(order_key.as_bytes());
    hasher.update(source_payload.to_string().as_bytes());
    let digest = hasher.finalize();
    format!(
        "legacy:{:x}",
        &digest[..8]
            .iter()
            .fold(0_u64, |acc, byte| { (acc << 8) | u64::from(*byte) })
    )
}

pub fn build_application_run_trace_projection(
    detail: &domain::ApplicationRunDetail,
) -> Result<ReplaceApplicationRunTraceProjectionInput> {
    let source_watermark = trace_projection_source_watermark(detail);
    let mut builder = TraceProjectionBuilder::new(detail.flow_run.id, source_watermark);
    let current_node_groups = trace_visible_current_node_run_groups(detail);
    let stitched_context_target_index = stitched_context_target_index(detail, &current_node_groups);

    for (index, node_runs) in current_node_groups.iter().enumerate() {
        let stitched_trace = if stitched_context_target_index == Some(index) {
            detail.stitched_trace.as_slice()
        } else {
            &[]
        };
        builder.push_node_run_root(index, node_runs, detail, stitched_trace)?;
    }

    if !detail.stitched_trace.is_empty() && stitched_context_target_index.is_none() {
        builder.push_stitched_context_group(current_node_groups.len(), &detail.stitched_trace)?;
    }

    Ok(builder.finish())
}

pub fn projection_status_needs_lazy_rebuild(
    status: Option<&domain::ApplicationRunTraceProjectionStatusRecord>,
    current_source_watermark: &str,
) -> bool {
    let Some(status) = status else {
        return true;
    };

    if status.projection_version != APPLICATION_RUN_TRACE_PROJECTION_VERSION {
        return true;
    }

    match status.status {
        domain::ApplicationRunTraceProjectionStatus::Succeeded => {
            status.source_watermark != current_source_watermark
        }
        domain::ApplicationRunTraceProjectionStatus::Stale
        | domain::ApplicationRunTraceProjectionStatus::Partial => true,
        domain::ApplicationRunTraceProjectionStatus::Pending
        | domain::ApplicationRunTraceProjectionStatus::Running
        | domain::ApplicationRunTraceProjectionStatus::Failed => false,
    }
}

pub fn trace_projection_source_watermark(detail: &domain::ApplicationRunDetail) -> String {
    trace_projection_source_watermark_from_counts(
        detail.flow_run.updated_at,
        detail.node_runs.len(),
        detail.callback_tasks.len(),
        detail.events.len(),
        detail.stitched_trace.len(),
        detail.subagent_traces.len(),
    )
}

pub fn trace_projection_source_watermark_from_counts(
    flow_run_updated_at: OffsetDateTime,
    node_run_count: usize,
    callback_task_count: usize,
    event_count: usize,
    stitched_trace_count: usize,
    subagent_trace_count: usize,
) -> String {
    format!(
        "flow_run_updated_at:{}/node_runs:{}/callback_tasks:{}/events:{}/stitched:{}/subagents:{}",
        flow_run_updated_at.unix_timestamp_nanos(),
        node_run_count,
        callback_task_count,
        event_count,
        stitched_trace_count,
        subagent_trace_count
    )
}

fn trace_visible_node_runs(node_runs: &[domain::NodeRunRecord]) -> Vec<domain::NodeRunRecord> {
    node_runs
        .iter()
        .filter(|node_run| !is_legacy_waiting_answer_snapshot_node_run(node_run))
        .cloned()
        .collect()
}

fn trace_visible_current_node_run_groups(
    detail: &domain::ApplicationRunDetail,
) -> Vec<Vec<domain::NodeRunRecord>> {
    trace_visible_node_run_groups(&detail.node_runs)
}

fn trace_visible_node_run_groups(
    node_runs: &[domain::NodeRunRecord],
) -> Vec<Vec<domain::NodeRunRecord>> {
    let mut groups = Vec::<Vec<domain::NodeRunRecord>>::new();
    let mut llm_group_index_by_node = HashMap::<(Uuid, String), usize>::new();

    for node_run in trace_visible_node_runs(node_runs) {
        if node_run.node_type != "llm" {
            groups.push(vec![node_run]);
            continue;
        }

        let group_key = (node_run.flow_run_id, node_run.node_id.clone());
        if let Some(group_index) = llm_group_index_by_node.get(&group_key).copied() {
            groups[group_index].push(node_run);
            continue;
        }

        llm_group_index_by_node.insert(group_key, groups.len());
        groups.push(vec![node_run]);
    }

    groups
}

fn stitched_context_target_index(
    detail: &domain::ApplicationRunDetail,
    current_node_groups: &[Vec<domain::NodeRunRecord>],
) -> Option<usize> {
    current_node_groups
        .iter()
        .position(|node_runs| {
            node_runs.first().is_some_and(|node_run| {
                node_run.node_type == "llm"
                    && detail
                        .flow_run
                        .target_node_id
                        .as_deref()
                        .is_some_and(|target_node_id| target_node_id == node_run.node_id)
            })
        })
        .or_else(|| {
            current_node_groups.iter().position(|node_runs| {
                node_runs
                    .first()
                    .is_some_and(|node_run| node_run.node_type == "llm")
            })
        })
}

fn is_legacy_waiting_answer_snapshot_node_run(node_run: &domain::NodeRunRecord) -> bool {
    if node_run.node_type != "answer" {
        return false;
    }
    let input_marker = node_run
        .input_payload
        .get("presentation")
        .and_then(serde_json::Value::as_object)
        .and_then(|presentation| presentation.get("materialized_from"))
        .and_then(serde_json::Value::as_str);
    let debug_marker = node_run
        .debug_payload
        .get("answer_presentation")
        .and_then(serde_json::Value::as_object)
        .and_then(|presentation| presentation.get("materialized_from"))
        .and_then(serde_json::Value::as_str);

    [input_marker, debug_marker]
        .into_iter()
        .flatten()
        .any(|marker| matches!(marker, "waiting_prefix" | "canonical_stream_state"))
}

fn trace_node_duration_ms(
    started_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
) -> Option<i64> {
    finished_at.map(|finished| {
        (finished - started_at)
            .whole_milliseconds()
            .max(0)
            .try_into()
            .unwrap_or(i64::MAX)
    })
}

struct TraceProjectionBuilder {
    flow_run_id: Uuid,
    source_watermark: String,
    nodes: Vec<ApplicationRunTraceNodeProjectionInput>,
    contents: Vec<ApplicationRunTraceNodeContentProjectionInput>,
}

struct ToolCallProjection<'a> {
    task: &'a domain::CallbackTaskRecord,
    tool_call: serde_json::Value,
}

struct SubagentNodeRunProjectionContext<'a> {
    order_key: String,
    parent_trace_node_id: Uuid,
    parent_stable_locator: &'a str,
    node_alias: &'a str,
    parent_tool_call_description: Option<&'a str>,
    subagent_trace: &'a domain::ApplicationRunSubagentTrace,
}

impl TraceProjectionBuilder {
    fn new(flow_run_id: Uuid, source_watermark: String) -> Self {
        Self {
            flow_run_id,
            source_watermark,
            nodes: Vec::new(),
            contents: Vec::new(),
        }
    }

    fn finish(self) -> ReplaceApplicationRunTraceProjectionInput {
        ReplaceApplicationRunTraceProjectionInput {
            flow_run_id: self.flow_run_id,
            projection_version: APPLICATION_RUN_TRACE_PROJECTION_VERSION,
            source_watermark: self.source_watermark,
            nodes: self.nodes,
            contents: self.contents,
        }
    }

    fn push_node_run_root(
        &mut self,
        index: usize,
        node_runs: &[domain::NodeRunRecord],
        detail: &domain::ApplicationRunDetail,
        stitched_trace: &[domain::ApplicationRunStitchedTrace],
    ) -> Result<()> {
        let first_node_run = &node_runs[0];
        let summary_node_run = merge_node_run_group(node_runs);
        let order_key = root_order_key(index);
        let stable_locator = if node_runs.len() == 1 {
            format!("run:{}/node:{}", self.flow_run_id, first_node_run.id)
        } else {
            format!("run:{}/node_group:{}", self.flow_run_id, first_node_run.id)
        };
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let node_run_ids = node_runs
            .iter()
            .map(|node_run| node_run.id)
            .collect::<HashSet<_>>();
        let callback_tasks = callback_tasks_for_node_run_ids(detail, &node_run_ids);
        let tool_tasks: Vec<&domain::CallbackTaskRecord> = callback_tasks
            .iter()
            .filter(|task| task.callback_kind == "llm_tool_calls")
            .collect();
        let synthetic_tool_calls =
            synthetic_tool_calls_not_in_callback_tasks(node_runs, &tool_tasks);
        let linked_subagent_count = count_linked_subagent_tool_calls(detail, &tool_tasks);
        let total_callback_tool_call_count = tool_tasks
            .iter()
            .flat_map(|task| tool_calls_from_callback_task(task))
            .count();
        let ordinary_tool_call_count = total_callback_tool_call_count
            .saturating_sub(linked_subagent_count)
            + synthetic_tool_calls.len();
        let non_tool_callback_count = callback_tasks
            .iter()
            .filter(|task| task.callback_kind != "llm_tool_calls")
            .count();
        let child_group_count = usize::from(!stitched_trace.is_empty())
            + usize::from(ordinary_tool_call_count > 0)
            + usize::from(linked_subagent_count > 0);
        let child_count =
            i64::try_from(non_tool_callback_count + child_group_count).unwrap_or(i64::MAX);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: None,
            stable_locator: stable_locator.clone(),
            node_kind: "node_run".to_string(),
            owner_kind: Some(if node_runs.len() == 1 {
                "node_run".to_string()
            } else {
                "node_run_group".to_string()
            }),
            owner_id: Some(first_node_run.id.to_string()),
            order_key: order_key.clone(),
            node_id: Some(first_node_run.node_id.clone()),
            node_type: Some(first_node_run.node_type.clone()),
            node_mode: None,
            node_alias: first_node_run.node_alias.clone(),
            status: summary_node_run.status.as_str().to_string(),
            started_at: first_node_run.started_at,
            finished_at: summary_node_run.finished_at,
            duration_ms: trace_node_group_duration_ms(node_runs),
            metrics_payload: summary_node_run.metrics_payload.clone(),
            has_children: child_count > 0,
            child_count,
            has_content: true,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        self.contents
            .push(node_run_group_content(trace_node_id, node_runs, detail)?);

        self.push_callback_children(
            &order_key,
            trace_node_id,
            &stable_locator,
            node_runs,
            &callback_tasks,
            detail,
            stitched_trace,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn push_callback_children(
        &mut self,
        parent_order_key: &str,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        parent_node_runs: &[domain::NodeRunRecord],
        callback_tasks: &[domain::CallbackTaskRecord],
        detail: &domain::ApplicationRunDetail,
        stitched_trace: &[domain::ApplicationRunStitchedTrace],
    ) -> Result<()> {
        let mut child_index = 0_usize;
        let tool_tasks: Vec<&domain::CallbackTaskRecord> = callback_tasks
            .iter()
            .filter(|task| task.callback_kind == "llm_tool_calls")
            .collect();
        let synthetic_tool_calls =
            synthetic_tool_calls_not_in_callback_tasks(parent_node_runs, &tool_tasks);
        let ordinary_tool_calls = ordinary_tool_calls_not_linked_to_subagents(detail, &tool_tasks);
        let linked_subagent_traces = linked_subagent_traces_for_tool_tasks(detail, &tool_tasks);

        if !stitched_trace.is_empty() {
            child_index += 1;
            self.push_stitched_context_child(
                child_order_key(parent_order_key, child_index),
                parent_trace_node_id,
                parent_stable_locator,
                stitched_trace,
            )?;
        }

        if !ordinary_tool_calls.is_empty() {
            child_index += 1;
            self.push_tool_group(
                child_order_key(parent_order_key, child_index),
                parent_trace_node_id,
                parent_stable_locator,
                parent_node_runs,
                &ordinary_tool_calls,
                &synthetic_tool_calls,
            )?;
        } else if !synthetic_tool_calls.is_empty() {
            child_index += 1;
            self.push_synthetic_tool_group(
                child_order_key(parent_order_key, child_index),
                parent_trace_node_id,
                parent_stable_locator,
                parent_node_runs,
                &synthetic_tool_calls,
            )?;
        }

        if !linked_subagent_traces.is_empty() {
            child_index += 1;
            self.push_agent_group(
                child_order_key(parent_order_key, child_index),
                parent_trace_node_id,
                parent_stable_locator,
                detail,
                &linked_subagent_traces,
            )?;
        }

        for task in callback_tasks
            .iter()
            .filter(|task| task.callback_kind != "llm_tool_calls")
        {
            child_index += 1;
            self.push_callback_task_node(
                child_order_key(parent_order_key, child_index),
                parent_trace_node_id,
                parent_stable_locator,
                task,
            )?;
        }

        Ok(())
    }

    fn push_tool_group(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        parent_node_runs: &[domain::NodeRunRecord],
        tool_calls: &[ToolCallProjection<'_>],
        synthetic_tool_calls: &[serde_json::Value],
    ) -> Result<()> {
        let stable_locator = format!("{parent_stable_locator}/tools");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let tool_call_count = tool_calls.len() + synthetic_tool_calls.len();

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "tool_group".to_string(),
            owner_kind: Some("node_run_tools".to_string()),
            owner_id: Some(parent_trace_node_id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("tools".to_string()),
            node_mode: None,
            node_alias: "Tools".to_string(),
            status: tool_group_status(
                &tool_calls
                    .iter()
                    .map(|tool_call| tool_call.task)
                    .collect::<Vec<_>>(),
            ),
            started_at: tool_calls
                .iter()
                .map(|tool_call| tool_call.task.created_at)
                .min()
                .unwrap_or(OffsetDateTime::UNIX_EPOCH),
            finished_at: tool_calls
                .iter()
                .filter_map(|tool_call| tool_call.task.completed_at)
                .max(),
            duration_ms: None,
            metrics_payload: serde_json::json!({}),
            has_children: tool_call_count > 0,
            child_count: i64::try_from(tool_call_count).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });

        let mut tool_index = 0_usize;
        for tool_call in tool_calls {
            tool_index += 1;
            self.push_tool_callback_node(
                child_order_key(&order_key, tool_index),
                trace_node_id,
                &stable_locator,
                parent_node_runs,
                tool_call.task,
                &tool_call.tool_call,
            )?;
        }
        for tool_call in synthetic_tool_calls {
            tool_index += 1;
            self.push_synthetic_tool_callback_node(
                child_order_key(&order_key, tool_index),
                trace_node_id,
                &stable_locator,
                parent_node_runs,
                tool_call,
            )?;
        }

        Ok(())
    }

    fn push_agent_group(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        detail: &domain::ApplicationRunDetail,
        subagent_traces: &[&domain::ApplicationRunSubagentTrace],
    ) -> Result<()> {
        let stable_locator = format!("{parent_stable_locator}/agents");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let child_count = subagent_traces.len();

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "agent_group".to_string(),
            owner_kind: Some("node_run_agents".to_string()),
            owner_id: Some(parent_trace_node_id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("agents".to_string()),
            node_mode: None,
            node_alias: "Agents".to_string(),
            status: subagent_group_status(subagent_traces),
            started_at: subagent_traces
                .iter()
                .map(|trace| trace.source_flow_run.started_at)
                .min()
                .unwrap_or(OffsetDateTime::UNIX_EPOCH),
            finished_at: subagent_group_finished_at(subagent_traces),
            duration_ms: None,
            metrics_payload: serde_json::json!({}),
            has_children: child_count > 0,
            child_count: i64::try_from(child_count).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });

        let mut subagent_index = 0_usize;
        for subagent_trace in subagent_traces {
            subagent_index += 1;
            let parent_tool_call_description =
                subagent_parent_tool_call_description(detail, subagent_trace);
            let node_alias = subagent_display_alias(parent_tool_call_description.as_deref());
            if let Some(node_runs) = subagent_primary_node_run_group(subagent_trace) {
                self.push_subagent_node_run(
                    SubagentNodeRunProjectionContext {
                        order_key: child_order_key(&order_key, subagent_index),
                        parent_trace_node_id: trace_node_id,
                        parent_stable_locator: &stable_locator,
                        node_alias: &node_alias,
                        parent_tool_call_description: parent_tool_call_description.as_deref(),
                        subagent_trace,
                    },
                    &node_runs,
                )?;
            } else {
                self.push_subagent_flow_run_fallback(
                    child_order_key(&order_key, subagent_index),
                    trace_node_id,
                    &stable_locator,
                    &node_alias,
                    parent_tool_call_description.as_deref(),
                    subagent_trace,
                )?;
            }
        }

        Ok(())
    }

    fn push_subagent_flow_run_fallback(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        node_alias: &str,
        parent_tool_call_description: Option<&str>,
        subagent_trace: &domain::ApplicationRunSubagentTrace,
    ) -> Result<()> {
        let source_run = &subagent_trace.source_flow_run;
        let stable_locator = format!(
            "{parent_stable_locator}/agent:{}/run:{}/flow-run",
            subagent_trace.parent_tool_call_id, source_run.id
        );
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "node_run".to_string(),
            owner_kind: Some("subagent_flow_run".to_string()),
            owner_id: Some(source_run.id.to_string()),
            order_key,
            node_id: None,
            node_type: Some("llm".to_string()),
            node_mode: None,
            node_alias: node_alias.to_string(),
            status: source_run.status.as_str().to_string(),
            started_at: source_run.started_at,
            finished_at: source_run.finished_at,
            duration_ms: trace_node_duration_ms(source_run.started_at, source_run.finished_at),
            metrics_payload: serde_json::json!({}),
            has_children: false,
            child_count: 0,
            has_content: true,
            content_ref: None,
            source_flow_run_id: Some(source_run.id),
            source_trace_node_id: None,
            parent_callback_task_id: Some(subagent_trace.parent_callback_task_id),
            parent_tool_call_id: Some(subagent_trace.parent_tool_call_id.clone()),
            trace_relation_kind: Some("subagent".to_string()),
        });
        self.contents.push(subagent_flow_run_fallback_content(
            trace_node_id,
            subagent_trace,
            parent_tool_call_description,
        )?);

        Ok(())
    }

    fn push_subagent_node_run(
        &mut self,
        context: SubagentNodeRunProjectionContext<'_>,
        node_runs: &[domain::NodeRunRecord],
    ) -> Result<()> {
        let SubagentNodeRunProjectionContext {
            order_key,
            parent_trace_node_id,
            parent_stable_locator,
            node_alias,
            parent_tool_call_description,
            subagent_trace,
        } = context;
        let first_node_run = &node_runs[0];
        let summary_node_run = merge_node_run_group(node_runs);
        let stable_locator = format!(
            "{parent_stable_locator}/agent:{}/run:{}/node:{}",
            subagent_trace.parent_tool_call_id,
            subagent_trace.source_flow_run.id,
            first_node_run.id
        );
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let source_stable_locator = if node_runs.len() == 1 {
            format!(
                "run:{}/node:{}",
                subagent_trace.source_flow_run.id, first_node_run.id
            )
        } else {
            format!(
                "run:{}/node_group:{}",
                subagent_trace.source_flow_run.id, first_node_run.id
            )
        };
        let source_trace_node_id =
            trace_node_id_for_locator(subagent_trace.source_flow_run.id, &source_stable_locator);
        let node_run_ids = node_runs
            .iter()
            .map(|node_run| node_run.id)
            .collect::<HashSet<_>>();
        let callback_tasks = subagent_trace
            .callback_tasks
            .iter()
            .filter(|task| node_run_ids.contains(&task.node_run_id))
            .cloned()
            .collect::<Vec<_>>();
        let tool_tasks = callback_tasks
            .iter()
            .filter(|task| task.callback_kind == "llm_tool_calls")
            .collect::<Vec<_>>();
        let synthetic_tool_calls =
            synthetic_tool_calls_not_in_callback_tasks(node_runs, &tool_tasks);
        let total_tool_call_count = tool_tasks
            .iter()
            .flat_map(|task| tool_calls_from_callback_task(task))
            .count()
            + synthetic_tool_calls.len();
        let child_count = i64::from(total_tool_call_count > 0);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "node_run".to_string(),
            owner_kind: Some(if node_runs.len() == 1 {
                "subagent_node_run".to_string()
            } else {
                "subagent_node_run_group".to_string()
            }),
            owner_id: Some(first_node_run.id.to_string()),
            order_key: order_key.clone(),
            node_id: Some(first_node_run.node_id.clone()),
            node_type: Some(first_node_run.node_type.clone()),
            node_mode: None,
            node_alias: node_alias.to_string(),
            status: subagent_trace.source_flow_run.status.as_str().to_string(),
            started_at: first_node_run.started_at,
            finished_at: summary_node_run.finished_at,
            duration_ms: trace_node_group_duration_ms(node_runs),
            metrics_payload: summary_node_run.metrics_payload.clone(),
            has_children: child_count > 0,
            child_count,
            has_content: true,
            content_ref: None,
            source_flow_run_id: Some(subagent_trace.source_flow_run.id),
            source_trace_node_id: Some(source_trace_node_id),
            parent_callback_task_id: Some(subagent_trace.parent_callback_task_id),
            parent_tool_call_id: Some(subagent_trace.parent_tool_call_id.clone()),
            trace_relation_kind: Some("subagent".to_string()),
        });
        self.contents.push(subagent_node_run_group_content(
            trace_node_id,
            node_runs,
            subagent_trace,
            parent_tool_call_description,
        )?);

        if total_tool_call_count > 0 {
            let tool_calls = tool_tasks
                .iter()
                .flat_map(|task| {
                    tool_calls_from_callback_task(task)
                        .into_iter()
                        .map(|tool_call| ToolCallProjection { task, tool_call })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            self.push_tool_group(
                child_order_key(&order_key, 1),
                trace_node_id,
                &stable_locator,
                node_runs,
                &tool_calls,
                &synthetic_tool_calls,
            )?;
        }

        Ok(())
    }

    fn push_tool_callback_node(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        parent_node_runs: &[domain::NodeRunRecord],
        task: &domain::CallbackTaskRecord,
        tool_call: &serde_json::Value,
    ) -> Result<()> {
        let tool_call_id = tool_call
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| legacy_locator_component("tool_call", &order_key, tool_call));
        let tool_name = tool_call
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| tool_call_id.clone());
        let stable_locator = format!("{parent_stable_locator}/tool:{tool_call_id}");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let tool_result = tool_result_for_call(task, &tool_call_id);
        let route_trace = route_trace_for_tool_call(parent_node_runs, &tool_call_id);
        let metrics_payload =
            tool_callback_metrics_payload(tool_call, tool_result.as_ref(), route_trace.as_ref());
        let node_mode = route_trace
            .as_ref()
            .map(|trace| route_trace_node_kind(trace).to_string());
        let payload = tool_callback_content_payload(
            Some(task),
            &tool_call_id,
            &tool_name,
            tool_call,
            tool_result.as_ref(),
            route_trace.as_ref(),
        );
        let has_route_child = route_trace.is_some();

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "tool_callback".to_string(),
            owner_kind: Some("tool_call".to_string()),
            owner_id: Some(tool_call_id.clone()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("tool".to_string()),
            node_mode,
            node_alias: tool_name,
            status: route_trace_tool_callback_status(route_trace.as_ref())
                .unwrap_or_else(|| callback_task_trace_node_status(task)),
            started_at: task.created_at,
            finished_at: task.completed_at,
            duration_ms: trace_node_duration_ms(task.created_at, task.completed_at),
            metrics_payload,
            has_children: has_route_child,
            child_count: i64::from(has_route_child),
            has_content: true,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        self.contents
            .push(ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id,
                content_kind: "tool_callback".to_string(),
                payload,
                source_refs: serde_json::json!([{
                    "source_kind": "callback_task",
                    "source_locator": task.id
                }]),
            });
        if let Some(route_trace) = route_trace.as_ref() {
            self.push_tool_route_node(
                child_order_key(&order_key, 1),
                trace_node_id,
                &stable_locator,
                task.created_at,
                task.completed_at,
                route_trace,
            )?;
        }

        Ok(())
    }

    fn push_synthetic_tool_group(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        parent_node_runs: &[domain::NodeRunRecord],
        tool_calls: &[serde_json::Value],
    ) -> Result<()> {
        let stable_locator = format!("{parent_stable_locator}/tools");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let first_node_run = &parent_node_runs[0];

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "tool_group".to_string(),
            owner_kind: Some("node_run_tools".to_string()),
            owner_id: Some(parent_trace_node_id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("tools".to_string()),
            node_mode: None,
            node_alias: "Tools".to_string(),
            status: trace_node_group_status(parent_node_runs)
                .as_str()
                .to_string(),
            started_at: first_node_run.started_at,
            finished_at: trace_node_group_finished_at(parent_node_runs),
            duration_ms: None,
            metrics_payload: serde_json::json!({}),
            has_children: true,
            child_count: i64::try_from(tool_calls.len()).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });

        for (index, tool_call) in tool_calls.iter().enumerate() {
            self.push_synthetic_tool_callback_node(
                child_order_key(&order_key, index + 1),
                trace_node_id,
                &stable_locator,
                parent_node_runs,
                tool_call,
            )?;
        }

        Ok(())
    }

    fn push_synthetic_tool_callback_node(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        parent_node_runs: &[domain::NodeRunRecord],
        tool_call: &serde_json::Value,
    ) -> Result<()> {
        let tool_call_id = tool_call
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| legacy_locator_component("tool_call", &order_key, tool_call));
        let tool_name = tool_call
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| tool_call_id.clone());
        let stable_locator = format!("{parent_stable_locator}/tool:{tool_call_id}");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let route_trace = route_trace_for_tool_call(parent_node_runs, &tool_call_id);
        let tool_result = tool_result_for_call_from_node_runs(parent_node_runs, &tool_call_id);
        let metrics_payload =
            tool_callback_metrics_payload(tool_call, tool_result.as_ref(), route_trace.as_ref());
        let node_mode = route_trace
            .as_ref()
            .map(|trace| route_trace_node_kind(trace).to_string());
        let payload = tool_callback_content_payload(
            None,
            &tool_call_id,
            &tool_name,
            tool_call,
            tool_result.as_ref(),
            route_trace.as_ref(),
        );
        let has_route_child = route_trace.is_some();

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "tool_callback".to_string(),
            owner_kind: Some("tool_call".to_string()),
            owner_id: Some(tool_call_id.clone()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("tool".to_string()),
            node_mode,
            node_alias: tool_name,
            status: route_trace_tool_callback_status(route_trace.as_ref()).unwrap_or_else(|| {
                trace_node_group_status(parent_node_runs)
                    .as_str()
                    .to_string()
            }),
            started_at: parent_node_runs[0].started_at,
            finished_at: trace_node_group_finished_at(parent_node_runs),
            duration_ms: None,
            metrics_payload,
            has_children: has_route_child,
            child_count: i64::from(has_route_child),
            has_content: true,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        self.contents
            .push(ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id,
                content_kind: "tool_callback".to_string(),
                payload,
                source_refs: serde_json::json!([{
                    "source_kind": "node_run_tool_call",
                    "source_locator": tool_call_id
                }]),
            });
        if let Some(route_trace) = route_trace.as_ref() {
            self.push_tool_route_node(
                child_order_key(&order_key, 1),
                trace_node_id,
                &stable_locator,
                parent_node_runs[0].started_at,
                trace_node_group_finished_at(parent_node_runs),
                route_trace,
            )?;
        }

        Ok(())
    }

    fn push_tool_route_node(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        started_at: OffsetDateTime,
        finished_at: Option<OffsetDateTime>,
        route_trace: &serde_json::Value,
    ) -> Result<()> {
        let node_kind = route_trace_node_kind(route_trace).to_string();
        let locator_component = route_trace_locator_component(route_trace, &node_kind, &order_key);
        let stable_locator = format!("{parent_stable_locator}/{node_kind}:{locator_component}");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let branch_traces = route_trace_branch_traces(route_trace);
        let child_count = i64::try_from(branch_traces.len()).unwrap_or(i64::MAX);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: node_kind.clone(),
            owner_kind: Some(node_kind.clone()),
            owner_id: Some(locator_component.clone()),
            order_key: order_key.clone(),
            node_id: route_trace
                .get("route_id")
                .or_else(|| route_trace.get("node_id"))
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned),
            node_type: Some(node_kind.clone()),
            node_mode: None,
            node_alias: route_trace_node_alias(route_trace, &node_kind),
            status: route_trace_status(route_trace),
            started_at,
            finished_at,
            duration_ms: trace_node_duration_ms(started_at, finished_at),
            metrics_payload: route_trace_metrics_payload(route_trace),
            has_children: child_count > 0,
            child_count,
            has_content: true,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        self.contents
            .push(ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id,
                content_kind: node_kind,
                payload: route_trace.clone(),
                source_refs: serde_json::json!([{
                    "source_kind": "visible_internal_llm_tool_trace",
                    "source_locator": locator_component
                }]),
            });

        for (index, branch_trace) in branch_traces.iter().enumerate() {
            self.push_route_branch_node(
                child_order_key(&order_key, index + 1),
                trace_node_id,
                &stable_locator,
                started_at,
                finished_at,
                branch_trace,
            )?;
        }

        Ok(())
    }

    fn push_route_branch_node(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        started_at: OffsetDateTime,
        finished_at: Option<OffsetDateTime>,
        branch_trace: &serde_json::Value,
    ) -> Result<()> {
        let locator_component = branch_locator_component(branch_trace, &order_key);
        let stable_locator = format!("{parent_stable_locator}/branch:{locator_component}");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator,
            node_kind: "branch".to_string(),
            owner_kind: Some("branch".to_string()),
            owner_id: Some(locator_component.clone()),
            order_key,
            node_id: branch_trace
                .get("node_id")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned),
            node_type: branch_trace
                .get("node_type")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| Some("branch".to_string())),
            node_mode: None,
            node_alias: branch_trace_node_alias(branch_trace),
            status: branch_trace_status(branch_trace),
            started_at,
            finished_at,
            duration_ms: trace_node_duration_ms(started_at, finished_at),
            metrics_payload: route_trace_metrics_payload(branch_trace),
            has_children: false,
            child_count: 0,
            has_content: true,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        self.contents
            .push(ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id,
                content_kind: "branch".to_string(),
                payload: branch_trace.clone(),
                source_refs: serde_json::json!([{
                    "source_kind": "visible_internal_llm_tool_branch",
                    "source_locator": locator_component
                }]),
            });

        Ok(())
    }

    fn push_callback_task_node(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        task: &domain::CallbackTaskRecord,
    ) -> Result<()> {
        let stable_locator = format!("{parent_stable_locator}/callback_task:{}", task.id);
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator,
            node_kind: "callback_task".to_string(),
            owner_kind: Some("callback_task".to_string()),
            owner_id: Some(task.id.to_string()),
            order_key,
            node_id: None,
            node_type: Some(task.callback_kind.clone()),
            node_mode: None,
            node_alias: task.callback_kind.clone(),
            status: callback_task_trace_node_status(task),
            started_at: task.created_at,
            finished_at: task.completed_at,
            duration_ms: trace_node_duration_ms(task.created_at, task.completed_at),
            metrics_payload: serde_json::json!({}),
            has_children: false,
            child_count: 0,
            has_content: true,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        self.contents
            .push(ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id,
                content_kind: "callback_task".to_string(),
                payload: serde_json::to_value(task)?,
                source_refs: serde_json::json!([{
                    "source_kind": "callback_task",
                    "source_locator": task.id
                }]),
            });

        Ok(())
    }

    fn push_stitched_context_group(
        &mut self,
        root_index: usize,
        stitched_trace: &[domain::ApplicationRunStitchedTrace],
    ) -> Result<()> {
        let order_key = root_order_key(root_index);
        self.push_stitched_context_node(
            order_key,
            None,
            format!("run:{}/stitched_context", self.flow_run_id),
            stitched_trace,
        )
    }

    fn push_stitched_context_child(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        stitched_trace: &[domain::ApplicationRunStitchedTrace],
    ) -> Result<()> {
        self.push_stitched_context_node(
            order_key,
            Some(parent_trace_node_id),
            format!("{parent_stable_locator}/stitched_context"),
            stitched_trace,
        )
    }

    fn push_stitched_context_node(
        &mut self,
        order_key: String,
        parent_trace_node_id: Option<Uuid>,
        stable_locator: String,
        stitched_trace: &[domain::ApplicationRunStitchedTrace],
    ) -> Result<()> {
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id,
            stable_locator: stable_locator.clone(),
            node_kind: "stitched_context".to_string(),
            owner_kind: Some("stitched_context".to_string()),
            owner_id: Some(self.flow_run_id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("stitched_context".to_string()),
            node_mode: None,
            node_alias: "Stitched context".to_string(),
            status: "succeeded".to_string(),
            started_at: stitched_trace
                .iter()
                .map(|trace| trace.source_flow_run.started_at)
                .min()
                .unwrap_or(OffsetDateTime::UNIX_EPOCH),
            finished_at: stitched_trace
                .iter()
                .filter_map(|trace| trace.source_flow_run.finished_at)
                .max(),
            duration_ms: None,
            metrics_payload: serde_json::json!({}),
            has_children: true,
            child_count: i64::try_from(stitched_trace.len()).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });

        for (index, trace) in stitched_trace.iter().enumerate() {
            self.push_stitched_run_summary(
                child_order_key(&order_key, index + 1),
                trace_node_id,
                &stable_locator,
                trace,
            )?;
        }

        Ok(())
    }

    fn push_stitched_run_summary(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        trace: &domain::ApplicationRunStitchedTrace,
    ) -> Result<()> {
        let source_run = &trace.source_flow_run;
        let stable_locator = format!("{parent_stable_locator}/run:{}", source_run.id);
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let node_run_groups = trace_visible_node_run_groups(&trace.node_runs);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "stitched_run".to_string(),
            owner_kind: Some("flow_run".to_string()),
            owner_id: Some(source_run.id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("flow_run".to_string()),
            node_mode: None,
            node_alias: source_run.title.clone(),
            status: source_run.status.as_str().to_string(),
            started_at: source_run.started_at,
            finished_at: source_run.finished_at,
            duration_ms: trace_node_duration_ms(source_run.started_at, source_run.finished_at),
            metrics_payload: serde_json::json!({}),
            has_children: !node_run_groups.is_empty(),
            child_count: i64::try_from(node_run_groups.len()).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: Some(source_run.id),
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });

        for (index, node_runs) in node_run_groups.iter().enumerate() {
            self.push_stitched_node_run_root(
                child_order_key(&order_key, index + 1),
                trace_node_id,
                &stable_locator,
                trace,
                node_runs,
            )?;
        }

        Ok(())
    }

    fn push_stitched_node_run_root(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        trace: &domain::ApplicationRunStitchedTrace,
        node_runs: &[domain::NodeRunRecord],
    ) -> Result<()> {
        let source_run = &trace.source_flow_run;
        let first_node_run = &node_runs[0];
        let summary_node_run = merge_node_run_group(node_runs);
        let stable_locator = if node_runs.len() == 1 {
            format!("{parent_stable_locator}/node:{}", first_node_run.id)
        } else {
            format!("{parent_stable_locator}/node_group:{}", first_node_run.id)
        };
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let source_stable_locator = if node_runs.len() == 1 {
            format!("run:{}/node:{}", source_run.id, first_node_run.id)
        } else {
            format!("run:{}/node_group:{}", source_run.id, first_node_run.id)
        };
        let source_trace_node_id = trace_node_id_for_locator(source_run.id, &source_stable_locator);
        let node_run_ids = node_runs
            .iter()
            .map(|node_run| node_run.id)
            .collect::<HashSet<_>>();
        let callback_tasks = trace
            .callback_tasks
            .iter()
            .filter(|task| node_run_ids.contains(&task.node_run_id))
            .cloned()
            .collect::<Vec<_>>();
        let tool_tasks = callback_tasks
            .iter()
            .filter(|task| task.callback_kind == "llm_tool_calls")
            .collect::<Vec<_>>();
        let synthetic_tool_calls =
            synthetic_tool_calls_not_in_callback_tasks(node_runs, &tool_tasks);
        let total_tool_call_count = tool_tasks
            .iter()
            .flat_map(|task| tool_calls_from_callback_task(task))
            .count()
            + synthetic_tool_calls.len();
        let non_tool_callback_count = callback_tasks
            .iter()
            .filter(|task| task.callback_kind != "llm_tool_calls")
            .count();
        let child_count =
            i64::try_from(non_tool_callback_count + usize::from(total_tool_call_count > 0))
                .unwrap_or(i64::MAX);

        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "node_run".to_string(),
            owner_kind: Some(if node_runs.len() == 1 {
                "stitched_node_run".to_string()
            } else {
                "stitched_node_run_group".to_string()
            }),
            owner_id: Some(first_node_run.id.to_string()),
            order_key: order_key.clone(),
            node_id: Some(first_node_run.node_id.clone()),
            node_type: Some(first_node_run.node_type.clone()),
            node_mode: None,
            node_alias: first_node_run.node_alias.clone(),
            status: summary_node_run.status.as_str().to_string(),
            started_at: first_node_run.started_at,
            finished_at: summary_node_run.finished_at,
            duration_ms: trace_node_group_duration_ms(node_runs),
            metrics_payload: summary_node_run.metrics_payload.clone(),
            has_children: child_count > 0,
            child_count,
            has_content: true,
            content_ref: None,
            source_flow_run_id: Some(source_run.id),
            source_trace_node_id: Some(source_trace_node_id),
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        self.contents.push(stitched_node_run_group_content(
            trace_node_id,
            node_runs,
            trace,
        )?);

        if total_tool_call_count > 0 {
            let tool_calls = tool_tasks
                .iter()
                .flat_map(|task| {
                    tool_calls_from_callback_task(task)
                        .into_iter()
                        .map(|tool_call| ToolCallProjection { task, tool_call })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            self.push_tool_group(
                child_order_key(&order_key, 1),
                trace_node_id,
                &stable_locator,
                node_runs,
                &tool_calls,
                &synthetic_tool_calls,
            )?;
        }

        for (index, task) in callback_tasks
            .iter()
            .filter(|task| task.callback_kind != "llm_tool_calls")
            .enumerate()
        {
            self.push_callback_task_node(
                child_order_key(
                    &order_key,
                    usize::from(total_tool_call_count > 0) + index + 1,
                ),
                trace_node_id,
                &stable_locator,
                task,
            )?;
        }

        Ok(())
    }
}

mod aggregation;
use aggregation::*;

mod tool_callbacks;

pub use tool_callbacks::merge_trace_node_run_detail;
use tool_callbacks::{
    branch_locator_component, branch_trace_node_alias, branch_trace_status,
    callback_task_trace_node_status, callback_tasks_for_node_run_ids, node_run_group_content,
    route_trace_branch_traces, route_trace_for_tool_call, route_trace_locator_component,
    route_trace_metrics_payload, route_trace_node_alias, route_trace_node_kind, route_trace_status,
    route_trace_tool_callback_status, synthetic_tool_calls_not_in_callback_tasks,
    tool_callback_content_payload, tool_callback_metrics_payload, tool_calls_from_callback_task,
    tool_group_status, tool_result_for_call, tool_result_for_call_from_node_runs,
};

fn root_order_key(index: usize) -> String {
    format!("{:06}", index + 1)
}

fn child_order_key(parent_order_key: &str, index: usize) -> String {
    format!("{parent_order_key}/{index:06}")
}

#[cfg(test)]
mod tests;
