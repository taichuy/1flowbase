use super::*;

/// Task-level groups under the anchor run's LLM node: later member calls as
/// rounds (each carrying its own client tool callbacks) and child tasks the
/// client spawned from this turn. Both reuse the existing group / source-run
/// contract so the console expands them through the lazy children path.
impl TraceProjectionBuilder {
    pub(super) fn push_task_groups(
        &mut self,
        parent_order_key: &str,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        mut child_index: usize,
        detail: &domain::ApplicationRunDetail,
    ) -> Result<usize> {
        if !detail.task_rounds.is_empty() {
            child_index += 1;
            self.push_round_group(
                child_order_key(parent_order_key, child_index),
                parent_trace_node_id,
                parent_stable_locator,
                detail,
            )?;
        }
        if !detail.child_task_traces.is_empty() {
            child_index += 1;
            self.push_child_task_group(
                child_order_key(parent_order_key, child_index),
                parent_trace_node_id,
                parent_stable_locator,
                detail,
            )?;
        }
        Ok(child_index)
    }

    fn push_round_group(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        detail: &domain::ApplicationRunDetail,
    ) -> Result<()> {
        let stable_locator = format!("{parent_stable_locator}/rounds");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let rounds = &detail.task_rounds;
        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "round_group".to_string(),
            owner_kind: Some("task_rounds".to_string()),
            owner_id: Some(self.flow_run_id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("rounds".to_string()),
            node_mode: None,
            node_alias: "Rounds".to_string(),
            status: source_run_group_status(rounds.iter().map(|round| &round.source_flow_run)),
            started_at: rounds
                .iter()
                .map(|round| round.source_flow_run.started_at)
                .min()
                .unwrap_or(OffsetDateTime::UNIX_EPOCH),
            finished_at: source_run_group_finished_at(
                rounds.iter().map(|round| &round.source_flow_run),
            ),
            duration_ms: None,
            metrics_payload: serde_json::json!({}),
            has_children: true,
            child_count: i64::try_from(rounds.len()).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        for (index, round) in rounds.iter().enumerate() {
            self.push_task_round(
                child_order_key(&order_key, index + 1),
                trace_node_id,
                &stable_locator,
                index + 2,
                round,
            )?;
        }
        Ok(())
    }

    fn push_task_round(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        ordinal: usize,
        round: &domain::ApplicationRunTaskRoundTrace,
    ) -> Result<()> {
        let source_run = &round.source_flow_run;
        let stable_locator = format!("{parent_stable_locator}/run:{}", source_run.id);
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let node_run_groups = trace_visible_node_run_groups(&round.node_runs);
        let tool_messages = round_tool_messages(&round.native_messages);
        let is_compaction = round.call_kind == "compact";
        let child_count = node_run_groups.len() + usize::from(!tool_messages.is_empty());
        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "task_round".to_string(),
            owner_kind: Some("flow_run".to_string()),
            owner_id: Some(source_run.id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("flow_run".to_string()),
            node_mode: Some(round.call_kind.clone()),
            node_alias: if is_compaction {
                format!("Compaction {ordinal}")
            } else {
                format!("Round {ordinal}")
            },
            status: source_run.status.as_str().to_string(),
            started_at: source_run.started_at,
            finished_at: source_run.finished_at,
            duration_ms: trace_node_duration_ms(source_run.started_at, source_run.finished_at),
            metrics_payload: serde_json::json!({}),
            has_children: child_count > 0,
            child_count: i64::try_from(child_count).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: Some(source_run.id),
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        let mut index = 0_usize;
        for node_runs in &node_run_groups {
            index += 1;
            self.push_source_run_node_run(
                child_order_key(&order_key, index),
                trace_node_id,
                &stable_locator,
                source_run,
                node_runs,
                &round.callback_tasks,
                "task_round_node_run",
            )?;
        }
        if !tool_messages.is_empty() {
            index += 1;
            self.push_native_tool_group(
                child_order_key(&order_key, index),
                trace_node_id,
                &stable_locator,
                source_run,
                &tool_messages,
            )?;
        }
        Ok(())
    }

    fn push_child_task_group(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        detail: &domain::ApplicationRunDetail,
    ) -> Result<()> {
        let stable_locator = format!("{parent_stable_locator}/child_tasks");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let children = &detail.child_task_traces;
        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "agent_group".to_string(),
            owner_kind: Some("task_child_tasks".to_string()),
            owner_id: Some(self.flow_run_id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("agents".to_string()),
            node_mode: None,
            node_alias: "Agents".to_string(),
            status: source_run_group_status(children.iter().map(|child| &child.source_flow_run)),
            started_at: children
                .iter()
                .map(|child| child.source_flow_run.started_at)
                .min()
                .unwrap_or(OffsetDateTime::UNIX_EPOCH),
            finished_at: source_run_group_finished_at(
                children.iter().map(|child| &child.source_flow_run),
            ),
            duration_ms: None,
            metrics_payload: serde_json::json!({}),
            has_children: true,
            child_count: i64::try_from(children.len()).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: None,
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        for (index, child) in children.iter().enumerate() {
            let source_run = &child.source_flow_run;
            let child_locator = format!("{stable_locator}/run:{}", source_run.id);
            let child_trace_node_id = trace_node_id_for_locator(self.flow_run_id, &child_locator);
            let node_run_groups = trace_visible_node_run_groups(&child.node_runs);
            let child_order = child_order_key(&order_key, index + 1);
            self.nodes.push(ApplicationRunTraceNodeProjectionInput {
                trace_node_id: child_trace_node_id,
                parent_trace_node_id: Some(trace_node_id),
                stable_locator: child_locator.clone(),
                node_kind: "child_task".to_string(),
                owner_kind: Some("flow_run".to_string()),
                owner_id: Some(source_run.id.to_string()),
                order_key: child_order.clone(),
                node_id: None,
                node_type: Some("flow_run".to_string()),
                node_mode: child.subagent_kind.clone(),
                node_alias: match child.subagent_kind.as_deref() {
                    Some(kind) => format!("{kind} · {}", source_run.title),
                    None => source_run.title.clone(),
                },
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
            for (node_index, node_runs) in node_run_groups.iter().enumerate() {
                self.push_source_run_node_run(
                    child_order_key(&child_order, node_index + 1),
                    child_trace_node_id,
                    &child_locator,
                    source_run,
                    node_runs,
                    &child.callback_tasks,
                    "child_task_node_run",
                )?;
            }
        }
        Ok(())
    }

    /// A node-run row that belongs to another run of the same task. Its
    /// content is served lazily from that run's own projection.
    #[allow(clippy::too_many_arguments)]
    fn push_source_run_node_run(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        source_run: &domain::FlowRunRecord,
        node_runs: &[domain::NodeRunRecord],
        callback_tasks: &[domain::CallbackTaskRecord],
        owner_kind: &str,
    ) -> Result<()> {
        let first_node_run = &node_runs[0];
        let summary_node_run = merge_node_run_group(node_runs);
        let (stable_locator, source_stable_locator) = if node_runs.len() == 1 {
            (
                format!("{parent_stable_locator}/node:{}", first_node_run.id),
                format!("run:{}/node:{}", source_run.id, first_node_run.id),
            )
        } else {
            (
                format!("{parent_stable_locator}/node_group:{}", first_node_run.id),
                format!("run:{}/node_group:{}", source_run.id, first_node_run.id),
            )
        };
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let node_run_ids = node_runs
            .iter()
            .map(|node_run| node_run.id)
            .collect::<HashSet<_>>();
        let tool_tasks = callback_tasks
            .iter()
            .filter(|task| {
                node_run_ids.contains(&task.node_run_id) && task.callback_kind == "llm_tool_calls"
            })
            .collect::<Vec<_>>();
        let tool_call_count = tool_tasks
            .iter()
            .flat_map(|task| tool_calls_from_callback_task(task))
            .count();
        let child_count = usize::from(tool_call_count > 0);
        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "node_run".to_string(),
            owner_kind: Some(owner_kind.to_string()),
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
            child_count: i64::try_from(child_count).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: Some(source_run.id),
            source_trace_node_id: Some(trace_node_id_for_locator(
                source_run.id,
                &source_stable_locator,
            )),
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        if tool_call_count > 0 {
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
                &[],
            )?;
        }
        Ok(())
    }

    /// Client tool calls observed for a round through the formal output facts.
    fn push_native_tool_group(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        source_run: &domain::FlowRunRecord,
        tool_messages: &[&serde_json::Value],
    ) -> Result<()> {
        let stable_locator = format!("{parent_stable_locator}/tools");
        let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
        let all_returned = tool_messages.iter().all(|message| {
            message
                .get("tool_result")
                .is_some_and(|value| !value.is_null())
        });
        self.nodes.push(ApplicationRunTraceNodeProjectionInput {
            trace_node_id,
            parent_trace_node_id: Some(parent_trace_node_id),
            stable_locator: stable_locator.clone(),
            node_kind: "tool_group".to_string(),
            owner_kind: Some("task_round_tools".to_string()),
            owner_id: Some(parent_trace_node_id.to_string()),
            order_key: order_key.clone(),
            node_id: None,
            node_type: Some("tools".to_string()),
            node_mode: None,
            node_alias: "Tools".to_string(),
            status: if all_returned {
                "succeeded".to_string()
            } else {
                "waiting_callback".to_string()
            },
            started_at: source_run.started_at,
            finished_at: source_run.finished_at,
            duration_ms: None,
            metrics_payload: serde_json::json!({}),
            has_children: true,
            child_count: i64::try_from(tool_messages.len()).unwrap_or(i64::MAX),
            has_content: false,
            content_ref: None,
            source_flow_run_id: Some(source_run.id),
            source_trace_node_id: None,
            parent_callback_task_id: None,
            parent_tool_call_id: None,
            trace_relation_kind: None,
        });
        for (index, message) in tool_messages.iter().enumerate() {
            let item = &message["_source_item"];
            let call_id = item["call_id"].as_str().unwrap_or_default();
            let name = item["name"].as_str().unwrap_or(call_id);
            let result = message.get("tool_result").filter(|value| !value.is_null());
            let tool_locator = format!("{stable_locator}/tool:{call_id}");
            let tool_trace_node_id = trace_node_id_for_locator(self.flow_run_id, &tool_locator);
            self.nodes.push(ApplicationRunTraceNodeProjectionInput {
                trace_node_id: tool_trace_node_id,
                parent_trace_node_id: Some(trace_node_id),
                stable_locator: tool_locator,
                node_kind: "tool_callback".to_string(),
                owner_kind: Some("tool_call".to_string()),
                owner_id: Some(call_id.to_string()),
                order_key: child_order_key(&order_key, index + 1),
                node_id: None,
                node_type: Some("tool".to_string()),
                node_mode: None,
                node_alias: name.to_string(),
                status: if result.is_some() {
                    "returned".to_string()
                } else {
                    "waiting_callback".to_string()
                },
                started_at: source_run.started_at,
                finished_at: None,
                duration_ms: None,
                metrics_payload: serde_json::json!({}),
                has_children: false,
                child_count: 0,
                has_content: true,
                content_ref: None,
                source_flow_run_id: Some(source_run.id),
                source_trace_node_id: None,
                parent_callback_task_id: None,
                parent_tool_call_id: None,
                trace_relation_kind: None,
            });
            self.contents.push(ApplicationRunTraceNodeContentProjectionInput {
                trace_node_id: tool_trace_node_id,
                content_kind: "tool_callback".to_string(),
                payload: tool_callback_content_payload(None, call_id, name, item, result, None),
                source_refs: serde_json::json!([{"source_kind":"application_run_conversation_message_items","source_locator":call_id,"source_flow_run_id":source_run.id}]),
            });
        }
        Ok(())
    }
}

fn round_tool_messages(native_messages: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    native_messages
        .iter()
        .filter(|message| {
            matches!(
                message["_source_item"]["type"].as_str(),
                Some("custom_tool_call" | "function_call")
            ) && message["_source_item"]["call_id"].is_string()
        })
        .collect()
}

fn source_run_group_status<'a>(runs: impl Iterator<Item = &'a domain::FlowRunRecord>) -> String {
    let mut worst = "succeeded";
    for run in runs {
        match run.status {
            domain::FlowRunStatus::Failed => return "failed".to_string(),
            domain::FlowRunStatus::Succeeded => {}
            _ => worst = run.status.as_str(),
        }
    }
    worst.to_string()
}

fn source_run_group_finished_at<'a>(
    runs: impl Iterator<Item = &'a domain::FlowRunRecord>,
) -> Option<OffsetDateTime> {
    let mut latest = None;
    for run in runs {
        let finished_at = run.finished_at?;
        latest = Some(latest.map_or(finished_at, |current: OffsetDateTime| {
            current.max(finished_at)
        }));
    }
    latest
}
