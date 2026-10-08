use super::*;
use crate::execution_engine::{DataModelCallback, DataModelInvocationOutput};
use serde_json::{Map, Value};

fn target_plan(node_type: &str, bindings: BTreeMap<String, CompiledBinding>) -> CompiledPlan {
    let mut plan = sample_compiled_plan();
    let node = plan.nodes.get_mut("node-llm").unwrap();
    node.node_type = node_type.to_string();
    node.bindings = bindings;
    node.llm_runtime = None;
    node.outputs = vec![CompiledOutput {
        key: "result".to_string(),
        title: "Result".to_string(),
        value_type: "string".to_string(),
        selector: Vec::new(),
        json_schema: None,
    }];
    plan
}

fn selector_binding(path: &[&str]) -> CompiledBinding {
    CompiledBinding {
        i18n_text_ref: None,
        kind: "selector".to_string(),
        raw_value: json!({"kind": "selector", "value": path}),
        selector_paths: vec![path.iter().map(|part| part.to_string()).collect()],
    }
}

#[tokio::test]
async fn template_preview_uses_explicit_pool_without_replaying_upstream() {
    let plan = target_plan(
        "template_transform",
        BTreeMap::from([(
            "template".to_string(),
            CompiledBinding {
                i18n_text_ref: None,
                kind: "templated_text".to_string(),
                raw_value: json!("{{trigger.type}}-{{node-start.query}}"),
                selector_paths: vec![vec!["node-start".to_string(), "query".to_string()]],
            },
        )]),
    );
    let invoker = StubPreviewInvoker {
        captured_input: Arc::new(Mutex::new(None)),
    };
    let outcome = preview_executor::run_node_preview(
        &plan,
        "node-llm",
        &json!({
            "node-start": {"query": "C-42"}, "trigger": {"type": "extension"}
        }),
        &invoker,
    )
    .await
    .unwrap();
    assert_eq!(outcome.node_output, json!({"result": "extension-C-42"}));
    assert_eq!(outcome.resolved_inputs["template"], "extension-C-42");
    assert!(!outcome.is_failed());
    assert!(invoker.captured_input.lock().unwrap().is_none());
}

#[tokio::test]
async fn workflow_boundary_preview_matches_graph_input_and_output_projection() {
    let mut plan = target_plan(
        "workflow_end",
        BTreeMap::from([
            (
                "result".to_string(),
                selector_binding(&["node-start", "customer_id"]),
            ),
            ("internal".to_string(), selector_binding(&["env", "secret"])),
        ]),
    );
    plan.nodes.get_mut("node-start").unwrap().node_type = "workflow_start".to_string();
    let input = json!({
        "node-start": {"customer_id": "C-42"},
        "sys": {"workflow_run_id": "run-42"}, "env": {"secret": "preserved"},
        "trigger": {"type": "extension"}
    });
    let invoker = StubPreviewInvoker {
        captured_input: Arc::new(Mutex::new(None)),
    };
    let graph = crate::execution_engine::start_flow_debug_run(&plan, &input, &invoker)
        .await
        .unwrap();
    for node_id in ["node-start", "node-llm"] {
        let preview = preview_executor::run_node_preview(&plan, node_id, &input, &invoker)
            .await
            .unwrap();
        let trace = graph
            .node_traces
            .iter()
            .find(|trace| trace.node_id == node_id)
            .unwrap();
        assert_eq!(json!(preview.resolved_inputs), trace.input_payload);
        assert_eq!(preview.node_output, trace.output_payload);
        assert!(!preview.is_failed());
    }
    assert_eq!(
        graph.node_traces[0].input_payload["env"]["secret"],
        "preserved"
    );
    assert_eq!(
        graph.node_traces[1].output_payload,
        json!({"result": "C-42"})
    );
}

struct DataModelPreviewInvoker {
    records: Mutex<BTreeMap<String, Value>>,
    calls: Mutex<Vec<(String, Value, Value)>>,
    denied: bool,
    confirmation: bool,
}

#[async_trait]
impl ProviderInvoker for DataModelPreviewInvoker {
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        panic!("single node preview must not execute upstream provider nodes")
    }
}
#[async_trait]
impl CodeInvoker for DataModelPreviewInvoker {
    async fn invoke_code_node(
        &self,
        _: &CompiledCodeRuntime,
        _: Value,
        _: Value,
    ) -> Result<CodeInvocationOutput> {
        panic!("single node preview must not execute upstream code nodes")
    }
}
#[async_trait]
impl CapabilityInvoker for DataModelPreviewInvoker {
    async fn invoke_capability_node(
        &self,
        _: &crate::compiled_plan::CompiledPluginRuntime,
        _: Value,
        _: Value,
    ) -> Result<CapabilityInvocationOutput> {
        panic!("unexpected plugin invocation")
    }
    async fn invoke_data_model_node(
        &self,
        node: &CompiledNode,
        inputs: &Map<String, Value>,
    ) -> Result<DataModelInvocationOutput> {
        self.calls.lock().unwrap().push((
            node.node_type.clone(),
            node.config.clone(),
            json!(inputs),
        ));
        let mut records = self.records.lock().unwrap();
        let (output, error, callback) = if self.denied {
            (
                json!({}),
                Some(json!({"error_code": "permission_denied"})),
                None,
            )
        } else if self.confirmation {
            (
                json!({}),
                None,
                Some(DataModelCallback {
                    callback_kind: "data_model_side_effect_confirmation".to_string(),
                    request_payload: json!({"record_id": "r-1"}),
                }),
            )
        } else {
            let id = inputs
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("r-1");
            let output = match node.node_type.as_str() {
                "data_model_create" | "data_model_update" => {
                    let record = inputs["payload"].clone();
                    records.insert(id.to_string(), record.clone());
                    json!({"record": record})
                }
                "data_model_get" => json!({"record": records.get(id)}),
                "data_model_list" => json!({"records": records.values().collect::<Vec<_>>()}),
                "data_model_delete" => json!({"deleted": records.remove(id).is_some()}),
                _ => panic!("unexpected data model operation"),
            };
            (output, None, None)
        };
        Ok(DataModelInvocationOutput {
            output_payload: output,
            error_payload: error,
            metrics_payload: json!({"runtime": "data_model", "side_effect_replayed": false}),
            debug_payload: json!({"source": "authorized-invoker"}),
            pending_callback: callback,
        })
    }
}

fn data_model_invoker(denied: bool, confirmation: bool) -> DataModelPreviewInvoker {
    DataModelPreviewInvoker {
        records: Mutex::new(BTreeMap::new()),
        calls: Mutex::new(Vec::new()),
        denied,
        confirmation,
    }
}
fn data_model_plan(node_type: &str) -> CompiledPlan {
    let mut plan = target_plan(
        node_type,
        BTreeMap::from([
            (
                "record_id".to_string(),
                selector_binding(&["supplied", "id"]),
            ),
            (
                "payload".to_string(),
                selector_binding(&["supplied", "payload"]),
            ),
        ]),
    );
    plan.nodes.get_mut("node-llm").unwrap().config = json!({
        "data_model_id": "model-42", "data_source_instance_id": "source-42",
        "side_effect_policy": "allow_with_idempotency"
    });
    plan
}

#[tokio::test]
async fn data_model_preview_delegates_all_operations_and_preserves_mutations() {
    let invoker = data_model_invoker(false, false);
    let input = json!({"supplied": {"id": "r-1", "payload": {"name": "original"}}});
    for (node_type, expected) in [
        ("data_model_create", json!({"record": {"name": "original"}})),
        ("data_model_get", json!({"record": {"name": "original"}})),
        (
            "data_model_list",
            json!({"records": [{"name": "original"}]}),
        ),
        ("data_model_update", json!({"record": {"name": "updated"}})),
        ("data_model_delete", json!({"deleted": true})),
    ] {
        let plan = data_model_plan(node_type);
        let mut input = input.clone();
        if node_type == "data_model_update" {
            input["supplied"]["payload"]["name"] = json!("updated");
        }
        let preview = preview_executor::run_node_preview(&plan, "node-llm", &input, &invoker)
            .await
            .unwrap();
        assert_eq!(preview.node_output, expected);
        assert!(!preview.is_failed());
        assert_eq!(preview.metrics_payload["runtime"], "data_model");
        assert_eq!(preview.debug_payload["source"], "authorized-invoker");
        let calls = invoker.calls.lock().unwrap();
        let (_, config, supplied) = calls.last().unwrap();
        assert_eq!(config["data_source_instance_id"], "source-42");
        assert_eq!(supplied["payload"], input["supplied"]["payload"]);
    }
    assert!(invoker.records.lock().unwrap().is_empty());
    assert_eq!(invoker.calls.lock().unwrap().len(), 5);
}

#[tokio::test]
async fn data_model_preview_preserves_denial_and_does_not_bypass_confirmation() {
    let input = json!({"supplied": {"id": "r-1", "payload": {"name": "untouched"}}});
    let plan = data_model_plan("data_model_create");
    for (denied, confirmation, error_code) in [
        (true, false, "permission_denied"),
        (false, true, "node_preview_callback_not_supported"),
    ] {
        let invoker = data_model_invoker(denied, confirmation);
        let preview = preview_executor::run_node_preview(&plan, "node-llm", &input, &invoker)
            .await
            .unwrap();
        assert_eq!(preview.error_payload.unwrap()["error_code"], error_code);
        assert!(invoker.records.lock().unwrap().is_empty());
        assert_eq!(preview.debug_payload["source"], "authorized-invoker");
    }
}

#[tokio::test]
async fn data_model_preview_missing_explicit_pool_cannot_perform_a_write() {
    let invoker = data_model_invoker(false, false);
    let result = preview_executor::run_node_preview(
        &data_model_plan("data_model_create"),
        "node-llm",
        &json!({}),
        &invoker,
    )
    .await;
    assert!(result.is_err());
    assert!(invoker.records.lock().unwrap().is_empty());
    assert!(invoker.calls.lock().unwrap().is_empty());
}
