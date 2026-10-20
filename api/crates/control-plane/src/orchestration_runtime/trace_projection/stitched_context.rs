use super::*;

impl TraceProjectionBuilder {
    pub(super) fn push_stitched_context_group(
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

    pub(super) fn push_stitched_context_child(
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

    pub(super) fn push_stitched_context_node(
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

    pub(super) fn push_stitched_run_summary(
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

    pub(super) fn push_stitched_node_run_root(
        &mut self,
        order_key: String,
        parent_trace_node_id: Uuid,
        parent_stable_locator: &str,
        trace: &domain::ApplicationRunStitchedTrace,
        node_runs: &[domain::NodeRunRecord],
    ) -> Result<()> {
        let source_run = &trace.source_flow_run;
        let first_node_run = &node_runs[0];
        let summary_node_run = node_run_group_summary(node_runs);
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
