//! Real service + PostgreSQL evidence; only the external provider is controlled.
use control_plane::{
    notifications::{
        system_notification_descriptor, CreateNotificationInstance, NotificationProviderPort,
        NotificationSenderPort, NotificationService,
    },
    ports::{
        CreatePluginAssignmentInput, NotificationLogListInput, NotificationRepository,
        PluginRepository, SaveNotificationAuthorizationInput, UpsertPluginInstallationInput,
    },
};
use domain::{ActorContext, NotificationSendLog, NotificationSource};
use extension_contracts::{ContributionId, NotificationLogSnapshot, NotificationTypeDescriptor};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

const MASTER_KEY: &str = "log-mapping-fixture-key";
const CONFIG_SECRET: &str = "private-config-log-token";
const MANAGED_SECRET: &str = "private-managed-log-token";
const NEW_SECRET: &str = "private-rotated-log-token";
const QR_CREDENTIAL: &str = "private-log-qr-challenge";
const QR_URL: &str = "https://example.invalid/qr/private-log-qr-challenge";

fn descriptor(managed: bool) -> NotificationTypeDescriptor {
    let mut descriptor = system_notification_descriptor();
    descriptor.type_code = "acme/log-fixture/message".into();
    descriptor.contribution_id = ContributionId::new("acme.log-fixture.send").unwrap();
    descriptor.config_schema = json!({
        "type":"object", "properties":{"token":{"type":"string"}},
        "required":["token"], "additionalProperties":false
    });
    descriptor.config_form = serde_json::from_value(json!({
        "schema_version":"1.0.0", "fields":[
            {"key":"token","label":"Token","type":"string","control":"password","required":true}
        ]
    }))
    .unwrap();
    descriptor.input_schema = json!({
        "type":"object", "properties":{
            "title":{"type":"string"}, "body":{"type":"string"},
            "context":{"type":"object"}, "skipped":{"type":"string"},
            "alias_probe":{"type":"string"}
        }, "required":["body"], "additionalProperties":false
    });
    descriptor.input_form = serde_json::from_value(json!({
        "schema_version":"1.0.0", "fields":[
            {"key":"body","label":"Message","type":"string","required":true}
        ]
    }))
    .unwrap();
    descriptor.output_schema = json!({
        "type":"object", "properties":{
            "receipt":{"type":"string"}, "context":{"type":"object"},
            "skipped":{"type":"string"}, "alias_probe":{"type":"string"}
        }, "required":["receipt"], "additionalProperties":false
    });
    descriptor.log = Some(
        serde_json::from_value(json!({
            "title":"/title", "content":"/body", "extra":{
                "context":{"source":"input","pointer":"/context"},
                "receipt":{"source":"result","pointer":"/receipt"},
                "result_context":{"source":"result","pointer":"/context"},
                (CONFIG_SECRET):{"source":"input","pointer":"/alias_probe"},
                (NEW_SECRET):{"source":"result","pointer":"/alias_probe"}
            }
        }))
        .unwrap(),
    );
    if managed {
        descriptor.authorization = Some(
            serde_json::from_value(json!({
                "actions":[{"code":"qr","label":"Scan","kind":"qr_code"}],
                "managed_secret_keys":["bot_token","rotated_token","qr_credential","qr_code_url"]
            }))
            .unwrap(),
        );
    }
    descriptor.validate().unwrap();
    descriptor
}

struct ObservingProvider {
    store: PgControlPlaneStore,
    calls: AtomicUsize,
    pending: Mutex<Option<NotificationSendLog>>,
    reply: Mutex<Result<Value, String>>,
}

#[async_trait::async_trait]
impl NotificationProviderPort for ObservingProvider {
    async fn available(
        &self,
        _: &ActorContext,
        _: Uuid,
        _: &ContributionId,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn send(
        &self,
        actor: &ActorContext,
        _: Uuid,
        _: &NotificationTypeDescriptor,
        _: Value,
        _: Value,
    ) -> anyhow::Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let pending = self
            .store
            .list_notification_send_logs(
                actor.current_workspace_id,
                NotificationLogListInput {
                    status: Some("pending".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            pending.total, 1,
            "invocation is durable before provider I/O"
        );
        let pending = pending.items.into_iter().next().unwrap();
        assert!(pending.completed_at.is_none() && pending.failure_code.is_none());
        *self.pending.lock().unwrap() = Some(pending);
        self.reply
            .lock()
            .unwrap()
            .clone()
            .map_err(anyhow::Error::msg)
    }
}

struct Fixture {
    store: PgControlPlaneStore,
    actor: ActorContext,
    service: NotificationService<PgControlPlaneStore>,
    provider: Arc<ObservingProvider>,
    instance_id: Uuid,
}

impl Fixture {
    async fn new(descriptor: NotificationTypeDescriptor) -> Self {
        let (store, workspace, root) = super::support::seed_store().await;
        let actor = store
            .load_actor_context(root.id, workspace.tenant_id, workspace.id, None)
            .await
            .unwrap();
        let installation = store
            .upsert_installation(&UpsertPluginInstallationInput {
                installation_id: Uuid::now_v7(),
                category: domain::ExtensionCategory::RuntimeExtensions,
                organization: "acme".into(),
                provider_code: "log-fixture".into(),
                plugin_id: "acme.log-fixture".into(),
                plugin_version: "1.0.0".into(),
                contract_version: "1flowbase.extension-bus/v1".into(),
                protocol: "stdio_json".into(),
                display_name: "Log mapping fixture".into(),
                source_kind: "uploaded".into(),
                trust_level: "unverified".into(),
                verification_status: domain::PluginVerificationStatus::Valid,
                desired_state: domain::PluginDesiredState::ActiveRequested,
                expected_checksum: None,
                signature_status: domain::ExtensionSignatureStatus::Missing,
                signature_algorithm: None,
                signing_key_id: None,
                metadata_json: json!({"notification_types":[descriptor]}),
                is_system_reserved: false,
                actor_user_id: root.id,
            })
            .await
            .unwrap();
        store
            .create_assignment(&CreatePluginAssignmentInput {
                installation_id: installation.id,
                workspace_id: workspace.id,
                provider_code: installation.provider_code,
                actor_user_id: root.id,
            })
            .await
            .unwrap();
        let provider = Arc::new(ObservingProvider {
            store: store.clone(),
            calls: AtomicUsize::new(0),
            pending: Mutex::new(None),
            reply: Mutex::new(Ok(json!({"receipt":"receipt-1"}))),
        });
        let service = NotificationService::new(store.clone(), MASTER_KEY.into())
            .with_provider(provider.clone());
        let instance = service
            .create_instance(
                &actor,
                CreateNotificationInstance {
                    type_code: descriptor.type_code,
                    installation_id: Some(installation.id),
                    display_name: "Log mapping fixture".into(),
                    config_json: json!({"token":CONFIG_SECRET}),
                    status: None,
                },
            )
            .await
            .unwrap();
        let fixture = Self {
            store,
            actor,
            service,
            provider,
            instance_id: instance.record.id,
        };
        if descriptor.authorization.is_some() {
            let state = fixture
                .store
                .resolve_notification_authorization(workspace.id, fixture.instance_id, MASTER_KEY)
                .await
                .unwrap();
            fixture
                .store
                .save_notification_authorization(SaveNotificationAuthorizationInput {
                    workspace_id: workspace.id,
                    instance_id: fixture.instance_id,
                    expected_revision: state.revision,
                    authorization: serde_json::from_value(json!({"status":"authorized"})).unwrap(),
                    managed_secrets: BTreeMap::from([
                        ("bot_token".into(), json!(MANAGED_SECRET)),
                        ("qr_credential".into(), json!(QR_CREDENTIAL)),
                        ("qr_code_url".into(), json!(QR_URL)),
                    ]),
                    master_key: MASTER_KEY.into(),
                })
                .await
                .unwrap()
                .unwrap();
            fixture.reply(managed_success(json!({"receipt":"receipt-1"}), json!({})));
        }
        fixture
    }

    fn reply(&self, reply: Value) {
        *self.provider.reply.lock().unwrap() = Ok(reply);
    }

    async fn send(&self, input: Value) -> anyhow::Result<domain::NotificationSendResult> {
        self.service
            .send(
                &self.actor,
                NotificationSource::Api,
                self.instance_id,
                input,
            )
            .await
    }

    async fn latest(&self) -> NotificationSendLog {
        self.service
            .logs(
                &self.actor,
                NotificationLogListInput {
                    instance_id: Some(self.instance_id),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
            .items
            .into_iter()
            .next()
            .unwrap()
    }

    fn pending(&self) -> NotificationSendLog {
        self.provider.pending.lock().unwrap().clone().unwrap()
    }
}

fn managed_success(result: Value, patch: Value) -> Value {
    json!({"type":"send", "outcome":{"status":"succeeded","result":result},
        "managed_secret_patch":patch})
}

fn assert_message(log: &NotificationSendLog, title: &str, content: &str, extra: Value) {
    assert_eq!(log.title.as_deref(), Some(title));
    assert_eq!(log.content.as_deref(), Some(content));
    assert_eq!(log.extra, extra);
}

fn assert_metadata_only(log: &NotificationSendLog) {
    assert!(log.title.is_none() && log.content.is_none());
    assert_eq!(log.extra, json!({}));
}

#[tokio::test]
async fn declared_log_is_durable_before_provider_and_merges_only_mapped_result_fields() {
    let fixture = Fixture::new(descriptor(false)).await;
    let body = format!("{}\n**Complete body** <tag>", "汉😀".repeat(300));
    let context = json!({"ids":[1,2],"ok":true,"nested":{"empty":null}});
    fixture.reply(json!({
        "receipt":"receipt-1", "context":{"provider":"accepted"},
        "skipped":"unselected-provider-field"
    }));
    fixture
        .send(json!({
            "title":"Explicit title", "body":body, "context":context,
            "skipped":"unselected-input-field"
        }))
        .await
        .unwrap();
    let pending = fixture.pending();
    assert_eq!(pending.status, "pending");
    assert_message(
        &pending,
        "Explicit title",
        &body,
        json!({"context":context}),
    );
    let log = fixture.latest().await;
    assert_eq!(log.id, pending.id);
    assert_eq!(log.status, "success");
    assert!(log.completed_at.is_some());
    assert_message(
        &log,
        "Explicit title",
        &body,
        json!({"context":context,"receipt":"receipt-1","result_context":{"provider":"accepted"}}),
    );
    let detail = fixture.service.log(&fixture.actor, log.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(detail).unwrap(),
        serde_json::to_value(&log).unwrap()
    );
    let stored_type: String =
        sqlx::query_scalar("SELECT pg_typeof(extra)::text FROM notification_send_logs WHERE id=$1")
            .bind(log.id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(stored_type, "jsonb");
    let public = serde_json::to_string(&log).unwrap();
    assert!(!public.contains("unselected-input-field"));
    assert!(!public.contains("unselected-provider-field"));

    // A final log cannot be re-snapshotted or completed by a late writer.
    assert!(fixture
        .store
        .snapshot_notification_send_log(
            fixture.actor.current_workspace_id,
            log.id,
            NotificationLogSnapshot {
                title: "late".into(),
                content: "late".into(),
                extra: json!({"late":true}),
            },
        )
        .await
        .is_err());
    assert!(fixture
        .store
        .complete_notification_send_log(
            fixture.actor.current_workspace_id,
            log.id,
            "failure",
            Some("late"),
            Some(json!({"late":true})),
        )
        .await
        .is_err());
    assert_eq!(
        serde_json::to_value(fixture.latest().await).unwrap(),
        serde_json::to_value(&log).unwrap()
    );

    fixture.reply(json!({"receipt":"receipt-2"}));
    for title in [None, Some(""), Some(" \n ")] {
        let mut input = json!({"body":body});
        if let Some(title) = title {
            input["title"] = json!(title);
        }
        fixture.send(input).await.unwrap();
        let expected_title = format!("{}汉", "汉😀".repeat(127));
        assert_message(&fixture.pending(), &expected_title, &body, json!({}));
        assert_message(
            &fixture.latest().await,
            &expected_title,
            &body,
            json!({"receipt":"receipt-2"}),
        );
    }
}

#[tokio::test]
async fn invalid_inputs_are_metadata_only_and_unknown_provider_outcomes_keep_the_snapshot() {
    let fixture = Fixture::new(descriptor(false)).await;
    for input in [
        json!({"body":42,"skipped":"invalid-payload-marker"}),
        json!({"skipped":"invalid-payload-marker"}),
        json!({"body":"invalid-payload-marker","undeclared":true}),
    ] {
        fixture.send(input).await.unwrap_err();
        let log = fixture.latest().await;
        assert_eq!(log.status, "failure");
        assert_eq!(log.failure_code.as_deref(), Some("input_invalid"));
        assert_metadata_only(&log);
        assert!(!serde_json::to_string(&log)
            .unwrap()
            .contains("invalid-payload-marker"));
    }
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    for (reply, code) in [
        (
            Err(format!("provider failed with {CONFIG_SECRET}")),
            "provider_result_unknown",
        ),
        (Ok(json!({"receipt":42})), "provider_output_invalid"),
        (
            Ok(json!({"receipt":CONFIG_SECRET})),
            "provider_output_invalid",
        ),
        (
            Ok(json!({"receipt":"safe","context":{(CONFIG_SECRET):"safe"}})),
            "provider_output_invalid",
        ),
        (
            Ok(json!({"receipt":"safe","skipped":CONFIG_SECRET})),
            "provider_output_invalid",
        ),
        (
            Ok(json!({"receipt":"safe","skipped":42})),
            "provider_output_invalid",
        ),
    ] {
        *fixture.provider.reply.lock().unwrap() = reply;
        let error = fixture
            .send(json!({"body":"Valid message before provider failure","context":{"attempt":1}}))
            .await
            .unwrap_err();
        assert!(!error.to_string().contains(CONFIG_SECRET));
        let log = fixture.latest().await;
        assert_eq!(log.status, "unknown");
        assert_eq!(log.failure_code.as_deref(), Some(code));
        assert_message(
            &log,
            "Valid message before provider failure",
            "Valid message before provider failure",
            json!({"context":{"attempt":1}}),
        );
        assert_eq!(fixture.pending().id, log.id);
        assert!(!serde_json::to_string(&log).unwrap().contains(CONFIG_SECRET));
    }
}

#[tokio::test]
async fn config_and_managed_secrets_in_message_extra_values_or_keys_never_reach_provider_or_logs() {
    for managed in [false, true] {
        let fixture = Fixture::new(descriptor(managed)).await;
        let mut inputs = vec![
            json!({"body":format!("echo {CONFIG_SECRET}")}),
            json!({"title":CONFIG_SECRET,"body":"Safe body"}),
            json!({"body":"Safe body","context":{"nested":[CONFIG_SECRET]}}),
            json!({"body":"Safe body","context":{(CONFIG_SECRET):"safe"}}),
            // The mapping's destination key is checked even when its value is harmless.
            json!({"body":"Safe body","alias_probe":"harmless"}),
        ];
        if managed {
            inputs.extend([
                json!({"body":MANAGED_SECRET}),
                json!({"body":"Safe body","context":{"nested":[MANAGED_SECRET]}}),
                json!({"body":"Safe body","context":{(MANAGED_SECRET):"safe"}}),
                json!({"body":QR_CREDENTIAL}),
                // The authorization view's cached-QR exemption is not a log exemption.
                json!({"body":"Safe body","context":{"user_action":{"qr_code_url":QR_URL}}}),
                json!({"body":"Safe body","context":{"authorization":{"user_action":{"qr_code_url":QR_URL}}}}),
            ]);
        }
        for input in inputs {
            let error = fixture.send(input).await.unwrap_err();
            assert!(!error.to_string().contains(CONFIG_SECRET));
            let log = fixture.latest().await;
            assert_eq!(log.status, "failure");
            assert_eq!(log.failure_code.as_deref(), Some("input_invalid"));
            assert_metadata_only(&log);
        }
        assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn managed_send_keeps_snapshot_for_failed_unknown_and_local_authorization_outcomes() {
    let fixture = Fixture::new(descriptor(true)).await;
    fixture.reply(managed_success(
        json!({"receipt":"managed-receipt","skipped":"unselected-managed-result"}),
        json!({"rotated_token":"safe-new-managed-value"}),
    ));
    fixture
        .send(json!({"body":"Managed body","context":{"channel":"ops"}}))
        .await
        .unwrap();
    assert_message(
        &fixture.pending(),
        "Managed body",
        "Managed body",
        json!({"context":{"channel":"ops"}}),
    );
    assert_message(
        &fixture.latest().await,
        "Managed body",
        "Managed body",
        json!({"context":{"channel":"ops"},"receipt":"managed-receipt"}),
    );
    let state = fixture
        .store
        .resolve_notification_authorization(
            fixture.actor.current_workspace_id,
            fixture.instance_id,
            MASTER_KEY,
        )
        .await
        .unwrap();
    assert_eq!(
        state.managed_secrets["rotated_token"],
        "safe-new-managed-value"
    );
    for (outcome, status, code) in [
        (
            json!({"status":"failed","failure_code":"provider_rejected"}),
            "failure",
            "provider_rejected",
        ),
        (
            json!({"status":"unknown","failure_code":"provider_result_unknown"}),
            "unknown",
            "provider_result_unknown",
        ),
    ] {
        fixture.reply(json!({"type":"send","outcome":outcome}));
        fixture
            .send(json!({"body":"Retained managed message","context":{"attempt":2}}))
            .await
            .unwrap_err();
        let log = fixture.latest().await;
        assert_eq!(log.status, status);
        assert_eq!(log.failure_code.as_deref(), Some(code));
        assert_message(
            &log,
            "Retained managed message",
            "Retained managed message",
            json!({"context":{"attempt":2}}),
        );
        assert_eq!(fixture.pending().id, log.id);
    }
    fixture
        .store
        .save_notification_authorization(SaveNotificationAuthorizationInput {
            workspace_id: fixture.actor.current_workspace_id,
            instance_id: fixture.instance_id,
            expected_revision: state.revision,
            authorization: serde_json::from_value(json!({"status":"unauthorized"})).unwrap(),
            managed_secrets: BTreeMap::new(),
            master_key: MASTER_KEY.into(),
        })
        .await
        .unwrap()
        .unwrap();
    let calls = fixture.provider.calls.load(Ordering::SeqCst);
    fixture
        .send(json!({"body":"Awaiting authorization"}))
        .await
        .unwrap_err();
    let log = fixture.latest().await;
    assert_eq!(log.failure_code.as_deref(), Some("authorization_required"));
    assert_message(
        &log,
        "Awaiting authorization",
        "Awaiting authorization",
        json!({}),
    );
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), calls);
}

#[tokio::test]
async fn managed_output_and_new_patch_secrets_cannot_enter_result_extra_or_its_destination_keys() {
    let fixture = Fixture::new(descriptor(true)).await;
    for (result, patch, code) in [
        (
            json!({"receipt":CONFIG_SECRET}),
            json!({}),
            "provider_result_unknown",
        ),
        (
            json!({"receipt":MANAGED_SECRET}),
            json!({}),
            "provider_result_unknown",
        ),
        (
            json!({"receipt":"safe","context":{(MANAGED_SECRET):"safe"}}),
            json!({}),
            "provider_result_unknown",
        ),
        (
            json!({"receipt":NEW_SECRET}),
            json!({"rotated_token":NEW_SECRET}),
            "provider_result_unknown",
        ),
        (
            json!({"receipt":"safe","context":{(NEW_SECRET):"safe"}}),
            json!({"rotated_token":NEW_SECRET}),
            "provider_result_unknown",
        ),
        (
            json!({"receipt":"safe","skipped":NEW_SECRET}),
            json!({"rotated_token":NEW_SECRET}),
            "provider_result_unknown",
        ),
        (
            json!({"receipt":"safe","alias_probe":"harmless"}),
            json!({"rotated_token":NEW_SECRET}),
            "provider_output_invalid",
        ),
        (
            json!({"receipt":"safe","context":{"user_action":{"qr_code_url":QR_URL}}}),
            json!({}),
            "provider_result_unknown",
        ),
        (json!({"receipt":42}), json!({}), "provider_result_unknown"),
    ] {
        fixture.reply(managed_success(result, patch));
        fixture
            .send(json!({"body":"Public message","context":{"request":"public"}}))
            .await
            .unwrap_err();
        let log = fixture.latest().await;
        assert_eq!(log.status, "unknown");
        assert_eq!(log.failure_code.as_deref(), Some(code));
        assert_message(
            &log,
            "Public message",
            "Public message",
            json!({"context":{"request":"public"}}),
        );
        let public = serde_json::to_string(&log).unwrap();
        for secret in [
            CONFIG_SECRET,
            MANAGED_SECRET,
            NEW_SECRET,
            QR_CREDENTIAL,
            QR_URL,
        ] {
            assert!(!public.contains(secret));
        }
        let state = fixture
            .store
            .resolve_notification_authorization(
                fixture.actor.current_workspace_id,
                fixture.instance_id,
                MASTER_KEY,
            )
            .await
            .unwrap();
        assert!(
            !state.managed_secrets.contains_key("rotated_token"),
            "unsafe output cannot commit the provider patch"
        );
    }
}

#[tokio::test]
async fn legacy_descriptor_without_log_keeps_metadata_only_and_empty_jsonb_extra() {
    let mut legacy = descriptor(false);
    legacy.log = None;
    let fixture = Fixture::new(legacy).await;
    fixture.reply(json!({"receipt":"legacy-result","context":{"raw":"not logged"}}));
    fixture
        .send(json!({"title":"Legacy title","body":"Legacy body","context":{"raw":"not logged"}}))
        .await
        .unwrap();
    assert_metadata_only(&fixture.pending());
    let log = fixture.latest().await;
    assert_eq!(log.status, "success");
    assert_metadata_only(&log);
    let stored: Value = sqlx::query_scalar("SELECT extra FROM notification_send_logs WHERE id=$1")
        .bind(log.id)
        .fetch_one(fixture.store.pool())
        .await
        .unwrap();
    assert_eq!(stored, json!({}));
}

#[tokio::test]
async fn finalization_failure_keeps_external_send_pending_and_rolls_back_system_inbox() {
    let fixture = Fixture::new(descriptor(false)).await;
    let system_id = fixture
        .service
        .list_instances(&fixture.actor)
        .await
        .unwrap()
        .into_iter()
        .find(|instance| instance.record.type_code == "system")
        .unwrap()
        .record
        .id;
    sqlx::query("CREATE FUNCTION reject_log_finalization() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture finalize unavailable'; END $$")
        .execute(fixture.store.pool()).await.unwrap();
    // Input snapshot updates must still succeed; only the terminal transition is unavailable.
    sqlx::query("CREATE TRIGGER log_finalization_fixture BEFORE UPDATE ON notification_send_logs FOR EACH ROW WHEN (NEW.status <> OLD.status) EXECUTE FUNCTION reject_log_finalization()")
        .execute(fixture.store.pool()).await.unwrap();
    let external = fixture
        .send(json!({"body":"Externally accepted","context":{"request":1}}))
        .await;
    let system = fixture.service.send(&fixture.actor, NotificationSource::Api, system_id,
        json!({"user_ids":[fixture.actor.user_id],"title":"System title","content":"System body"})).await;
    sqlx::query("DROP TRIGGER log_finalization_fixture ON notification_send_logs")
        .execute(fixture.store.pool())
        .await
        .unwrap();
    assert!(external.is_err() && system.is_err());
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 1);
    let pending = fixture.latest().await;
    assert_eq!(pending.status, "pending");
    assert!(pending.completed_at.is_none() && pending.failure_code.is_none());
    assert_message(
        &pending,
        "Externally accepted",
        "Externally accepted",
        json!({"context":{"request":1}}),
    );
    assert_eq!(fixture.pending().id, pending.id);
    let system_log = fixture
        .service
        .logs(
            &fixture.actor,
            NotificationLogListInput {
                instance_id: Some(system_id),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .items
        .remove(0);
    assert_eq!(system_log.status, "pending");
    assert_message(&system_log, "System title", "System body", json!({}));
    assert!(system_log.recipient_count.is_none() && system_log.completed_at.is_none());
    assert!(fixture
        .service
        .messages(&fixture.actor, Default::default())
        .await
        .unwrap()
        .items
        .is_empty());

    let other = fixture
        .store
        .upsert_workspace(fixture.actor.tenant_id, "Foreign log workspace")
        .await
        .unwrap();
    let foreign = fixture
        .store
        .load_actor_context(
            fixture.actor.user_id,
            fixture.actor.tenant_id,
            other.id,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        fixture
            .service
            .logs(&foreign, Default::default())
            .await
            .unwrap()
            .total,
        0
    );
    assert!(fixture.service.log(&foreign, pending.id).await.is_err());
    assert!(fixture
        .store
        .snapshot_notification_send_log(
            other.id,
            pending.id,
            NotificationLogSnapshot {
                title: "Foreign".into(),
                content: "Foreign".into(),
                extra: json!({"foreign":true})
            }
        )
        .await
        .is_err());
    assert!(fixture
        .store
        .complete_notification_send_log(
            other.id,
            pending.id,
            "success",
            None,
            Some(json!({"foreign":true}))
        )
        .await
        .is_err());
    assert_eq!(
        serde_json::to_value(fixture.latest().await).unwrap(),
        serde_json::to_value(pending).unwrap()
    );
}

#[tokio::test]
async fn numeric_managed_credentials_and_new_patches_never_enter_log_projections() {
    for secret in [json!(726493), json!(726493.0)] {
        let fixture = Fixture::new(descriptor(true)).await;
        let mut state = fixture
            .store
            .resolve_notification_authorization(
                fixture.actor.current_workspace_id,
                fixture.instance_id,
                MASTER_KEY,
            )
            .await
            .unwrap();
        state.managed_secrets.insert("rotated_token".into(), secret);
        fixture
            .store
            .save_notification_authorization(SaveNotificationAuthorizationInput {
                workspace_id: fixture.actor.current_workspace_id,
                instance_id: fixture.instance_id,
                expected_revision: state.revision,
                authorization: state.authorization.unwrap(),
                managed_secrets: state.managed_secrets,
                master_key: MASTER_KEY.into(),
            })
            .await
            .unwrap()
            .unwrap();
        for input in [
            json!({"body":"Public message","context":{"otp":726493}}),
            json!({"body":"Public message","context":{"otp":726493.0}}),
            json!({"body":"Private OTP: 726493"}),
            json!({"body":"Private OTP: 726493.0"}),
            json!({"body":"Public message","context":{"726493":"value"}}),
            json!({"body":"Public message","context":{"726493.0":"value"}}),
        ] {
            fixture.send(input).await.unwrap_err();
            assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
            let log = fixture.latest().await;
            assert_eq!(log.failure_code.as_deref(), Some("input_invalid"));
            assert_metadata_only(&log);
            assert!(!serde_json::to_string(&log).unwrap().contains("726493"));
        }
    }

    let fixture = Fixture::new(descriptor(true)).await;
    for secret in [json!(726493), json!(726493.0)] {
        for context in [
            json!({"otp":726493}),
            json!({"otp":726493.0}),
            json!({"otp":"value:726493"}),
            json!({"otp":"value:726493.0"}),
            json!({"726493":"safe"}),
            json!({"726493.0":"safe"}),
        ] {
            fixture.reply(managed_success(
                json!({"receipt":"safe","context":context}),
                json!({"rotated_token":secret}),
            ));
            fixture
                .send(json!({"body":"Public message"}))
                .await
                .unwrap_err();
            let log = fixture.latest().await;
            assert_eq!(log.status, "unknown");
            assert_eq!(log.failure_code.as_deref(), Some("provider_output_invalid"));
            assert_message(&log, "Public message", "Public message", json!({}));
            let state = fixture
                .store
                .resolve_notification_authorization(
                    fixture.actor.current_workspace_id,
                    fixture.instance_id,
                    MASTER_KEY,
                )
                .await
                .unwrap();
            assert!(!state.managed_secrets.contains_key("rotated_token"));
        }
    }
    // Boolean/null are ordinary protocol state, and unrelated numeric data is still public.
    fixture.reply(managed_success(
        json!({"receipt":"safe","context":{"accepted":true,"empty":null,"count":42}}),
        json!({"rotated_token":true}),
    ));
    fixture
        .send(json!({"body":"Public message"}))
        .await
        .unwrap();
    assert_eq!(
        fixture.latest().await.extra,
        json!({
            "receipt":"safe","result_context":{"accepted":true,"empty":null,"count":42}
        })
    );
}

#[tokio::test]
async fn numeric_credentials_keep_their_original_exponent_spelling_private() {
    let secret = json!(1e16);
    let spelling = secret.to_string();
    assert_eq!(spelling, "1e+16");
    let fixture = Fixture::new(descriptor(true)).await;
    let mut state = fixture
        .store
        .resolve_notification_authorization(
            fixture.actor.current_workspace_id,
            fixture.instance_id,
            MASTER_KEY,
        )
        .await
        .unwrap();
    state
        .managed_secrets
        .insert("rotated_token".into(), secret.clone());
    fixture
        .store
        .save_notification_authorization(SaveNotificationAuthorizationInput {
            workspace_id: fixture.actor.current_workspace_id,
            instance_id: fixture.instance_id,
            expected_revision: state.revision,
            authorization: state.authorization.unwrap(),
            managed_secrets: state.managed_secrets,
            master_key: MASTER_KEY.into(),
        })
        .await
        .unwrap()
        .unwrap();
    for input in [
        json!({"body":format!("Credential: {spelling}")}),
        json!({"body":"Public message","context":{"token":spelling}}),
        json!({"body":"Public message","context":{(spelling.clone()):"value"}}),
    ] {
        fixture.send(input).await.unwrap_err();
        assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), 0);
        let log = fixture.latest().await;
        assert_eq!(log.failure_code.as_deref(), Some("input_invalid"));
        assert_metadata_only(&log);
    }

    let fixture = Fixture::new(descriptor(true)).await;
    for context in [
        json!({"token":spelling}),
        json!({(spelling.clone()):"value"}),
    ] {
        fixture.reply(managed_success(
            json!({"receipt":"safe","context":context}),
            json!({"rotated_token":secret}),
        ));
        fixture
            .send(json!({"body":"Public message"}))
            .await
            .unwrap_err();
        let log = fixture.latest().await;
        assert_eq!(log.status, "unknown");
        assert_eq!(log.failure_code.as_deref(), Some("provider_output_invalid"));
        assert_message(&log, "Public message", "Public message", json!({}));
        let state = fixture
            .store
            .resolve_notification_authorization(
                fixture.actor.current_workspace_id,
                fixture.instance_id,
                MASTER_KEY,
            )
            .await
            .unwrap();
        assert!(!state.managed_secrets.contains_key("rotated_token"));
    }
}

#[tokio::test]
async fn numeric_log_credentials_preserve_distinct_large_integer_values() {
    let fixture = Fixture::new(descriptor(true)).await;
    for (secret, public) in [
        (
            json!(9_007_199_254_740_993_u64),
            json!(9_007_199_254_740_992.0),
        ),
        (json!(u64::MAX), json!(18_446_744_073_709_551_616.0)),
    ] {
        fixture.reply(managed_success(
            json!({"receipt":"safe","context":{"count":public}}),
            json!({"rotated_token":secret}),
        ));
        fixture
            .send(json!({"body":"Public message"}))
            .await
            .unwrap();
        let log = fixture.latest().await;
        assert_eq!(log.status, "success");
        assert_eq!(log.extra["result_context"]["count"], public);
        // The now-persisted credential must not block distinct input business data either.
        fixture.reply(managed_success(json!({"receipt":"safe"}), json!({})));
        fixture
            .send(json!({"body":"Public message","context":{"count":public}}))
            .await
            .unwrap();
        assert_eq!(fixture.latest().await.extra["context"]["count"], public);
    }
}

#[tokio::test]
async fn system_success_preserves_the_host_generated_title_and_complete_body() {
    let fixture = Fixture::new(descriptor(false)).await;
    let system_id = fixture
        .service
        .list_instances(&fixture.actor)
        .await
        .unwrap()
        .into_iter()
        .find(|instance| instance.record.type_code == "system")
        .unwrap()
        .record
        .id;
    let content = "系统🙂".repeat(180);
    fixture
        .service
        .send(
            &fixture.actor,
            NotificationSource::Api,
            system_id,
            json!({"user_ids":[fixture.actor.user_id],"title":"  ","content":content}),
        )
        .await
        .unwrap();
    let log = fixture
        .service
        .logs(
            &fixture.actor,
            NotificationLogListInput {
                instance_id: Some(system_id),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .items
        .remove(0);
    assert_eq!(log.status, "success");
    assert_eq!(log.recipient_count, Some(1));
    assert_message(
        &log,
        &content.chars().take(255).collect::<String>(),
        &content,
        json!({}),
    );
    // Logging does not rewrite the user's separately committed inbox message.
    let inbox = fixture
        .service
        .messages(&fixture.actor, Default::default())
        .await
        .unwrap();
    assert_eq!(inbox.items.len(), 1);
    assert_eq!(inbox.items[0].title, "  ");
    assert_eq!(inbox.items[0].content, content);
}
