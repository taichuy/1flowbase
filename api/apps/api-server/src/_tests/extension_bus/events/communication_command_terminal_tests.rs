//! Actual installed SDK delivery plus the production dispatcher and PostgreSQL terminal owner.
use super::*;
use control_plane::lifecycle_outbox_dispatcher::{
    LifecycleDeliveryCompletionPort, LifecycleFactDeliveryCompletion, LifecycleOutboxDispatcher,
};
use domain::{CommunicationCommandSetting, CommunicationSendStatus};

struct Delivery(Arc<CommandFixture>);
#[async_trait::async_trait]
impl LifecycleFactDeliveryPort for Delivery {
    async fn deliver(&self, fact: &LifecycleOutboxRecord) -> anyhow::Result<()> {
        self.0.base.delivery.deliver(fact).await
    }
    async fn terminate_command(
        &self,
        fact: &LifecycleOutboxRecord,
        termination: CommunicationCommandTermination,
    ) -> anyhow::Result<()> {
        self.0
            .base
            .delivery
            .terminate_command(fact, termination)
            .await
    }
}
struct Completion;
impl LifecycleDeliveryCompletionPort for Completion {
    fn complete(&self, _outcome: CompletionOutcome<LifecycleFactDeliveryCompletion>) {}
}
fn dispatcher(
    fixture: &Arc<CommandFixture>,
) -> LifecycleOutboxDispatcher<storage_durable_postgres::MainDurableStore> {
    LifecycleOutboxDispatcher::new(
        fixture.base.store.clone(),
        Arc::new(Delivery(fixture.clone())),
        Arc::new(Completion),
    )
}

impl CommandFixture {
    async fn append(&self, key: &str, text: &str) -> Uuid {
        let revision = self
            .base
            .store
            .resolve_notification_authorization(
                self.scope.workspace_id,
                self.scope.instance_id,
                "fixture-key",
            )
            .await
            .unwrap()
            .revision;
        self.service
            .commit_receive(CommitCommunicationReceiveInput {
                authorization: SaveNotificationAuthorizationInput {
                    workspace_id: self.scope.workspace_id,
                    instance_id: self.scope.instance_id,
                    expected_revision: revision,
                    authorization: serde_json::from_value(json!({"status":"authorized"})).unwrap(),
                    managed_secrets: Default::default(),
                    master_key: "fixture-key".into(),
                },
                candidate_cursor: None,
                messages: vec![CommunicationReceiveMessage {
                    provider_message_id: key.into(),
                    conversation_id: "command-chat".into(),
                    sender_id: self.scope.external_identity.clone(),
                    sender_display_name: None,
                    received_at_ms: 43,
                    text: Some(text.into()),
                    attachments: vec![],
                }],
            })
            .await
            .unwrap()
            .unwrap();
        sqlx::query_scalar("SELECT id FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND external_identity=$3 AND external_message_key=$4")
            .bind(self.scope.workspace_id).bind(self.scope.instance_id).bind(&self.scope.external_identity).bind(key).fetch_one(self.base.store.pool()).await.unwrap()
    }
    fn worker_frames(&self) -> Vec<serde_json::Value> {
        match std::fs::read_to_string(&self.trace_path) {
            Ok(text) => text
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("SDK trace unavailable: {error}"),
        }
    }
    async fn later_inputs(&self) -> (Uuid, Uuid) {
        let ordinary = self
            .append("later-ordinary", "later ordinary question")
            .await;
        let new = self.append("later-new", "/new").await;
        assert!(
            self.service
                .admit_input(&self.scope, ordinary)
                .await
                .unwrap()
                .is_none(),
            "command must initially hold its admission barrier"
        );
        (ordinary, new)
    }
    async fn assert_released(&self, ordinary: Uuid, new: Uuid) {
        let result = self
            .service
            .command_for_input(&self.scope, self.input)
            .await
            .unwrap()
            .unwrap();
        assert!(result.reply.is_some());
        assert!(result.failure_code.is_some());
        assert!(matches!(
            result.send_status,
            CommunicationSendStatus::Pending | CommunicationSendStatus::Unknown
        ));
        let outbox: String =
            sqlx::query_scalar("SELECT status FROM lifecycle_outbox_deliveries WHERE event_id=$1")
                .bind(result.event_id)
                .fetch_one(self.base.store.pool())
                .await
                .unwrap();
        assert_eq!(
            outbox, "delivered",
            "transport ACK follows durable Host rejection"
        );
        assert_eq!(
            self.service.pending_admissions(&self.scope).await.unwrap(),
            vec![ordinary, new]
        );
        let candidates = self
            .service
            .scan_pending_admissions(self.scope.workspace_id, self.scope.instance_id, 100)
            .await
            .unwrap();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.input_id)
                .collect::<Vec<_>>(),
            vec![ordinary]
        );
        let admitted = self
            .service
            .admit_input(&self.scope, ordinary)
            .await
            .unwrap()
            .expect("ordinary turn must progress after terminal command");
        let candidates = self
            .service
            .scan_pending_admissions(self.scope.workspace_id, self.scope.instance_id, 100)
            .await
            .unwrap();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.input_id)
                .collect::<Vec<_>>(),
            vec![new]
        );
        let intent = self
            .service
            .resolve_command(&self.base.actor, &[], "/new")
            .await
            .unwrap();
        let queued = self
            .service
            .enqueue_command(&self.scope, new, intent)
            .await
            .unwrap();
        let claim = self
            .service
            .claim_builtin_command(self.scope.workspace_id, self.scope.instance_id, 60)
            .await
            .unwrap()
            .expect("new command must be claimable after failed plugin command");
        assert_eq!(claim.record.event_id, queued.event_id);
        let rotation = self
            .service
            .rotate_session(&self.scope, &queued.event_id.to_string())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rotation.invalidated_session_id, Some(admitted.session.id));
        self.service.complete_builtin_command(&claim,rotation.session.id,CommunicationCommandReply { text:"The previous conversation has stopped generating. A new conversation has started.".into() },None).await.unwrap();
        assert!(self
            .service
            .pending_admissions(&self.scope)
            .await
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn communication_command_terminal_revocation_disable_and_handler_loss_release_later_inputs() {
    for scenario in [
        "setting_disabled",
        "permission_revoked",
        "installation_disabled",
        "installation_removed",
        "frozen_graph_missing",
        "frozen_handler_missing",
    ] {
        let fixture = Arc::new(CommandFixture::new().await);
        fixture.enqueue().await;
        let (ordinary, new) = fixture.later_inputs().await;
        match scenario {
            "setting_disabled" => {
                let mut binding = fixture
                    .service
                    .get_binding(&fixture.base.actor, fixture.scope.instance_id)
                    .await
                    .unwrap()
                    .unwrap();
                binding.config.command_settings = vec![CommunicationCommandSetting {
                    command_id: "acme/acme.composition-commands/status".into(),
                    enabled: false,
                }];
                fixture
                    .service
                    .save_binding(
                        &fixture.base.actor,
                        fixture.scope.instance_id,
                        binding.config,
                        "fixture-tools".into(),
                    )
                    .await
                    .unwrap();
            }
            "permission_revoked" => {
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
            }
            "installation_disabled" => {
                sqlx::query(
                    "UPDATE extension_installations SET desired_state='disabled' WHERE id=$1",
                )
                .bind(fixture.installation)
                .execute(fixture.base.store.pool())
                .await
                .unwrap();
            }
            "installation_removed" => {
                // Installation and its grants may be absent while deletion-independent history remains.
                sqlx::query("DELETE FROM extension_installations WHERE id=$1")
                    .bind(fixture.installation)
                    .execute(fixture.base.store.pool())
                    .await
                    .unwrap();
            }
            "frozen_graph_missing" => {
                // Simulate a persisted pre-restart graph which is absent from retained executables.
                sqlx::query("UPDATE lifecycle_outbox SET graph_fingerprint='retired-command-graph' WHERE event_id=$1").bind(fixture.input).execute(fixture.base.store.pool()).await.unwrap();
                sqlx::query("UPDATE communication_command_events SET graph_fingerprint='retired-command-graph' WHERE event_id=$1").bind(fixture.input).execute(fixture.base.store.pool()).await.unwrap();
            }
            "frozen_handler_missing" => {
                sqlx::query("UPDATE lifecycle_outbox_deliveries SET handler_version='retired-command-handler' WHERE event_id=$1").bind(fixture.input).execute(fixture.base.store.pool()).await.unwrap();
                sqlx::query("UPDATE communication_command_events SET handler_version='retired-command-handler' WHERE event_id=$1").bind(fixture.input).execute(fixture.base.store.pool()).await.unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            dispatcher(&fixture).run_once().await.unwrap(),
            1,
            "{scenario}"
        );
        assert!(
            fixture.worker_frames().is_empty(),
            "unusable frozen target must never execute: {scenario}"
        );
        fixture.assert_released(ordinary, new).await;
        assert_eq!(dispatcher(&fixture).run_once().await.unwrap(), 0);
    }
}

#[tokio::test]
async fn communication_command_terminal_exhaustion_and_old_paused_backlog_never_reexecute() {
    for paused in [false, true] {
        let fixture = Arc::new(CommandFixture::new().await);
        fixture.enqueue().await;
        let (ordinary, new) = fixture.later_inputs().await;
        if paused {
            sqlx::query("UPDATE lifecycle_outbox_deliveries SET status='paused',pause_reason='frozen_graph_unavailable',paused_at=now(),attempt_count=0 WHERE event_id=$1").bind(fixture.input).execute(fixture.base.store.pool()).await.unwrap();
        } else {
            sqlx::query("UPDATE lifecycle_outbox_deliveries SET attempt_count=4 WHERE event_id=$1")
                .bind(fixture.input)
                .execute(fixture.base.store.pool())
                .await
                .unwrap();
        }
        let generic = Uuid::now_v7();
        fixture
            .base
            .store
            .record_lifecycle_fact(&RecordLifecycleFactInput {
                event_id: generic,
                transaction_id: generic,
                contract_id: "fixture.generic.paused".into(),
                contract_version: "1".into(),
                canonical_payload: b"{}".to_vec(),
                occurred_at: time::OffsetDateTime::now_utc(),
                publication: LifecyclePublicationPlan {
                    graph_fingerprint: "generic-graph".into(),
                    subscribers: vec![LifecycleSubscriberTarget {
                        subscriber_id: "generic-handler".into(),
                        handler_id: "generic-handler".into(),
                        handler_version: "1".into(),
                    }],
                },
            })
            .await
            .unwrap();
        sqlx::query("UPDATE lifecycle_outbox_deliveries SET status='paused',pause_reason='frozen_graph_unavailable',paused_at=now() WHERE event_id=$1").bind(generic).execute(fixture.base.store.pool()).await.unwrap();
        assert_eq!(dispatcher(&fixture).run_once().await.unwrap(), 1);
        let result = fixture
            .service
            .command_for_input(&fixture.scope, fixture.input)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.failure_code.as_deref(),
            Some(if paused {
                "command_outcome_unknown"
            } else {
                "command_delivery_exhausted"
            })
        );
        assert!(
            fixture.worker_frames().is_empty(),
            "prior attempt cannot replay a possibly external effect"
        );
        let generic_status: String =
            sqlx::query_scalar("SELECT status FROM lifecycle_outbox_deliveries WHERE event_id=$1")
                .bind(generic)
                .fetch_one(fixture.base.store.pool())
                .await
                .unwrap();
        assert_eq!(
            generic_status, "paused",
            "generic pause semantics remain unchanged"
        );
        fixture.assert_released(ordinary, new).await;
    }
}

#[tokio::test]
async fn communication_command_terminal_commit_failure_recovers_without_sdk_effect_replay() {
    let fixture = Arc::new(CommandFixture::new().await);
    fixture.enqueue().await;
    let (ordinary, new) = fixture.later_inputs().await;
    sqlx::query("CREATE FUNCTION reject_command_reply() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.reply_json IS NOT NULL THEN RAISE EXCEPTION 'fixture transaction abort'; END IF; RETURN NEW; END $$").execute(fixture.base.store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_command_reply BEFORE UPDATE ON communication_command_events FOR EACH ROW EXECUTE FUNCTION reject_command_reply()").execute(fixture.base.store.pool()).await.unwrap();
    assert!(dispatcher(&fixture).run_once().await.is_err());
    let frames = fixture.worker_frames();
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[0]["delivery"]["payload"]["input_id"],
        fixture.input.to_string()
    );
    assert!(fixture
        .service
        .command_for_input(&fixture.scope, fixture.input)
        .await
        .unwrap()
        .unwrap()
        .reply
        .is_none());
    let before: String =
        sqlx::query_scalar("SELECT status FROM lifecycle_outbox_deliveries WHERE event_id=$1")
            .bind(fixture.input)
            .fetch_one(fixture.base.store.pool())
            .await
            .unwrap();
    assert_eq!(
        before, "claimed",
        "failed receipt must stay recoverable, never paused with the barrier unresolved"
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication_messages WHERE input_id=$1 AND direction='outbound'",
    )
    .bind(fixture.input)
    .fetch_one(fixture.base.store.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER reject_command_reply ON communication_command_events")
        .execute(fixture.base.store.pool())
        .await
        .unwrap();
    let expired = sqlx::query("UPDATE lifecycle_outbox_deliveries SET claimed_at=clock_timestamp()-interval '2 seconds',claim_expires_at=clock_timestamp()-interval '1 second' WHERE event_id=$1 AND status='claimed'").bind(fixture.input).execute(fixture.base.store.pool()).await.unwrap();
    assert_eq!(
        expired.rows_affected(),
        1,
        "expire exactly the live command target while preserving the claim interval"
    );
    assert_eq!(dispatcher(&fixture).run_once().await.unwrap(), 1);
    assert_eq!(
        fixture.worker_frames().len(),
        1,
        "reclaimed uncertain attempt must not invoke the SDK again"
    );
    let send = fixture
        .service
        .reserve_command_response(&fixture.scope, fixture.input)
        .await
        .unwrap()
        .unwrap();
    fixture
        .service
        .record_command_response(
            &fixture.scope,
            fixture.input,
            send.claim_id,
            CommunicationCommandSendOutcome::Unknown,
        )
        .await
        .unwrap();
    // Controlled stale transport replay: the already durable command result and provider uncertainty win.
    sqlx::query("UPDATE lifecycle_outbox_deliveries SET status='pending',delivered_at=NULL,available_at=now() WHERE event_id=$1").bind(fixture.input).execute(fixture.base.store.pool()).await.unwrap();
    assert_eq!(dispatcher(&fixture).run_once().await.unwrap(), 1);
    assert_eq!(fixture.worker_frames().len(), 1);
    let retained = fixture
        .service
        .command_for_input(&fixture.scope, fixture.input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        retained.failure_code.as_deref(),
        Some("command_outcome_unknown")
    );
    assert_eq!(retained.send_status, CommunicationSendStatus::Unknown);
    assert!(fixture
        .service
        .reserve_command_response(&fixture.scope, fixture.input)
        .await
        .unwrap()
        .is_none());
    fixture.assert_released(ordinary, new).await;
}

#[tokio::test]
async fn communication_command_terminal_receipt_rejects_stale_claim_and_changed_target() {
    let fixture = Arc::new(CommandFixture::new().await);
    fixture.enqueue().await;
    let worker = Uuid::now_v7();
    let records = fixture
        .base
        .store
        .claim_lifecycle_facts(worker, 32, time::Duration::seconds(30))
        .await
        .unwrap();
    let fact = records
        .into_iter()
        .find(|fact| fact.event_id == fixture.input)
        .unwrap();
    for field in ["claim", "target", "payload"] {
        let mut forged = fact.clone();
        match field {
            "claim" => forged.claim_id = Some(Uuid::now_v7()),
            "target" => forged.handler_version = "different-target".into(),
            "payload" => forged.canonical_payload = b"{}".to_vec(),
            _ => unreachable!(),
        }
        assert!(fixture
            .base
            .delivery
            .terminate_command(&forged, CommunicationCommandTermination::OutcomeUnknown)
            .await
            .is_err());
        assert!(fixture
            .service
            .command_for_input(&fixture.scope, fixture.input)
            .await
            .unwrap()
            .unwrap()
            .reply
            .is_none());
    }
    fixture
        .base
        .store
        .retry_lifecycle_fact(
            fact.event_id,
            &fact.subscriber_id,
            worker,
            fact.claim_id.unwrap(),
            time::OffsetDateTime::now_utc(),
            "controlled claim replacement",
        )
        .await
        .unwrap();
    let replacement = fixture
        .base
        .store
        .claim_lifecycle_facts(Uuid::now_v7(), 32, time::Duration::seconds(30))
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.event_id == fixture.input)
        .unwrap();
    assert_ne!(replacement.claim_id, fact.claim_id);
    assert!(fixture
        .base
        .delivery
        .terminate_command(&fact, CommunicationCommandTermination::OutcomeUnknown)
        .await
        .is_err());
    fixture
        .base
        .delivery
        .terminate_command(
            &replacement,
            CommunicationCommandTermination::OutcomeUnknown,
        )
        .await
        .unwrap();
    assert!(fixture
        .service
        .pending_admissions(&fixture.scope)
        .await
        .unwrap()
        .is_empty());
    assert!(fixture.worker_frames().is_empty());
}

#[tokio::test]
async fn communication_command_authority_context_reuses_single_connection_and_current_settings() {
    let fixture = Arc::new(CommandFixture::new().await);
    let request = fixture.enqueue().await;
    let subject = ManagedContributionSubject::new(
        ManagedInstallationId::new(fixture.installation.to_string()).unwrap(),
        ManagedWorkspaceId::new(fixture.scope.workspace_id.to_string()).unwrap(),
        ContributionId::new("acme.composition-commands.status").unwrap(),
    );
    let mut lease = fixture
        .base
        .store
        .lock_contribution_authority(&subject)
        .await
        .unwrap();
    let context = lease
        .load_communication_command_context(&subject, request.event_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(context.command.request, request.request);
    assert!(context.channel_enabled);
    assert!(context.binding.unwrap().config.command_settings.is_empty());
    let foreign = ManagedContributionSubject::new(
        ManagedInstallationId::new(fixture.installation.to_string()).unwrap(),
        ManagedWorkspaceId::new(Uuid::now_v7().to_string()).unwrap(),
        subject.contribution_id().clone(),
    );
    assert!(
        lease
            .load_communication_command_context(&foreign, request.event_id)
            .await
            .is_err(),
        "the connection lease cannot read another workspace"
    );
    lease.release().await.unwrap();

    let mut binding = fixture
        .service
        .get_binding(&fixture.base.actor, fixture.scope.instance_id)
        .await
        .unwrap()
        .unwrap();
    binding.config.command_settings = vec![CommunicationCommandSetting {
        command_id: request.request.command_id.clone(),
        enabled: false,
    }];
    fixture
        .service
        .save_binding(
            &fixture.base.actor,
            fixture.scope.instance_id,
            binding.config,
            "fixture-tools".into(),
        )
        .await
        .unwrap();
    let mut lease = fixture
        .base
        .store
        .lock_contribution_authority(&subject)
        .await
        .unwrap();
    let context = lease
        .load_communication_command_context(&subject, request.event_id)
        .await
        .unwrap()
        .unwrap();
    assert!(!context.binding.unwrap().config.command_settings[0].enabled);
    lease.release().await.unwrap();

    assert_eq!(dispatcher(&fixture).run_once().await.unwrap(), 1);
    let result = fixture
        .service
        .command_for_input(&fixture.scope, fixture.input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.failure_code.as_deref(),
        Some("command_authorization_revoked"),
        "a disabled setting must reach current validation instead of timing out on another connection"
    );
    assert!(fixture.worker_frames().is_empty());
}
