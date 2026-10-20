use super::*;

impl TraceProjectionBuilder {
    pub(super) fn push_tool_route_node(
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
                payload: (route_trace.clone()).into(),
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

    pub(super) fn push_route_branch_node(
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
                payload: (branch_trace.clone()).into(),
                source_refs: serde_json::json!([{
                    "source_kind": "visible_internal_llm_tool_branch",
                    "source_locator": locator_component
                }]),
            });

        Ok(())
    }
}
