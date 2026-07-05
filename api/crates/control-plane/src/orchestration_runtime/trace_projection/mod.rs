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
        .filter(|node_run| !is_waiting_prefix_answer_node_run(node_run))
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

fn is_waiting_prefix_answer_node_run(node_run: &domain::NodeRunRecord) -> bool {
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

    input_marker == Some("waiting_prefix") || debug_marker == Some("waiting_prefix")
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

fn json_object_has_keys(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(|object| !object.is_empty())
}

fn first_non_empty_json(
    node_runs: &[domain::NodeRunRecord],
    selector: impl Fn(&domain::NodeRunRecord) -> &serde_json::Value,
) -> serde_json::Value {
    node_runs
        .iter()
        .find_map(|node_run| {
            let payload = selector(node_run);
            json_object_has_keys(payload).then(|| payload.clone())
        })
        .unwrap_or_else(|| serde_json::json!({}))
}

fn last_non_empty_json(
    node_runs: &[domain::NodeRunRecord],
    selector: impl Fn(&domain::NodeRunRecord) -> &serde_json::Value,
) -> serde_json::Value {
    node_runs
        .iter()
        .rev()
        .find_map(|node_run| {
            let payload = selector(node_run);
            json_object_has_keys(payload).then(|| payload.clone())
        })
        .unwrap_or_else(|| serde_json::json!({}))
}

fn trace_node_group_status(node_runs: &[domain::NodeRunRecord]) -> domain::NodeRunStatus {
    if node_runs
        .iter()
        .any(|node_run| node_run.status == domain::NodeRunStatus::Failed)
    {
        return domain::NodeRunStatus::Failed;
    }

    if node_runs
        .iter()
        .any(|node_run| node_run.status == domain::NodeRunStatus::WaitingHuman)
    {
        return domain::NodeRunStatus::WaitingHuman;
    }

    if node_runs
        .iter()
        .any(|node_run| node_run.status == domain::NodeRunStatus::WaitingCallback)
    {
        return domain::NodeRunStatus::WaitingCallback;
    }

    if node_runs.iter().any(|node_run| {
        matches!(
            node_run.status,
            domain::NodeRunStatus::Running
                | domain::NodeRunStatus::Streaming
                | domain::NodeRunStatus::Retrying
                | domain::NodeRunStatus::WaitingTool
        )
    }) {
        return domain::NodeRunStatus::Running;
    }

    if node_runs
        .iter()
        .all(|node_run| node_run.status == domain::NodeRunStatus::Succeeded)
    {
        return domain::NodeRunStatus::Succeeded;
    }

    node_runs
        .last()
        .map(|node_run| node_run.status)
        .unwrap_or(domain::NodeRunStatus::Running)
}

fn trace_node_group_finished_at(node_runs: &[domain::NodeRunRecord]) -> Option<OffsetDateTime> {
    if node_runs
        .iter()
        .any(|node_run| node_run.finished_at.is_none())
    {
        return None;
    }

    node_runs.last().and_then(|node_run| node_run.finished_at)
}

fn trace_node_group_duration_ms(node_runs: &[domain::NodeRunRecord]) -> Option<i64> {
    let durations: Vec<i64> = node_runs
        .iter()
        .filter_map(|node_run| trace_node_duration_ms(node_run.started_at, node_run.finished_at))
        .collect();

    if durations.is_empty() {
        return None;
    }

    Some(
        durations
            .into_iter()
            .fold(0_i64, |total, duration| total.saturating_add(duration)),
    )
}

fn merge_debug_payloads(node_runs: &[domain::NodeRunRecord]) -> serde_json::Value {
    let mut merged = serde_json::Map::new();
    let mut llm_rounds = Vec::<serde_json::Value>::new();
    let mut visible_internal_route_traces = Vec::<serde_json::Value>::new();
    let mut visible_internal_route_events = Vec::<serde_json::Value>::new();

    for node_run in node_runs {
        let Some(debug_payload) = node_run.debug_payload.as_object() else {
            continue;
        };

        for (key, value) in debug_payload {
            match key.as_str() {
                "llm_rounds" => {
                    if let Some(items) = value.as_array() {
                        llm_rounds.extend(items.iter().cloned());
                    } else if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
                "visible_internal_llm_tool_trace" => {
                    if let Some(items) = value.as_array() {
                        visible_internal_route_traces.extend(items.iter().cloned());
                    } else if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
                "visible_internal_llm_tool_events" => {
                    if let Some(items) = value.as_array() {
                        visible_internal_route_events.extend(items.iter().cloned());
                    } else if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
                _ => {
                    if !merged.contains_key(key) {
                        merged.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }

    if !llm_rounds.is_empty() {
        merged.insert(
            "llm_rounds".to_string(),
            serde_json::Value::Array(llm_rounds),
        );
    }
    if !visible_internal_route_traces.is_empty() {
        merged.insert(
            "visible_internal_llm_tool_trace".to_string(),
            serde_json::Value::Array(visible_internal_route_traces),
        );
    }
    if !visible_internal_route_events.is_empty() {
        merged.insert(
            "visible_internal_llm_tool_events".to_string(),
            serde_json::Value::Array(visible_internal_route_events),
        );
    }

    serde_json::Value::Object(merged)
}

fn merge_metric_usage_value(node_runs: &[domain::NodeRunRecord], usage_key: &str) -> Option<i64> {
    let mut total = None;

    for node_run in node_runs {
        if let Some(value) = node_run
            .metrics_payload
            .get("usage")
            .and_then(|usage| usage.get(usage_key))
            .and_then(serde_json::Value::as_i64)
        {
            total = Some(total.unwrap_or(0_i64).saturating_add(value));
        }
    }

    total
}

fn merge_metrics_payloads(node_runs: &[domain::NodeRunRecord]) -> serde_json::Value {
    let mut usage = serde_json::Map::new();

    for key in [
        "total_tokens",
        "input_tokens",
        "output_tokens",
        "input_cache_hit_tokens",
        "cache_read_tokens",
    ] {
        if let Some(value) = merge_metric_usage_value(node_runs, key) {
            usage.insert(key.to_string(), serde_json::json!(value));
        }
    }

    if usage.is_empty() {
        return last_non_empty_json(node_runs, |node_run| &node_run.metrics_payload);
    }

    serde_json::json!({ "usage": usage })
}

fn merge_node_run_group(node_runs: &[domain::NodeRunRecord]) -> domain::NodeRunRecord {
    let mut merged = node_runs[0].clone();

    if node_runs.len() == 1 {
        return merged;
    }

    merged.status = trace_node_group_status(node_runs);
    merged.finished_at = trace_node_group_finished_at(node_runs);
    merged.input_payload = first_non_empty_json(node_runs, |node_run| &node_run.input_payload);
    merged.output_payload = last_non_empty_json(node_runs, |node_run| &node_run.output_payload);
    merged.error_payload = node_runs
        .iter()
        .rev()
        .find_map(|node_run| node_run.error_payload.clone());
    merged.metrics_payload = merge_metrics_payloads(node_runs);
    merged.debug_payload = merge_debug_payloads(node_runs);

    merged
}

fn tool_call_id(tool_call: &serde_json::Value) -> Option<&str> {
    tool_call
        .get("id")
        .or_else(|| tool_call.get("tool_call_id"))
        .or_else(|| tool_call.get("call_id"))
        .and_then(serde_json::Value::as_str)
}

fn subagent_trace_matches_tool_call(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    task: &domain::CallbackTaskRecord,
    tool_call: &serde_json::Value,
) -> bool {
    subagent_trace.parent_callback_task_id == task.id
        && tool_call_id(tool_call) == Some(subagent_trace.parent_tool_call_id.as_str())
}

fn linked_subagent_trace_for_tool_call<'a>(
    detail: &'a domain::ApplicationRunDetail,
    task: &domain::CallbackTaskRecord,
    tool_call: &serde_json::Value,
) -> Option<&'a domain::ApplicationRunSubagentTrace> {
    detail
        .subagent_traces
        .iter()
        .find(|subagent_trace| subagent_trace_matches_tool_call(subagent_trace, task, tool_call))
}

fn ordinary_tool_calls_not_linked_to_subagents<'a>(
    detail: &'a domain::ApplicationRunDetail,
    tool_tasks: &[&'a domain::CallbackTaskRecord],
) -> Vec<ToolCallProjection<'a>> {
    let mut tool_calls = Vec::new();

    for task in tool_tasks {
        for tool_call in tool_calls_from_callback_task(task) {
            if linked_subagent_trace_for_tool_call(detail, task, &tool_call).is_none() {
                tool_calls.push(ToolCallProjection { task, tool_call });
            }
        }
    }

    tool_calls
}

fn linked_subagent_traces_for_tool_tasks<'a>(
    detail: &'a domain::ApplicationRunDetail,
    tool_tasks: &[&domain::CallbackTaskRecord],
) -> Vec<&'a domain::ApplicationRunSubagentTrace> {
    let mut subagent_traces = Vec::new();
    let mut seen_flow_runs = HashSet::new();

    for task in tool_tasks {
        for tool_call in tool_calls_from_callback_task(task) {
            if let Some(subagent_trace) =
                linked_subagent_trace_for_tool_call(detail, task, &tool_call)
            {
                if seen_flow_runs.insert(subagent_trace.source_flow_run.id) {
                    subagent_traces.push(subagent_trace);
                }
            }
        }
    }

    subagent_traces
}

fn count_linked_subagent_tool_calls(
    detail: &domain::ApplicationRunDetail,
    tool_tasks: &[&domain::CallbackTaskRecord],
) -> usize {
    linked_subagent_traces_for_tool_tasks(detail, tool_tasks).len()
}

fn subagent_primary_node_run_group(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
) -> Option<Vec<domain::NodeRunRecord>> {
    trace_visible_node_run_groups(&subagent_trace.node_runs)
        .into_iter()
        .find(|group| {
            group
                .first()
                .is_some_and(|node_run| node_run.node_type == "llm")
        })
}

fn subagent_group_status(subagent_traces: &[&domain::ApplicationRunSubagentTrace]) -> String {
    if subagent_traces.iter().any(|trace| {
        matches!(
            trace.source_flow_run.status,
            domain::FlowRunStatus::Failed | domain::FlowRunStatus::Cancelled
        )
    }) {
        return domain::NodeRunStatus::Failed.as_str().to_string();
    }

    if subagent_traces.iter().any(|trace| {
        matches!(
            trace.source_flow_run.status,
            domain::FlowRunStatus::Queued
                | domain::FlowRunStatus::Running
                | domain::FlowRunStatus::WaitingCallback
                | domain::FlowRunStatus::WaitingHuman
                | domain::FlowRunStatus::Paused
        )
    }) {
        return domain::NodeRunStatus::Running.as_str().to_string();
    }

    domain::NodeRunStatus::Succeeded.as_str().to_string()
}

fn subagent_group_finished_at(
    subagent_traces: &[&domain::ApplicationRunSubagentTrace],
) -> Option<OffsetDateTime> {
    if subagent_traces
        .iter()
        .any(|trace| trace.source_flow_run.finished_at.is_none())
    {
        return None;
    }

    subagent_traces
        .iter()
        .filter_map(|trace| trace.source_flow_run.finished_at)
        .max()
}

fn subagent_display_alias(parent_tool_call_description: Option<&str>) -> String {
    parent_tool_call_description
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Subagent".to_string())
}

fn subagent_parent_tool_call_description(
    detail: &domain::ApplicationRunDetail,
    subagent_trace: &domain::ApplicationRunSubagentTrace,
) -> Option<String> {
    for task in &detail.callback_tasks {
        if task.id != subagent_trace.parent_callback_task_id {
            continue;
        }
        for tool_call in tool_calls_from_callback_task(task) {
            if tool_call_id(&tool_call) != Some(subagent_trace.parent_tool_call_id.as_str()) {
                continue;
            }

            return tool_call
                .get("arguments")
                .and_then(|arguments| arguments.get("description"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|description| !description.is_empty())
                .map(ToOwned::to_owned);
        }
    }

    None
}

fn subagent_parent_agent_tool_call_debug_payload(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    description: Option<&str>,
) -> serde_json::Value {
    let mut parent_tool_call = serde_json::Map::new();
    parent_tool_call.insert(
        "callback_task_id".to_string(),
        serde_json::json!(subagent_trace.parent_callback_task_id),
    );
    parent_tool_call.insert(
        "tool_call_id".to_string(),
        serde_json::json!(subagent_trace.parent_tool_call_id.clone()),
    );
    if let Some(description) = description {
        parent_tool_call.insert("description".to_string(), serde_json::json!(description));
    }

    let mut debug_payload = serde_json::Map::new();
    debug_payload.insert(
        "parent_agent_tool_call".to_string(),
        serde_json::Value::Object(parent_tool_call),
    );

    serde_json::Value::Object(debug_payload)
}

fn subagent_flow_run_fallback_debug_payload(
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    description: Option<&str>,
) -> serde_json::Value {
    let mut debug_payload =
        subagent_parent_agent_tool_call_debug_payload(subagent_trace, description);
    let Some(debug_payload_object) = debug_payload.as_object_mut() else {
        return debug_payload;
    };
    debug_payload_object.insert(
        "source_flow_run_id".to_string(),
        serde_json::json!(subagent_trace.source_flow_run.id),
    );
    debug_payload_object.insert(
        "runtime_event_count".to_string(),
        serde_json::json!(subagent_trace.runtime_events.len()),
    );

    debug_payload
}

fn subagent_flow_run_fallback_content(
    trace_node_id: Uuid,
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    parent_tool_call_description: Option<&str>,
) -> Result<ApplicationRunTraceNodeContentProjectionInput> {
    let source_run = &subagent_trace.source_flow_run;
    let source_refs = serde_json::json!([{
        "source_kind": "subagent_flow_run",
        "source_locator": source_run.id,
        "source_flow_run_id": source_run.id,
        "parent_callback_task_id": subagent_trace.parent_callback_task_id,
        "parent_tool_call_id": subagent_trace.parent_tool_call_id.clone(),
    }]);
    let detail_refs = serde_json::Value::Array(Vec::new());

    Ok(ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id,
        content_kind: "node_run".to_string(),
        payload: serde_json::json!({
            "payload_index": {
                "node_run_count": 0,
                "checkpoint_count": 0,
                "event_count": subagent_trace.events.len(),
                "node_run_ids": [],
                "source_flow_run_id": source_run.id
            },
            "source_refs": source_refs.clone(),
            "detail_refs": detail_refs,
            "input_payload": source_run.input_payload.clone(),
            "output_payload": source_run.output_payload.clone(),
            "error_payload": source_run.error_payload.clone(),
            "metrics_payload": {},
            "debug_payload": subagent_flow_run_fallback_debug_payload(
                subagent_trace,
                parent_tool_call_description
            )
        }),
        source_refs,
    })
}

fn subagent_node_run_group_content(
    trace_node_id: Uuid,
    node_runs: &[domain::NodeRunRecord],
    subagent_trace: &domain::ApplicationRunSubagentTrace,
    parent_tool_call_description: Option<&str>,
) -> Result<ApplicationRunTraceNodeContentProjectionInput> {
    let primary_node_run = &node_runs[0];
    let source_ref_values = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "source_kind": "subagent_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": subagent_trace.source_flow_run.id,
                "parent_callback_task_id": subagent_trace.parent_callback_task_id,
                "parent_tool_call_id": subagent_trace.parent_tool_call_id,
            })
        })
        .collect::<Vec<_>>();
    let node_run_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "detail_kind": "node_run",
                "source_kind": "subagent_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": subagent_trace.source_flow_run.id,
                "count": 1
            })
        })
        .collect::<Vec<_>>();
    let detail_refs = serde_json::json!([
        {
            "detail_ref_id": "node_run",
            "detail_kind": "node_run",
            "source_kind": "subagent_node_run",
            "source_locator": primary_node_run.id,
            "source_flow_run_id": subagent_trace.source_flow_run.id,
            "count": node_runs.len()
        },
        {
            "detail_ref_id": "checkpoints",
            "detail_kind": "checkpoints",
            "source_kind": "subagent_flow_run_checkpoints",
            "source_locator": trace_node_id,
            "source_flow_run_id": subagent_trace.source_flow_run.id,
            "count": 0
        },
        {
            "detail_ref_id": "events",
            "detail_kind": "events",
            "source_kind": "subagent_flow_run_events",
            "source_locator": trace_node_id,
            "source_flow_run_id": subagent_trace.source_flow_run.id,
            "count": subagent_trace.events.len()
        }
    ]);

    Ok(ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id,
        content_kind: "node_run".to_string(),
        payload: serde_json::json!({
            "payload_index": {
                "node_run_count": node_runs.len(),
                "checkpoint_count": 0,
                "event_count": subagent_trace.events.len(),
                "node_run_ids": node_runs.iter().map(|node_run| node_run.id).collect::<Vec<_>>(),
                "source_flow_run_id": subagent_trace.source_flow_run.id
            },
            "source_refs": source_ref_values.clone(),
            "detail_refs": detail_refs,
            "debug_payload": subagent_parent_agent_tool_call_debug_payload(
                subagent_trace,
                parent_tool_call_description
            ),
            "node_run_refs": node_run_refs
        }),
        source_refs: serde_json::Value::Array(source_ref_values),
    })
}

fn stitched_node_run_group_content(
    trace_node_id: Uuid,
    node_runs: &[domain::NodeRunRecord],
    trace: &domain::ApplicationRunStitchedTrace,
) -> Result<ApplicationRunTraceNodeContentProjectionInput> {
    let node_run_ids = node_runs
        .iter()
        .map(|node_run| node_run.id)
        .collect::<HashSet<_>>();
    let events = trace
        .events
        .iter()
        .filter(|event| {
            event
                .node_run_id
                .is_some_and(|node_run_id| node_run_ids.contains(&node_run_id))
        })
        .count();
    let source_run_id = trace.source_flow_run.id;
    let primary_node_run = &node_runs[0];
    let source_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "source_kind": "stitched_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": source_run_id
            })
        })
        .collect::<Vec<_>>();
    let node_run_refs = node_runs
        .iter()
        .map(|node_run| {
            serde_json::json!({
                "detail_kind": "node_run",
                "source_kind": "stitched_node_run",
                "source_locator": node_run.id,
                "source_flow_run_id": source_run_id,
                "count": 1
            })
        })
        .collect::<Vec<_>>();
    let detail_refs = serde_json::json!([
        {
            "detail_ref_id": "node_run",
            "detail_kind": "node_run",
            "source_kind": "stitched_node_run",
            "source_locator": primary_node_run.id,
            "source_flow_run_id": source_run_id,
            "count": node_runs.len()
        },
        {
            "detail_ref_id": "checkpoints",
            "detail_kind": "checkpoints",
            "source_kind": "stitched_flow_run_checkpoints",
            "source_locator": trace_node_id,
            "source_flow_run_id": source_run_id,
            "count": 0
        },
        {
            "detail_ref_id": "events",
            "detail_kind": "events",
            "source_kind": "stitched_flow_run_events",
            "source_locator": trace_node_id,
            "source_flow_run_id": source_run_id,
            "count": events
        }
    ]);

    Ok(ApplicationRunTraceNodeContentProjectionInput {
        trace_node_id,
        content_kind: "node_run".to_string(),
        payload: serde_json::json!({
            "payload_index": {
                "node_run_count": node_runs.len(),
                "checkpoint_count": 0,
                "event_count": events,
                "node_run_ids": node_runs.iter().map(|node_run| node_run.id).collect::<Vec<_>>(),
                "source_flow_run_id": source_run_id
            },
            "source_refs": source_refs.clone(),
            "detail_refs": detail_refs,
            "node_run_refs": node_run_refs
        }),
        source_refs: serde_json::Value::Array(source_refs),
    })
}

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
