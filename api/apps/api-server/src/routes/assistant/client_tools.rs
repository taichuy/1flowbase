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
pub struct AssistantClientToolBridge {
    enabled_tools: Vec<AssistantClientToolId>,
    connection_id: Uuid,
    outbound: Arc<Mutex<mpsc::Sender<String>>>,
    active_connection_id: Arc<Mutex<Uuid>>,
    declared_tools: Arc<Mutex<Vec<AssistantClientToolId>>>,
    pending: Arc<Mutex<PendingClientToolCalls>>,
    connected: Arc<AtomicBool>,
}

impl AssistantClientToolBridge {
    pub(super) fn new() -> (Self, mpsc::Receiver<String>) {
        let (outbound, frames) = mpsc::channel(8);
        let connection_id = Uuid::now_v7();
        (
            Self {
                enabled_tools: Vec::new(),
                connection_id,
                outbound: Arc::new(Mutex::new(outbound)),
                active_connection_id: Arc::new(Mutex::new(connection_id)),
                declared_tools: Arc::new(Mutex::new(Vec::new())),
                pending: Arc::new(Mutex::new(BTreeMap::new())),
                connected: Arc::new(AtomicBool::new(true)),
            },
            frames,
        )
    }

    pub(super) async fn for_tools(
        &self,
        enabled_tools: Vec<AssistantClientToolId>,
        declared_tools: Vec<AssistantClientToolId>,
    ) -> Self {
        *self.declared_tools.lock().await = declared_tools;
        Self {
            enabled_tools,
            connection_id: self.connection_id,
            outbound: self.outbound.clone(),
            active_connection_id: self.active_connection_id.clone(),
            declared_tools: self.declared_tools.clone(),
            pending: self.pending.clone(),
            connected: self.connected.clone(),
        }
    }

    pub(super) async fn complete_for_connection(
        &self,
        connection_id: Uuid,
        call_id: Uuid,
        result: Value,
        is_error: bool,
    ) -> bool {
        if *self.active_connection_id.lock().await != connection_id {
            return false;
        }
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

    pub(super) async fn replace_connection(
        &self,
        connection: &Self,
        declared_tools: Vec<AssistantClientToolId>,
    ) {
        self.fail_pending("assistant client connection replaced")
            .await;
        let replacement_outbound = connection.outbound.lock().await.clone();
        *self.outbound.lock().await = replacement_outbound;
        *self.active_connection_id.lock().await = connection.connection_id;
        *self.declared_tools.lock().await = declared_tools;
        self.connected.store(true, Ordering::Release);
    }

    pub(super) fn connection_id(&self) -> Uuid {
        self.connection_id
    }

    pub(super) async fn close_connection(&self, connection_id: Uuid) {
        if *self.active_connection_id.lock().await != connection_id {
            return;
        }
        self.connected.store(false, Ordering::Release);
        self.fail_pending("assistant client connection closed")
            .await;
    }

    pub(super) async fn close(&self) {
        self.connected.store(false, Ordering::Release);
        self.fail_pending("assistant client connection closed")
            .await;
    }

    async fn fail_pending(&self, message: &str) {
        let pending = std::mem::take(&mut *self.pending.lock().await);
        for (_, sender) in pending {
            let _ = sender.send(Err(message.to_string()));
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
            AssistantClientToolId::ListPageBlocks
            | AssistantClientToolId::InspectBlockRender
            | AssistantClientToolId::SearchBlockRender
            | AssistantClientToolId::ReadBlockRenderFragment
            | AssistantClientToolId::ClickBlockElement
            | AssistantClientToolId::RecompileBlock => (
                "Execute a Frontstage browser capability declared by an imported built-in MCP tool.",
                json!({"type":"object","additionalProperties":true}),
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
        let declared = self.declared_tools.lock().await;
        if !declared.iter().any(|tool| tool.as_str() == provider_name) {
            return Err(anyhow!("assistant client capability was not declared"));
        }
        drop(declared);
        let call_id = Uuid::now_v7();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(call_id, sender);
        let outbound = self.outbound.lock().await.clone();
        if outbound
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

    pub(crate) async fn invoke_capability(
        &self,
        capability_code: &str,
        arguments: Value,
    ) -> Result<(Value, bool)> {
        let (_, result) = self.call_client_tool(capability_code, arguments).await?;
        Ok((result.result, result.is_error))
    }

    pub(crate) async fn has_declared_capability(&self, capability_code: &str) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        self.declared_tools
            .lock()
            .await
            .iter()
            .any(|tool| tool.as_str() == capability_code)
    }

    pub(crate) async fn has_frontstage_capability_bundle(&self) -> bool {
        if !self.connected.load(Ordering::Acquire) {
            return false;
        }
        const REQUIRED: [&str; 6] = [
            "list_page_blocks",
            "inspect_block_render",
            "search_block_render",
            "read_block_render_fragment",
            "click_block_element",
            "recompile_block",
        ];
        let declared = self.declared_tools.lock().await;
        REQUIRED
            .iter()
            .all(|required| declared.iter().any(|tool| tool.as_str() == *required))
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
        let bridge = bridge
            .for_tools(
                vec![AssistantClientToolId::GetClientContext],
                vec![AssistantClientToolId::GetClientContext],
            )
            .await;
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
                .complete_for_connection(
                    bridge.connection_id(),
                    call_id,
                    json!({"url":"/settings"}),
                    false,
                )
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
        let bridge = bridge
            .for_tools(
                vec![AssistantClientToolId::RefreshClientView],
                vec![AssistantClientToolId::RefreshClientView],
            )
            .await;
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

    #[tokio::test]
    async fn ac_003_replacing_a_connection_fails_old_pending_calls_without_replay() {
        let (session, mut old_frames) = AssistantClientToolBridge::new();
        let session = session
            .for_tools(Vec::new(), vec![AssistantClientToolId::ClickBlockElement])
            .await;
        let pending = {
            let session = session.clone();
            tokio::spawn(async move {
                session
                    .invoke_capability("click_block_element", json!({}))
                    .await
            })
        };
        let _: Value = serde_json::from_str(&old_frames.recv().await.unwrap()).unwrap();
        let (replacement, mut replacement_frames) = AssistantClientToolBridge::new();
        session
            .replace_connection(&replacement, vec![AssistantClientToolId::ClickBlockElement])
            .await;
        assert!(pending.await.unwrap().is_err());
        assert!(replacement_frames.try_recv().is_err());
    }

    #[tokio::test]
    async fn ac_001_frontstage_discovery_tracks_the_active_connection_lease() {
        let frontstage_tools = vec![
            AssistantClientToolId::ListPageBlocks,
            AssistantClientToolId::InspectBlockRender,
            AssistantClientToolId::SearchBlockRender,
            AssistantClientToolId::ReadBlockRenderFragment,
            AssistantClientToolId::ClickBlockElement,
            AssistantClientToolId::RecompileBlock,
        ];
        let (session, _) = AssistantClientToolBridge::new();
        let session = session
            .for_tools(Vec::new(), frontstage_tools.clone())
            .await;
        assert!(session.has_frontstage_capability_bundle().await);

        session.close_connection(session.connection_id()).await;
        assert!(!session.has_frontstage_capability_bundle().await);

        let (replacement, _) = AssistantClientToolBridge::new();
        session
            .replace_connection(&replacement, frontstage_tools)
            .await;
        assert!(session.has_frontstage_capability_bundle().await);
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
