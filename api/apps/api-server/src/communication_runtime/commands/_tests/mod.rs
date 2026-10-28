use super::*;
use control_plane::ports::*;
use domain::*;
use serde_json::json;

mod admission;
mod delivery;
mod effects;

struct Fixture {
    state: Arc<ApiState>,
    actor: ActorContext,
    key: (Uuid, Uuid),
}
impl Fixture {
    async fn new() -> Self {
        let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let user: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE account='root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        let actor = ApplicationRepository::load_actor_context_for_user(&state.store, user)
            .await
            .unwrap();
        let channel = Uuid::now_v7();
        let now = time::OffsetDateTime::now_utc();
        state
            .store
            .save_notification_instance(SaveNotificationInstanceInput {
                record: NotificationInstanceRecord {
                    id: channel,
                    workspace_id: actor.current_workspace_id,
                    type_code: "communication.command-fixture".into(),
                    installation_id: None,
                    display_name: "Command fixture".into(),
                    status: NotificationInstanceStatus::Enabled,
                    config_json: json!({}),
                    configured_secret_fields: vec![],
                    created_at: now,
                    updated_at: now,
                },
                secret_config_json: json!({}),
                master_key: state.provider_secret_master_key.clone(),
                create: true,
                initial_authorization: None,
            })
            .await
            .unwrap();
        state
            .notification_service
            .save_binding(
                &actor,
                channel,
                CommunicationBindingInput {
                    application_id: Uuid::now_v7(),
                    model: None,
                    reasoning_effort: None,
                    file_table_id: None,
                    mcp_instance_ids: vec![],
                    system_prompt: "Command fixture".into(),
                    command_settings: vec![],
                },
                "fixture-tools".into(),
            )
            .await
            .unwrap();
        let key = (actor.current_workspace_id, channel);
        Self { state, actor, key }
    }

    async fn receive(
        &self,
        sender: &str,
        message_key: &str,
        text: &str,
    ) -> (CommunicationConversationScope, Uuid) {
        let revision = self
            .state
            .store
            .resolve_notification_authorization(
                self.key.0,
                self.key.1,
                &self.state.provider_secret_master_key,
            )
            .await
            .unwrap()
            .revision;
        self.state
            .notification_service
            .commit_receive(CommitCommunicationReceiveInput {
                authorization: SaveNotificationAuthorizationInput {
                    workspace_id: self.key.0,
                    instance_id: self.key.1,
                    expected_revision: revision,
                    authorization: serde_json::from_value(json!({"status":"authorized"})).unwrap(),
                    managed_secrets: Default::default(),
                    master_key: self.state.provider_secret_master_key.clone(),
                },
                candidate_cursor: None,
                messages: vec![CommunicationReceiveMessage {
                    provider_message_id: message_key.into(),
                    conversation_id: format!("external-chat-{sender}"),
                    sender_id: sender.into(),
                    sender_display_name: None,
                    received_at_ms: 42,
                    text: Some(text.into()),
                    attachments: vec![],
                }],
            })
            .await
            .unwrap()
            .unwrap();
        // Admission scanning intentionally exposes only each chat's earliest unresolved input.
        // A receive fixture locates its exact durable receipt by the provider's dedup identity.
        let (input_id, conversation_id): (Uuid, Uuid) = sqlx::query_as(
            "SELECT id,conversation_id FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND external_message_key=$4",
        )
        .bind(self.key.0)
        .bind(self.key.1)
        .bind(sender)
        .bind(message_key)
        .fetch_one(self.state.store.pool())
        .await
        .unwrap();
        (
            CommunicationConversationScope {
                workspace_id: self.key.0,
                instance_id: self.key.1,
                external_identity: sender.into(),
                conversation_id,
            },
            input_id,
        )
    }

    async fn effect(&self) {
        let (_signal, stop) = watch::channel(false);
        super::effects::process_next(&self.state, self.key, stop)
            .await
            .unwrap();
    }
}
