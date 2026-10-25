//! Root #2670: real receive worker, service commit, session selector and delegated MCP dispatch.
//! Requires the existing NOTIFICATION_WORKER_FIXTURE executable and migrated PostgreSQL.
use super::*;
use crate::_tests::extension_bus::managed_snapshot_tests::RuntimeFixture;
use control_plane::{
    notifications::{CreateNotificationInstance, NotificationService},
    plugin_management::*,
    ports::AuthRepository,
};
use orchestration_runtime::{
    compiled_plan::CompiledNode,
    execution_engine::{RuntimeInternalToolInvoker, RuntimeInternalToolOutput},
};
use serde_json::json;
use std::path::PathBuf;

fn receive_package() -> Vec<u8> {
    let fixture = crate::api_workspace_root()
        .unwrap()
        .join("plugins/fixtures/acme.notification");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(fixture.join("manifest.yaml")).unwrap()).unwrap();
    manifest["notification_types"][0]["receive"] =
        serde_json::from_slice(&std::fs::read(fixture.join("receive.json")).unwrap()).unwrap();
    manifest["managed"]["module"]["contributions"].as_array_mut().unwrap().push(json!({
        "contribution_id":"acme.notification-fixture.receive", "contributor_module_id":"notification-fixture",
        "point_id":"1flowbase.notification.receive", "contract_version":"1flowbase.notification/v1",
        "required_permissions":["notification.receive"], "mode":"append"
    }));
    manifest["managed"]["execution_bindings"].as_array_mut().unwrap().push(json!({
        "contribution_id":"acme.notification-fixture.receive", "execution_mode":"process_per_call",
        "runtime":{"protocol":"stdio_json","entry":"bin/worker"},"handler":"receive",
        "payload":{"kind":"notification_receive","type_code":"acme/notification-fixture/receipt"}
    }));
    plugin_framework::parse_plugin_manifest(&manifest.to_string()).unwrap();
    let worker = PathBuf::from(
        std::env::var_os("NOTIFICATION_WORKER_FIXTURE")
            .expect("prebuild notification_worker SDK example"),
    );
    assert!(
        worker.is_absolute() && worker.is_file(),
        "real SDK worker required"
    );
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", bytes.as_slice())
        .unwrap();
    archive.append_path_with_name(worker, "bin/worker").unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}

struct Fixture {
    state: Arc<ApiState>,
    runtime: RuntimeFixture,
    actor: ActorContext,
    channel: Uuid,
    mcp: String,
}
impl Fixture {
    async fn new() -> Self {
        let (mut state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let actor_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE account='root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        let actor = state
            .store
            .load_actor_context_for_user(actor_id)
            .await
            .unwrap();
        let runtime = RuntimeFixture::new(&state);
        let management = PluginManagementService::new(
            runtime.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new(runtime.services.clone()),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id)
        .with_managed_activation();
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor_id,
                file_name: "communication-fixture.1flowbasepkg".into(),
                package_bytes: receive_package(),
            })
            .await
            .unwrap();
        let installation_id = installed.installation.id;
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: actor_id,
                installation_id,
            })
            .await
            .unwrap();
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor_id,
                installation_id,
            })
            .await
            .unwrap();
        let service = Arc::new(
            NotificationService::new(
                runtime.store.clone(),
                state.provider_secret_master_key.clone(),
            )
            .with_provider(Arc::new(ManagedNotificationProvider::new(
                runtime.composition.clone(),
            ))),
        );
        let state_mut = Arc::get_mut(&mut state).unwrap();
        state_mut.store = runtime.store.clone();
        state_mut.provider_runtime = runtime.services.clone();
        state_mut.notification_service = service;
        let instance = state
            .notification_service
            .create_instance(
                &actor,
                CreateNotificationInstance {
                    type_code: "acme/notification-fixture/receipt".into(),
                    installation_id: Some(installation_id),
                    display_name: "Scoped receive".into(),
                    config_json: json!({"token":"private-token","channel":"ops"}),
                    status: None,
                },
            )
            .await
            .unwrap();
        let channel = instance.record.id;
        let catalog =
            crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies {
                store: state.store.clone(),
                openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies {
                    store: state.store.clone(),
                    console_operations: state.console_operation_registry.inventory().clone(),
                    interface_registry: state
                        .extension_boot_snapshot
                        .as_ref()
                        .and_then(|snapshot| snapshot.interface_registry())
                        .map(|registry| registry.snapshot()),
                    api_docs: state.api_docs.clone(),
                    template_catalog: state.runtime_engine.template_catalog().clone(),
                },
            };
        let deps = crate::routes::communications::interface::Dependencies::new(&state, catalog);
        let mcp =
            crate::routes::communications::provisioning::ensure_channel_mcp(&deps, &actor, channel)
                .await
                .unwrap()
                .unwrap();
        state
            .notification_service
            .save_binding(
                &actor,
                channel,
                domain::CommunicationBindingInput {
                    application_id: Uuid::now_v7(),
                    model: None,
                    reasoning_effort: None,
                    file_table_id: None,
                    mcp_instance_ids: vec![],
                    system_prompt: "Read scoped history with conversation_id from this session."
                        .into(),
                },
                mcp.clone(),
            )
            .await
            .unwrap();
        Self {
            state,
            runtime,
            actor,
            channel,
            mcp,
        }
    }
    async fn receive(&self) {
        let (signal, stop) = watch::channel(false);
        let state = self.state.clone();
        let key = (self.actor.current_workspace_id, self.channel);
        let worker = tokio::spawn(receive::run(state, key, stop));
        let received = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if self
                    .state
                    .notification_service
                    .receive_cursor(key.0, key.1)
                    .await
                    .unwrap()
                    == Some(json!("accepted-cursor"))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        signal.send(true).unwrap();
        worker.await.unwrap();
        received.expect("real receive must durably commit its cursor and inbox");
    }
    async fn invoker(&self, identity: &str) -> scoped_tools::ScopedTools {
        scoped_tools::ScopedTools {
            inner: Arc::new(
                crate::runtime_internal_tool_invoker_factory(&self.state, &self.actor)
                    .await
                    .unwrap()
                    .create_server_delegated(
                        axum::http::HeaderMap::new(),
                        self.actor.clone(),
                        vec![self.mcp.clone()],
                    )
                    .await
                    .unwrap(),
            ),
            scope: domain::CommunicationHistoryScope {
                workspace_id: self.actor.current_workspace_id,
                instance_id: self.channel,
                external_identity: identity.into(),
            },
        }
    }
}
fn node() -> CompiledNode {
    serde_json::from_value(json!({
        "node_id":"model","node_type":"llm","alias":"Model","container_id":null,
        "dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[],"config":{}
    }))
    .unwrap()
}
async fn invoke(
    invoker: &impl RuntimeInternalToolInvoker,
    operation: &str,
    arguments: Value,
) -> RuntimeInternalToolOutput {
    let node = node();
    let registrations = invoker.registrations_for_node(&node);
    let registration = registrations
        .iter()
        .find(|r| r.owner["operation"] == operation)
        .expect("progressive meta-tool registration");
    invoker
        .invoke_runtime_internal_tool(&node, registration, arguments)
        .await
        .unwrap()
}

#[tokio::test]
async fn communication_receive_session_progressive_mcp_reaches_scoped_history() {
    let fixture = Fixture::new().await;
    fixture.receive().await;
    let claim = fixture
        .state
        .notification_service
        .claim_pending_input(fixture.actor.current_workspace_id, fixture.channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.conversation.external_identity, "alice");
    let request = request::request(&claim, vec![]).unwrap();
    let session: Value =
        serde_json::from_str(request.history[0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        session["conversation_id"],
        claim.conversation.id.to_string(),
        "plugin guidance must supply the UUID accepted by the history route"
    );
    assert_eq!(session["external_conversation_key"], "external-room");
    let invoker = fixture.invoker("alice").await;
    let list = invoke(&invoker, "list", json!({"depth":8,"limit":20})).await;
    assert!(!list.is_error, "{:?}", list.content);
    let tool = format!("{}_messages", fixture.mcp);
    assert!(
        list.content.to_string().contains(&tool),
        "{:?}",
        list.content
    );
    let get = invoke(&invoker, "get", json!({"tool_id":tool})).await;
    assert!(!get.is_error, "{:?}", get.content);
    let des_id = get.content["structuredContent"]["des_id"]
        .as_str()
        .expect("mcp_get returns current des_id");
    let history = invoke(&invoker,"call",json!({"tool_id":tool,"des_id":des_id,"arguments":{"conversation_id":session["conversation_id"]}})).await;
    assert!(!history.is_error, "{:?}", history.content);
    let history = history.content.to_string();
    assert!(history.contains("Alice private input"), "{history}");
    assert!(!history.contains("Bob private input"), "{history}");
    assert!(!history.contains("private-token"));
    let bob = fixture
        .state
        .notification_service
        .scoped_conversations(
            &domain::CommunicationHistoryScope {
                external_identity: "bob".into(),
                workspace_id: fixture.actor.current_workspace_id,
                instance_id: fixture.channel,
            },
            None,
            100,
        )
        .await
        .unwrap()
        .items
        .remove(0)
        .id;
    let tampered = invoke(
        &invoker,
        "call",
        json!({"tool_id":tool,"des_id":des_id,"arguments":{"conversation_id":bob}}),
    )
    .await;
    assert!(!tampered.content.to_string().contains("Bob private input"));
    let injected = invoke(&invoker,"call",json!({"tool_id":tool,"des_id":des_id,"arguments":{"conversation_id":bob,"external_identity":"bob","instance_id":Uuid::now_v7()}})).await;
    assert!(
        injected.is_error,
        "server-bound channel and identity must not be replaceable: {:?}",
        injected.content
    );
    let missing_description = invoke(
        &invoker,
        "call",
        json!({"tool_id":tool,"arguments":{"conversation_id":session["conversation_id"]}}),
    )
    .await;
    assert!(
        missing_description.is_error,
        "a caller must obtain and supply des_id"
    );
    let other_sender = fixture.invoker("bob").await;
    let alice_under_bob = invoke(&other_sender,"call",json!({"tool_id":tool,"des_id":des_id,"arguments":{"conversation_id":session["conversation_id"]}})).await;
    assert!(!alice_under_bob
        .content
        .to_string()
        .contains("Alice private input"));
    fixture.runtime.host.stop().await.unwrap();
}

#[path = "pipeline.rs"]
mod pipeline;
