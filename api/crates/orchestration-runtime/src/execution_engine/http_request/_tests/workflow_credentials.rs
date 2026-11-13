use super::*;
use crate::{
    compiled_plan::CompiledLlmRuntime,
    execution_engine::{ProviderInvocationOutput, WorkflowInternalHttpResponse},
};
use extension_contracts::provider_contract::ProviderInvocationInput;
use std::sync::Mutex;

#[derive(Default)]
struct Host(Mutex<Vec<WorkflowInternalHttpRequest>>);

#[async_trait]
impl ProviderInvoker for Host {
    async fn invoke_llm(
        &self,
        _: &CompiledLlmRuntime,
        _: ProviderInvocationInput,
    ) -> Result<ProviderInvocationOutput> {
        unreachable!()
    }
    async fn invoke_workflow_internal_http(
        &self,
        reference: &str,
        request: WorkflowInternalHttpRequest,
    ) -> Result<WorkflowInternalHttpResponse> {
        assert!(reference.starts_with("wfcred:"));
        self.0.lock().unwrap().push(request);
        Ok(WorkflowInternalHttpResponse {
            status: 200,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: br#"{"ok":true}"#.to_vec(),
        })
    }
}

fn node(url: &str) -> CompiledNode {
    serde_json::from_value(json!({
        "node_id":"http","node_type":"http_request","alias":"HTTP","container_id":null,
        "dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[],
        "config":{"url":url,"method":"POST","body_type":"json"}
    }))
    .unwrap()
}

fn inputs() -> Map<String, Value> {
    json!({"headers":{"Authorization":"Bearer wfcred:fixture:executor"},
        "body":{"scope":{"batch_size":100}},"params":{"title":"space & symbol"}})
    .as_object()
    .unwrap()
    .clone()
}

#[tokio::test]
async fn credential_request_uses_host_without_network_and_keeps_response_contract() {
    let host = Host::default();
    let output = execute_http_request_node_with_provider_invoker(
        &node("/api/console/applications/{{ sys.application_id }}/logs/deletion-jobs"),
        &inputs(),
        json!({"sys":{"application_id":"app-one"}})
            .as_object()
            .unwrap(),
        None,
        Some(&host),
    )
    .await
    .unwrap();
    assert!(output.error_payload.is_none());
    assert_eq!(output.output_payload["status_code"], 200);
    assert_eq!(output.output_payload["body"], r#"{"ok":true}"#);
    let calls = host.0.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "POST");
    assert_eq!(
        calls[0].url,
        "/api/console/applications/app-one/logs/deletion-jobs?title=space+%26+symbol"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&calls[0].body).unwrap(),
        json!({"scope":{"batch_size":100}})
    );
}

#[tokio::test]
async fn credential_cannot_fall_back_to_external_http_or_unconfigured_host() {
    let host = Host::default();
    let output = execute_http_request_node_with_provider_invoker(
        &node("https://example.invalid/api/console/me"),
        &inputs(),
        &Map::new(),
        None,
        Some(&host),
    )
    .await
    .unwrap();
    assert!(output.error_payload.is_some());
    assert!(host.0.lock().unwrap().is_empty());
    let missing = execute_http_request_node(&node("/api/console/me"), &inputs(), &Map::new(), None)
        .await
        .unwrap();
    assert!(missing.error_payload.is_some());
}
