use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use orchestration_runtime::{
    compiled_plan::CompiledNode,
    execution_engine::{
        RuntimeInternalToolInvoker, RuntimeInternalToolOutput, RuntimeInternalToolRegistration,
    },
};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use super::AssistantClientToolId;

const CLIENT_TOOL_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
struct ClientToolResult {
    result: Value,
    is_error: bool,
}

type PendingClientToolCalls = BTreeMap<Uuid, oneshot::Sender<Result<ClientToolResult, String>>>;

#[derive(Clone)]
pub(super) struct AssistantClientToolBridge {
    enabled_tools: Vec<AssistantClientToolId>,
    outbound: mpsc::Sender<String>,
    pending: Arc<Mutex<PendingClientToolCalls>>,
    connected: Arc<AtomicBool>,
}

impl AssistantClientToolBridge {
    pub(super) fn new() -> (Self, mpsc::Receiver<String>) {
        let (outbound, frames) = mpsc::channel(8);
        (
            Self {
                enabled_tools: Vec::new(),
                outbound,
                pending: Arc::new(Mutex::new(BTreeMap::new())),
                connected: Arc::new(AtomicBool::new(true)),
            },
            frames,
        )
    }

    pub(super) fn for_tools(&self, enabled_tools: Vec<AssistantClientToolId>) -> Self {
        Self {
            enabled_tools,
            outbound: self.outbound.clone(),
            pending: self.pending.clone(),
            connected: self.connected.clone(),
        }
    }

    pub(super) async fn complete(&self, call_id: Uuid, result: Value, is_error: bool) -> bool {
        self.pending
            .lock()
            .await
            .remove(&call_id)
            .is_some_and(|sender| {
                sender
                    .send(Ok(ClientToolResult { result, is_error }))
                    .is_ok()
            })
    }

    pub(super) async fn close(&self) {
        self.connected.store(false, Ordering::Release);
        let pending = std::mem::take(&mut *self.pending.lock().await);
        for (_, sender) in pending {
            let _ = sender.send(Err("assistant client connection closed".to_string()));
        }
    }

    fn registration(tool_id: AssistantClientToolId) -> RuntimeInternalToolRegistration {
        let (description, input_schema) = match tool_id {
            AssistantClientToolId::GetClientContext => (
                "Read the current browser tab's console context at call time, including the complete address-bar URL without rewriting.",
                json!({"type":"object","properties":{},"additionalProperties":false}),
            ),
            AssistantClientToolId::RefreshClientView => (
                "Refresh the current console page or a registered semantic section in the same browser tab.",
                json!({
                    "type": "object",
                    "properties": {
                        "scope": {"type":"string","enum":["page","section"]},
                        "target_id": {"type":"string","enum":["current","application.current_section"]}
                    },
                    "required": ["scope", "target_id"],
                    "additionalProperties": false
                }),
            ),
        };
        RuntimeInternalToolRegistration {
            registration_id: format!("assistant_client|{}", tool_id.as_str()),
            provider_name: tool_id.as_str().to_string(),
            provider_tool: json!({
                "type": "function",
                "function": {
                    "name": tool_id.as_str(),
                    "description": description,
                    "parameters": input_schema,
                }
            }),
            owner: json!({
                "kind": "assistant_client",
                "tool_id": tool_id.as_str(),
                "source": {"kind": "run", "key": tool_id.as_str()}
            }),
        }
    }

    async fn call_client_tool(
        &self,
        provider_name: &str,
        arguments: Value,
    ) -> Result<(Uuid, ClientToolResult)> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(anyhow!("assistant client connection is unavailable"));
        }
        let call_id = Uuid::now_v7();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(call_id, sender);
        if self
            .outbound
            .send(
                json!({
                    "type":"client_tool.call",
                    "call_id":call_id,
                    "name":provider_name,
                    "arguments":arguments,
                })
                .to_string(),
            )
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&call_id);
            return Err(anyhow!("assistant client connection is unavailable"));
        }
        let result = match tokio::time::timeout(CLIENT_TOOL_TIMEOUT, receiver).await {
            Ok(Ok(Ok(result))) => result,
            Ok(Ok(Err(message))) => return Err(anyhow!(message)),
            Ok(Err(_)) => return Err(anyhow!("assistant client tool result channel closed")),
            Err(_) => {
                self.pending.lock().await.remove(&call_id);
                return Err(anyhow!("assistant client tool call timed out"));
            }
        };
        Ok((call_id, result))
    }
}

#[async_trait]
impl RuntimeInternalToolInvoker for AssistantClientToolBridge {
    fn registrations_for_node(&self, _node: &CompiledNode) -> Vec<RuntimeInternalToolRegistration> {
        self.enabled_tools
            .iter()
            .copied()
            .map(Self::registration)
            .collect()
    }

    async fn invoke_runtime_internal_tool(
        &self,
        node: &CompiledNode,
        registration: &RuntimeInternalToolRegistration,
        arguments: Value,
    ) -> Result<RuntimeInternalToolOutput> {
        let (call_id, result) = match self
            .call_client_tool(&registration.provider_name, arguments)
            .await
        {
            Ok(result) => result,
            Err(error) => {
                return Ok(RuntimeInternalToolOutput {
                    content: json!({
                        "status": "unavailable",
                        "code": "client_unavailable"
                    }),
                    is_error: true,
                    event: json!({
                        "event_type":"assistant_client_tool_call_unavailable",
                        "node_id":node.node_id,
                        "provider_name":registration.provider_name,
                        "owner":registration.owner,
                        "reason":error.to_string(),
                    }),
                });
            }
        };
        Ok(RuntimeInternalToolOutput {
            content: result.result,
            is_error: result.is_error,
            event: json!({
                "event_type":"assistant_client_tool_call_completed",
                "node_id":node.node_id,
                "provider_name":registration.provider_name,
                "owner":registration.owner,
                "call_id":call_id,
                "is_error":result.is_error,
            }),
        })
    }
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_001_client_tools_register_as_canonical_run_scoped_functions() {
        let registration =
            AssistantClientToolBridge::registration(AssistantClientToolId::GetClientContext);

        assert_eq!(registration.provider_tool["type"], json!("function"));
        assert_eq!(
            registration.provider_tool["function"]["name"],
            json!("get_client_context")
        );
        assert_eq!(
            registration.provider_tool["function"]["parameters"],
            json!({"type":"object","properties":{},"additionalProperties":false})
        );
        assert_eq!(registration.owner["kind"], json!("assistant_client"));
        assert_eq!(registration.owner["source"]["kind"], json!("run"));
    }

    #[tokio::test]
    async fn ac_002_client_tool_call_and_result_are_correlated_by_call_id() {
        let (bridge, mut frames) = AssistantClientToolBridge::new();
        let invocation = {
            let bridge = bridge.clone();
            tokio::spawn(async move {
                bridge
                    .call_client_tool("get_client_context", json!({}))
                    .await
            })
        };
        let frame: Value = serde_json::from_str(&frames.recv().await.unwrap()).unwrap();
        let call_id = Uuid::parse_str(frame["call_id"].as_str().unwrap()).unwrap();
        assert_eq!(frame["type"], "client_tool.call");
        assert_eq!(frame["name"], "get_client_context");

        assert!(
            bridge
                .complete(call_id, json!({"url":"/settings"}), false)
                .await
        );
        let (returned_call_id, result) = invocation.await.unwrap().unwrap();
        assert_eq!(returned_call_id, call_id);
        assert_eq!(result.result, json!({"url":"/settings"}));
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn ac_005_closing_the_socket_returns_client_unavailable_without_a_side_effect() {
        let (bridge, mut frames) = AssistantClientToolBridge::new();
        let registration =
            AssistantClientToolBridge::registration(AssistantClientToolId::RefreshClientView);
        let node = CompiledNode {
            node_id: "assistant".to_string(),
            node_type: "llm".to_string(),
            alias: "Assistant".to_string(),
            container_id: None,
            dependency_node_ids: Vec::new(),
            downstream_node_ids: Vec::new(),
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            config: json!({}),
            plugin_runtime: None,
            llm_runtime: None,
            code_runtime: None,
        };
        let invocation = {
            let bridge = bridge.clone();
            let registration = registration.clone();
            let node = node.clone();
            tokio::spawn(async move {
                bridge
                    .invoke_runtime_internal_tool(&node, &registration, json!({}))
                    .await
            })
        };
        let _ = frames.recv().await.unwrap();
        bridge.close().await;
        let output = invocation.await.unwrap().unwrap();
        assert!(output.is_error);
        assert_eq!(output.content["code"], json!("client_unavailable"));
        let disconnected = bridge
            .invoke_runtime_internal_tool(&node, &registration, json!({}))
            .await
            .unwrap();
        assert_eq!(disconnected.content["code"], json!("client_unavailable"));
        assert!(frames.try_recv().is_err());
    }
}

pub(super) struct AssistantRuntimeToolInvoker {
    mcp: Arc<dyn RuntimeInternalToolInvoker>,
    client: Option<Arc<AssistantClientToolBridge>>,
}

impl AssistantRuntimeToolInvoker {
    pub(super) fn new(
        mcp: Arc<dyn RuntimeInternalToolInvoker>,
        client: Option<Arc<AssistantClientToolBridge>>,
    ) -> Self {
        Self { mcp, client }
    }
}

#[async_trait]
impl RuntimeInternalToolInvoker for AssistantRuntimeToolInvoker {
    fn registrations_for_node(&self, node: &CompiledNode) -> Vec<RuntimeInternalToolRegistration> {
        let mut registrations = self.mcp.registrations_for_node(node);
        if let Some(client) = &self.client {
            registrations.extend(client.registrations_for_node(node));
        }
        registrations
    }

    async fn invoke_runtime_internal_tool(
        &self,
        node: &CompiledNode,
        registration: &RuntimeInternalToolRegistration,
        arguments: Value,
    ) -> Result<RuntimeInternalToolOutput> {
        if registration.owner.get("kind").and_then(Value::as_str) == Some("assistant_client") {
            return self
                .client
                .as_ref()
                .ok_or_else(|| anyhow!("assistant client tool is unavailable"))?
                .invoke_runtime_internal_tool(node, registration, arguments)
                .await;
        }
        self.mcp
            .invoke_runtime_internal_tool(node, registration, arguments)
            .await
    }
}
