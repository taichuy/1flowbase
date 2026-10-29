//! Real SDK worker, current authority and durable selected-target Outbox; no channel send.
use super::{
    generic_event_tests::Fixture,
    managed_event_authority_tests::{manifest, package},
};
use control_plane::{
    lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort, notifications::*, plugin_management::*,
};
use control_plane_contracts::ports::*;
use extension_contracts::*;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

struct CommandFixture {
    base: Fixture,
    service: NotificationService<storage_durable_postgres::MainDurableStore>,
    installation: Uuid,
    scope: domain::CommunicationConversationScope,
    input: Uuid,
    trace_path: PathBuf,
}
impl CommandFixture {
    async fn new() -> Self {
        let base = Fixture::new().await;
        assert_eq!(
            base.store.pool().options().get_max_connections(),
            1,
            "command validation must reuse the authority connection; do not mask reentrant borrowing with a larger pool"
        );
        let mut raw = manifest("commands");
        raw["communication_commands"] = json!([
            {"command_id":"acme/acme.composition-commands/status","name":"status","description":"Read status","usage":"/status","contribution_id":"acme.composition-commands.status","default_enabled":true},
            {"command_id":"acme/acme.composition-commands/other","name":"other","description":"Other target","usage":"/other","contribution_id":"acme.composition-commands.other","default_enabled":true}
        ]);
        raw["managed"]["module"]["contributions"] = json!(["status","other"].into_iter().map(|name|json!({"contribution_id":format!("acme.composition-commands.{name}"),"contributor_module_id":"acme.composition-commands","point_id":COMMUNICATION_COMMAND_POINT,"contract_version":"1","required_permissions":["event.subscribe"],"mode":"append"})).collect::<Vec<_>>());
        raw["managed"]["execution_bindings"] = json!(["status","other"].into_iter().map(|name|json!({"contribution_id":format!("acme.composition-commands.{name}"),"execution_mode":"process_per_call","runtime":{"protocol":"stdio_json","entry":"bin/worker.py"},"handler":"communication_command"})).collect::<Vec<_>>());
        let management = PluginManagementService::new(
            base.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new(base._services.clone()),
            base._state.official_plugin_source.clone(),
            &base._state.provider_install_root,
        )
        .with_node_id(&base._state.api_node_id)
        .with_managed_activation();
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: base.actor.user_id,
                file_name: "commands.1flowbasepkg".into(),
                package_bytes: package(&raw),
            })
            .await
            .unwrap();
        let installation = installed.installation.id;
        let trace_path = PathBuf::from(
            installed
                .local_artifact
                .local_path
                .clone()
                .expect("installed SDK path"),
        )
        .join("bin/worker.trace");
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: base.actor.user_id,
                installation_id: installation,
            })
            .await
            .unwrap();
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: base.actor.user_id,
                installation_id: installation,
            })
            .await
            .unwrap();
        let provider = Arc::new(
            crate::extension_bus::ManagedCommunicationCommandProvider::new(
                base._services.managed_composition().unwrap(),
            ),
        );
        let service = NotificationService::new(base.store.clone(), "fixture-key".into())
            .with_command_provider(provider);
        let instance_id = Uuid::now_v7();
        let conversation_id = Uuid::now_v7();
        let input = Uuid::now_v7();
        let scope = domain::CommunicationConversationScope {
            workspace_id: base.actor.current_workspace_id,
            instance_id,
            external_identity: "alice".into(),
            conversation_id,
        };
        sqlx::query("INSERT INTO notification_instances(id,workspace_id,type_code,display_name) VALUES($1,$2,'unrelated.channel.provider','Command fixture')").bind(instance_id).bind(scope.workspace_id).execute(base.store.pool()).await.unwrap();
        service
            .save_binding(
                &base.actor,
                instance_id,
                domain::CommunicationBindingInput {
                    application_id: Uuid::now_v7(),
                    model: None,
                    reasoning_effort: None,
                    file_table_id: None,
                    mcp_instance_ids: vec![],
                    system_prompt: String::new(),
                    command_settings: vec![],
                },
                "fixture-tools".into(),
            )
            .await
            .unwrap();
        sqlx::query("INSERT INTO communication_conversations(id,scope_id,instance_id,external_identity,external_conversation_key) VALUES($1,$2,$3,'alice','command-chat')").bind(conversation_id).bind(scope.workspace_id).bind(instance_id).execute(base.store.pool()).await.unwrap();
        sqlx::query("INSERT INTO communication_inputs(id,scope_id,instance_id,conversation_id,external_identity,external_message_key,received_at_ms) VALUES($1,$2,$3,$4,'alice','command-input',42)").bind(input).bind(scope.workspace_id).bind(instance_id).bind(conversation_id).execute(base.store.pool()).await.unwrap();
        Self {
            base,
            service,
            installation,
            scope,
            input,
            trace_path,
        }
    }
    async fn enqueue(&self) -> CommunicationCommandRecord {
        let intent = self
            .service
            .resolve_command(&self.base.actor, &[], "/status today")
            .await
            .unwrap();
        assert!(matches!(intent, CommunicationCommandIntent::Plugin { .. }));
        self.service
            .enqueue_command(&self.scope, self.input, intent)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn communication_command_installed_sdk_uses_one_frozen_target_and_one_durable_reply() {
    let fixture = CommandFixture::new().await;
    let queued = fixture.enqueue().await;
    assert_eq!(fixture.enqueue().await.event_id, queued.event_id);
    let rows = fixture.base.claim(Uuid::now_v7()).await;
    let command = rows
        .iter()
        .filter(|row| row.contract_id == COMMUNICATION_COMMAND_POINT)
        .collect::<Vec<_>>();
    assert_eq!(
        command.len(),
        1,
        "cardinality Many must not broadcast commands"
    );
    assert!(command[0].subscriber_id.ends_with(".status"));
    let mut forged = command[0].clone();
    let mut payload: serde_json::Value = serde_json::from_slice(&forged.canonical_payload).unwrap();
    payload["payload"]["arguments"] = "forged".into();
    forged.canonical_payload = serde_json::to_vec(&payload).unwrap();
    assert!(fixture.base.delivery.deliver(&forged).await.is_err());
    fixture.base.delivery.deliver(command[0]).await.unwrap();
    fixture.base.delivery.deliver(command[0]).await.unwrap();
    let result = fixture
        .base
        .store
        .communication_command(fixture.scope.workspace_id, queued.event_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.reply.unwrap().text,
        "`acme/acme.composition-commands/status`: today"
    );
    assert_eq!(
        result.send_status,
        domain::CommunicationSendStatus::Pending,
        "durable handler completion is not provider delivery"
    );
    assert_eq!(
        fixture
            .service
            .pending_command_responses(fixture.scope.workspace_id, fixture.scope.instance_id, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication_messages WHERE input_id=$1 AND direction='outbound'",
    )
    .bind(fixture.input)
    .fetch_one(fixture.base.store.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    assert!(fixture
        .service
        .pending_admissions(&fixture.scope)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn communication_command_revoked_or_disabled_installation_cannot_execute_frozen_request() {
    let fixture = CommandFixture::new().await;
    fixture.enqueue().await;
    let rows = fixture.base.claim(Uuid::now_v7()).await;
    let command = rows
        .iter()
        .find(|row| row.contract_id == COMMUNICATION_COMMAND_POINT)
        .unwrap();
    sqlx::query("UPDATE extension_installations SET desired_state='disabled' WHERE id=$1")
        .bind(fixture.installation)
        .execute(fixture.base.store.pool())
        .await
        .unwrap();
    assert!(fixture.base.delivery.deliver(command).await.is_err());
    sqlx::query("UPDATE extension_installations SET desired_state='active_requested' WHERE id=$1")
        .bind(fixture.installation)
        .execute(fixture.base.store.pool())
        .await
        .unwrap();
    let authority = PluginContributionAuthorityService::new(
        fixture.base.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let snapshot = authority
        .query(&fixture.base.actor, fixture.installation)
        .await
        .unwrap();
    let grant = snapshot
        .authorizations
        .iter()
        .find(|grant| {
            grant.contribution_id == "acme.composition-commands.status"
                && grant.permission == "event.subscribe"
        })
        .unwrap();
    authority
        .revoke(
            &fixture.base.actor,
            fixture.installation,
            RevokeContributionPermission {
                authorization_id: grant.id,
                expected_revision: snapshot.revision,
            },
        )
        .await
        .unwrap();
    assert!(fixture.base.delivery.deliver(command).await.is_err());
    assert!(fixture
        .base
        .store
        .communication_command(fixture.scope.workspace_id, fixture.input)
        .await
        .unwrap()
        .unwrap()
        .reply
        .is_none());
}

#[tokio::test]
async fn communication_command_explicit_handler_failure_is_a_durable_reply_receipt() {
    let fixture = CommandFixture::new().await;
    let intent = fixture
        .service
        .resolve_command(&fixture.base.actor, &[], "/status fixture-fail")
        .await
        .unwrap();
    let request = fixture
        .service
        .enqueue_command(&fixture.scope, fixture.input, intent)
        .await
        .unwrap();
    let worker = Uuid::now_v7();
    let rows = fixture.base.claim(worker).await;
    let command = rows
        .iter()
        .find(|row| row.contract_id == COMMUNICATION_COMMAND_POINT)
        .unwrap();
    fixture.base.delivery.deliver(command).await.unwrap();
    fixture.base.ack(worker, command).await;
    let result = fixture
        .base
        .store
        .communication_command(fixture.scope.workspace_id, request.event_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.failure_code.as_deref(),
        Some("fixture_command_failed")
    );
    assert_eq!(
        result.reply.unwrap().text,
        r"Command failed: fixture\_command\_failed"
    );
    assert_eq!(result.send_status, domain::CommunicationSendStatus::Pending);
    assert!(fixture
        .service
        .pending_admissions(&fixture.scope)
        .await
        .unwrap()
        .is_empty());
}

#[path = "communication_command_terminal_tests.rs"]
mod terminal_tests;

#[tokio::test]
async fn communication_command_sdk_markdown_is_preserved_in_receipt_history_and_send_claim() {
    let fixture = CommandFixture::new().await;
    let intent = fixture
        .service
        .resolve_command(&fixture.base.actor, &[], "/status fixture-markdown")
        .await
        .unwrap();
    let request = fixture
        .service
        .enqueue_command(&fixture.scope, fixture.input, intent)
        .await
        .unwrap();
    let worker = Uuid::now_v7();
    let rows = fixture.base.claim(worker).await;
    let command = rows
        .iter()
        .find(|row| row.contract_id == COMMUNICATION_COMMAND_POINT)
        .unwrap();
    fixture.base.delivery.deliver(command).await.unwrap();
    fixture.base.delivery.deliver(command).await.unwrap();
    fixture.base.ack(worker, command).await;
    let record = fixture
        .base
        .store
        .communication_command(fixture.scope.workspace_id, request.event_id)
        .await
        .unwrap()
        .unwrap();
    let expected = "**Status**\n\n- `/next [id]` — Ready\\\n  Second line";
    assert_eq!(record.reply.as_ref().unwrap().text, expected);
    let history: Vec<String> = sqlx::query_scalar(
        "SELECT content FROM communication_messages WHERE input_id=$1 AND direction='outbound'",
    )
    .bind(fixture.input)
    .fetch_all(fixture.base.store.pool())
    .await
    .unwrap();
    assert_eq!(
        history,
        vec![expected.to_string()],
        "one unchanged Markdown reply after redelivery"
    );
    let claimed = fixture
        .service
        .reserve_command_response(&fixture.scope, record.event_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.record.reply.unwrap().text, expected);
    assert!(fixture
        .service
        .reserve_command_response(&fixture.scope, record.event_id)
        .await
        .unwrap()
        .is_none());
}
