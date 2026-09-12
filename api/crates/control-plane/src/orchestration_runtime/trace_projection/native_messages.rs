use super::*;

impl TraceProjectionBuilder {
    pub(super) fn apply_native_messages(&mut self, detail: &domain::ApplicationRunDetail) {
        for message in &detail.native_messages {
            let Some(item) = message.get("_source_item") else {
                continue;
            };
            if !matches!(
                item.get("type").and_then(serde_json::Value::as_str),
                Some("custom_tool_call" | "function_call")
            ) {
                continue;
            }
            let Some(call_id) = item.get("call_id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let name = item
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(call_id);
            let result = message.get("tool_result").filter(|value| !value.is_null());
            let payload = tool_callback_content_payload(None, call_id, name, item, result, None);
            let existing = self.contents.iter().position(|content| {
                content.content_kind == "tool_callback"
                    && content
                        .payload
                        .get("tool_call_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(call_id)
            });
            if let Some(index) = existing {
                let content = &mut self.contents[index];
                // The original provider request is older than the callback task
                // and its result. Overlay its formal shape without erasing that truth.
                let has_callback_truth = ["callback_task_id", "tool_result", "callback_payload"]
                    .iter()
                    .any(|key| {
                        content
                            .payload
                            .get(*key)
                            .is_some_and(|value| !value.is_null())
                    });
                if has_callback_truth {
                    content.payload["request_payload"] = item.clone();
                    content.payload["tool_call"] = item.clone();
                } else {
                    content.payload = payload;
                    // A client result proves receipt, not verified execution.
                    if let Some(node) = self
                        .nodes
                        .iter_mut()
                        .find(|node| node.trace_node_id == content.trace_node_id)
                    {
                        node.status = if result.is_some() {
                            "returned"
                        } else {
                            "waiting_callback"
                        }
                        .into();
                        node.finished_at = None;
                        node.duration_ms = None;
                    }
                }
                if let Some(refs) = content.source_refs.as_array_mut() {
                    let source = serde_json::json!({"source_kind":"application_run_conversation_message_items","source_locator":call_id});
                    if !refs.contains(&source) {
                        refs.push(source);
                    }
                }
                continue;
            }
            // If the legacy node payload has no tool entry, add the formal tool
            // using the existing trace node/content contract, not another tree.
            let stable_locator = format!("run:{}/tool:{call_id}", self.flow_run_id);
            let trace_node_id = trace_node_id_for_locator(self.flow_run_id, &stable_locator);
            self.nodes.push(ApplicationRunTraceNodeProjectionInput {
                trace_node_id,
                parent_trace_node_id: None,
                stable_locator,
                node_kind: "tool_callback".into(),
                owner_kind: Some("tool_call".into()),
                owner_id: Some(call_id.into()),
                order_key: root_order_key(self.nodes.len()),
                node_id: None,
                node_type: Some("tool".into()),
                node_mode: None,
                node_alias: name.into(),
                status: if result.is_some() {
                    "returned"
                } else {
                    "waiting_callback"
                }
                .into(),
                started_at: detail.flow_run.started_at,
                finished_at: None,
                duration_ms: None,
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
            self.contents.push(ApplicationRunTraceNodeContentProjectionInput{trace_node_id,content_kind:"tool_callback".into(),payload,source_refs:serde_json::json!([{"source_kind":"application_run_conversation_message_items","source_locator":call_id}])});
        }
    }
}
