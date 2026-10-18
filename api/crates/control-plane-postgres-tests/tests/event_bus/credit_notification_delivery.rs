//! Real optional publication → frozen typed registry → dispatcher → idempotent PG effects.
//! These are test-only notification recipients; neither recipient owns monetary execution.
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use async_trait::async_trait;
use control_plane::lifecycle_outbox_dispatcher::{
    LifecycleDeliveryCompletionPort, LifecycleFactDeliveryCompletion, LifecycleFactDeliveryPort,
    LifecycleOutboxDispatcher,
};
use control_plane_contracts::ports::*;
use extension_contracts::{
    AfterCommitFact, CompletionOutcome, LifecycleContract, LifecycleFactId, LifecycleTransactionId,
};
use plugin_framework::extension_bus::*;
use serde_json::{json, Value};
use storage_durable_postgres::PgControlPlaneStore;
use time::OffsetDateTime;
use uuid::Uuid;

struct FixtureRecipient {
    pool: sqlx::PgPool,
    subscriber: &'static str,
    fail_before_effect: AtomicBool,
    lose_ack_after_effect: AtomicBool,
    transport_event_id: Arc<Mutex<Option<Uuid>>>,
}
impl TypedLifecycleSubscriberHandler<CreditNotificationFact> for FixtureRecipient {
    fn handle<'a>(
        &'a self,
        fact: &'a AfterCommitFact<CreditNotificationFact>,
    ) -> LifecycleHandlerFuture<'a> {
        Box::pin(async move {
            let effect = async {
                anyhow::ensure!(fact.payload().event_type == CreditNotificationEventType::CreditCharged,
                    "fixture accepts only an already committed charge notification");
                if self.fail_before_effect.load(Ordering::SeqCst) {
                    anyhow::bail!("recipient B unavailable before its effect");
                }
                let source = Uuid::parse_str(fact.fact_id().as_str())?;
                // The observable recipient effect is durable, keyed by original fact and target.
                // Conflict means a safe repeat of this exact effect, never a second debit.
                sqlx::query("insert into notification_fixture_effects(source_event_id,subscriber_id,payload) values ($1,$2,$3) on conflict(source_event_id,subscriber_id) do nothing")
                    .bind(source).bind(self.subscriber).bind(serde_json::to_value(fact.payload())?)
                    .execute(&self.pool).await?;
                if self.lose_ack_after_effect.swap(false, Ordering::SeqCst) {
                    let event_id = self.transport_event_id.lock().unwrap().expect("publication precedes delivery");
                    // Crash/lease loss after effect commit and before ACK: the real repository's
                    // ACK fence now refuses the old claim. No fake repository returns success.
                    let expired = sqlx::query("update lifecycle_outbox_deliveries set claimed_at=clock_timestamp()-interval '2 seconds',claim_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and subscriber_id=$2 and status='claimed'")
                        .bind(event_id).bind(self.subscriber).execute(&self.pool).await?;
                    anyhow::ensure!(expired.rows_affected() == 1, "claimed target must exist");
                }
                Ok::<(), anyhow::Error>(())
            }.await;
            effect.map_err(|error| LifecycleHandlerError::new(error.to_string()))
        })
    }
}

struct RegistryDelivery {
    registry: EffectiveLifecycleHandlerRegistry,
    plan: EffectiveLifecycleSubscriberPlan,
}
#[async_trait]
impl LifecycleFactDeliveryPort for RegistryDelivery {
    async fn deliver(&self, fact: &LifecycleOutboxRecord) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.plan
                .subscribers()
                .iter()
                .any(|target| target.subscriber_id == fact.subscriber_id
                    && target.handler_id == fact.handler_id
                    && target.handler_version == fact.handler_version
                    && target.fact_contract_id == fact.contract_id
                    && target.fact_contract_version == fact.contract_version),
            "unknown frozen target"
        );
        self.registry
            .deliver(
                &fact.graph_fingerprint,
                &fact.handler_id,
                &fact.handler_version,
                &fact.contract_id,
                &fact.contract_version,
                &fact.canonical_payload,
            )
            .await?;
        Ok(())
    }
}
#[derive(Default)]
struct Completions(Mutex<Vec<CompletionOutcome<LifecycleFactDeliveryCompletion>>>);
impl LifecycleDeliveryCompletionPort for Completions {
    fn complete(&self, outcome: CompletionOutcome<LifecycleFactDeliveryCompletion>) {
        self.0.lock().unwrap().push(outcome);
    }
}

fn plan() -> EffectiveLifecycleSubscriberPlan {
    let point = ExtensionPointId::new("fixture.credit.after-commit").unwrap();
    let core_id = ModuleId::new("fixture.core").unwrap();
    let mut modules = vec![ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: core_id.clone(),
        module_version: ModuleVersion::new("1.0.0").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::new(),
        extension_points: vec![ExtensionPointDescriptor {
            point_id: point.clone(),
            owner_module_id: core_id,
            point_kind: ExtensionPointKind::EventStream,
            contract: ContractDescriptor::new("fixture.credit.lane", "v1").unwrap(),
            scope: ScopeSemantics::Global,
            cardinality: Cardinality::Many,
            ordering: OrderingSemantics::Dependency,
            failure: FailureSemantics::IsolateContribution,
            delivery: DeliverySemantics::AfterCommitDurable,
            lifecycle: LifecycleSemantics::Invocation,
            allowed_permissions: BTreeSet::new(),
            override_policy: OverridePolicy::Sealed,
        }],
        contributions: vec![],
    }];
    let mut bindings = Vec::new();
    for subscriber in ["a", "b"] {
        let module = ModuleId::new(format!("fixture.credit.{subscriber}")).unwrap();
        let contribution =
            ContributionId::new(format!("fixture.credit.{subscriber}.subscription")).unwrap();
        modules.push(ModuleDescriptor {
            bus_version: ExtensionBusVersion::V1,
            module_id: module.clone(),
            module_version: ModuleVersion::new("1.0.0").unwrap(),
            module_kind: ModuleKind::TrustedHost,
            activation: ModuleActivationDeclaration::Active,
            dependencies: BTreeSet::new(),
            granted_permissions: BTreeSet::new(),
            extension_points: vec![],
            contributions: vec![ContributionDescriptor {
                contribution_id: contribution.clone(),
                contributor_module_id: module,
                point_id: point.clone(),
                contract_version: ContractVersion::new("v1").unwrap(),
                required_permissions: BTreeSet::new(),
                mode: ContributionMode::Append,
                ordering: ContributionOrdering::default(),
            }],
        });
        bindings.push(LifecycleSubscriberBinding {
            contribution_id: contribution,
            subscription_id: subscriber.into(),
            point_id: point.clone(),
            fact_contract_id: CreditNotificationFact::CONTRACT_ID.into(),
            fact_contract_version: CreditNotificationFact::CONTRACT_VERSION.into(),
            handler_id: format!("fixture.credit.{subscriber}"),
            handler_version: "v1".into(),
        });
    }
    let graph = compile_extension_graph(modules).unwrap();
    compile_lifecycle_subscriber_plan(&graph, bindings).unwrap()
}

struct Fixture {
    store: PgControlPlaneStore,
    dispatcher: LifecycleOutboxDispatcher<PgControlPlaneStore>,
    recipient_b: Arc<FixtureRecipient>,
    completions: Arc<Completions>,
    input: PublishOptionalNotificationInput,
    event_id: Uuid,
    monetary_before: Value,
}
impl Fixture {
    async fn new(fail_b: bool, lose_b_ack: bool) -> Self {
        let pool = super::pool().await;
        sqlx::query("create table notification_fixture_effects(source_event_id uuid not null,subscriber_id text not null,payload jsonb not null,primary key(source_event_id,subscriber_id))")
            .execute(&pool).await.unwrap();
        let store = PgControlPlaneStore::new(pool.clone());
        let tenant = store.upsert_root_tenant().await.unwrap();
        let workspace = Uuid::now_v7();
        sqlx::query(
            "insert into workspaces(id,tenant_id,name) values ($1,$2,'Typed credit recipients')",
        )
        .bind(workspace)
        .bind(tenant.id)
        .execute(&pool)
        .await
        .unwrap();
        let user = Uuid::now_v7();
        sqlx::query("insert into users(id,account,email,password_hash,name,nickname,introduction,default_display_role,email_login_enabled,phone_login_enabled,status,session_version) values ($1,$2,$3,'fixture-hash','Fixture','Fixture','','member',true,false,'active',1)")
            .bind(user).bind(format!("notification-{user}"))
            .bind(format!("notification-{user}@example.com")).execute(&pool).await.unwrap();
        let command = |command: &str, amount: &str| CreditCommandInput {
            workspace_id: workspace,
            user_id: user,
            amount: amount.into(),
            credit_unit: "USD".into(),
            command: command.into(),
            reason: "notification fixture monetary prerequisite".into(),
            source_type: Some("fixture".into()),
            source_id: None,
            idempotency_key: format!("notification:{command}:{user}"),
            actor_user_id: None,
            actor_plugin_id: Some("fixture.notifications".into()),
            metadata: json!({}),
        };
        // Monetary work is completed via its real repository before notification publication.
        store
            .execute_credit_command(&command("grant", "10"))
            .await
            .unwrap();
        let charged = store
            .execute_credit_command(&command("charge", "2"))
            .await
            .unwrap();
        let monetary_before = monetary_snapshot(&pool).await;
        assert_eq!(monetary_before["ledger"].as_array().unwrap().len(), 2);
        assert_eq!(
            monetary_before["accounts"][0]["current_balance"].as_f64(),
            Some(8.0)
        );

        let plan = plan();
        let catalog = LifecyclePublicationCatalog::new([(
            (
                CreditNotificationFact::CONTRACT_ID.into(),
                CreditNotificationFact::CONTRACT_VERSION.into(),
            ),
            LifecyclePublicationPlan {
                graph_fingerprint: plan.graph_fingerprint().into(),
                subscribers: plan
                    .subscribers()
                    .iter()
                    .map(|target| LifecycleSubscriberTarget {
                        subscriber_id: target.subscriber_id.clone(),
                        handler_id: target.handler_id.clone(),
                        handler_version: target.handler_version.clone(),
                    })
                    .collect(),
            },
        )])
        .unwrap();
        let store = store.with_lifecycle_publication_catalog(catalog);
        let transport_event_id = Arc::new(Mutex::new(None));
        let recipient_a = Arc::new(FixtureRecipient {
            pool: pool.clone(),
            subscriber: "a",
            fail_before_effect: AtomicBool::new(false),
            lose_ack_after_effect: AtomicBool::new(false),
            transport_event_id: transport_event_id.clone(),
        });
        let recipient_b = Arc::new(FixtureRecipient {
            pool: pool.clone(),
            subscriber: "b",
            fail_before_effect: AtomicBool::new(fail_b),
            lose_ack_after_effect: AtomicBool::new(lose_b_ack),
            transport_event_id: transport_event_id.clone(),
        });
        let registry = compile_lifecycle_handler_registry(
            &plan,
            vec![
                LifecycleHandlerBinding::typed::<CreditNotificationFact, _>(
                    "fixture.credit.a",
                    "v1",
                    recipient_a,
                ),
                LifecycleHandlerBinding::typed::<CreditNotificationFact, _>(
                    "fixture.credit.b",
                    "v1",
                    recipient_b.clone(),
                ),
            ],
        )
        .unwrap();
        let now = OffsetDateTime::now_utc();
        let fact = AfterCommitFact::new(
            LifecycleFactId::new(charged.id.to_string()).unwrap(),
            LifecycleTransactionId::new(charged.transaction_id.to_string()).unwrap(),
            (now.unix_timestamp_nanos() / 1_000_000) as i64,
            CreditNotificationFact {
                workspace_id: workspace,
                account_id: Some(charged.account_id),
                event_type: CreditNotificationEventType::CreditCharged,
                payload: serde_json::to_value(&charged).unwrap(),
            },
        );
        let input = PublishOptionalNotificationInput {
            source_event_id: charged.id,
            transaction_id: charged.transaction_id,
            workspace_id: Some(workspace),
            occurred_at: now,
            contract: OptionalNotificationContract::Credit,
            canonical_payload: serde_json::to_vec(&fact).unwrap(),
        };
        let outcome = store.publish_optional_notification(&input).await.unwrap();
        let OptionalNotificationPublicationOutcome::Enqueued {
            event_id,
            target_count,
        } = outcome
        else {
            panic!("actual declared targets must enqueue their independent deliveries");
        };
        assert_eq!(target_count, 2);
        *transport_event_id.lock().unwrap() = Some(event_id);
        let completions = Arc::new(Completions::default());
        let dispatcher = LifecycleOutboxDispatcher::new(
            store.clone(),
            Arc::new(RegistryDelivery { registry, plan }),
            completions.clone(),
        );
        Self {
            store,
            dispatcher,
            recipient_b,
            completions,
            input,
            event_id,
            monetary_before,
        }
    }

    async fn deliveries(&self) -> Vec<(String, String, i32)> {
        sqlx::query_as("select subscriber_id,status,attempt_count from lifecycle_outbox_deliveries where event_id=$1 order by subscriber_id")
            .bind(self.event_id).fetch_all(self.store.pool()).await.unwrap()
    }
    async fn effects(&self) -> Vec<(String, Value)> {
        sqlx::query_as("select subscriber_id,payload from notification_fixture_effects where source_event_id=$1 order by subscriber_id")
            .bind(self.input.source_event_id).fetch_all(self.store.pool()).await.unwrap()
    }
    async fn transport_body(&self) -> (Vec<u8>, Option<Vec<u8>>) {
        sqlx::query_as("select canonical_payload,notification_payload_digest from lifecycle_outbox where event_id=$1")
            .bind(self.event_id).fetch_one(self.store.pool()).await.unwrap()
    }
    async fn assert_final_effects_without_new_charge(&self) {
        let expected: AfterCommitFact<CreditNotificationFact> =
            serde_json::from_slice(&self.input.canonical_payload).unwrap();
        let expected_body = serde_json::to_value(expected.payload()).unwrap();
        assert_eq!(
            self.effects().await,
            vec![
                ("a".into(), expected_body.clone()),
                ("b".into(), expected_body)
            ]
        );
        assert_eq!(
            monetary_snapshot(self.store.pool()).await,
            self.monetary_before,
            "notification delivery/retry must not repeat monetary execution or mutate account revision/time"
        );
        let parent: String =
            sqlx::query_scalar("select status from lifecycle_outbox where event_id=$1")
                .bind(self.event_id)
                .fetch_one(self.store.pool())
                .await
                .unwrap();
        assert_eq!(parent, "delivered");
        let bodies: i64 =
            sqlx::query_scalar("select count(*) from lifecycle_outbox where event_id=$1")
                .bind(self.event_id)
                .fetch_one(self.store.pool())
                .await
                .unwrap();
        assert_eq!(bodies, 1);
        let (body, digest) = self.transport_body().await;
        assert!(
            body.is_empty(),
            "last fenced target ACK must release transport body"
        );
        assert_eq!(
            digest.unwrap().len(),
            32,
            "replay receipt retains immutable body digest"
        );
        assert_eq!(
            self.store
                .publish_optional_notification(&self.input)
                .await
                .unwrap(),
            OptionalNotificationPublicationOutcome::Enqueued {
                event_id: self.event_id,
                target_count: 2
            }
        );
        let mut conflict = self.input.clone();
        conflict.canonical_payload.push(b' '); // Valid JSON but a different canonical body.
        assert!(self
            .store
            .publish_optional_notification(&conflict)
            .await
            .is_err());
        assert!(self.transport_body().await.0.is_empty());
        assert_eq!(
            monetary_snapshot(self.store.pool()).await,
            self.monetary_before
        );
    }
}

async fn monetary_snapshot(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar("select jsonb_build_object('accounts',(select coalesce(jsonb_agg(to_jsonb(a) order by id),'[]'::jsonb) from user_credit_accounts a),'ledger',(select coalesce(jsonb_agg(to_jsonb(l) order by id),'[]'::jsonb) from runtime_credit_ledger l),'sessions',(select coalesce(jsonb_agg(to_jsonb(s) order by id),'[]'::jsonb) from billing_sessions s))")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn typed_credit_notification_a_success_b_failure_retries_only_b_without_new_charge() {
    let fixture = Fixture::new(true, false).await;
    assert_eq!(fixture.dispatcher.run_once().await.unwrap(), 2);
    assert_eq!(
        fixture.deliveries().await,
        vec![
            ("a".into(), "delivered".into(), 1),
            ("b".into(), "pending".into(), 1)
        ]
    );
    assert_eq!(
        fixture.transport_body().await.0,
        fixture.input.canonical_payload,
        "failed B target keeps complete recoverable body after A ACK"
    );
    assert_eq!(fixture.effects().await.len(), 1);
    assert_eq!(fixture.effects().await[0].0, "a");
    let parent: String =
        sqlx::query_scalar("select status from lifecycle_outbox where event_id=$1")
            .bind(fixture.event_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(
        parent, "pending",
        "A ACK cannot complete B's independent responsibility"
    );
    fixture
        .recipient_b
        .fail_before_effect
        .store(false, Ordering::SeqCst);
    sqlx::query("update lifecycle_outbox_deliveries set available_at=now() where event_id=$1 and subscriber_id='b'")
        .bind(fixture.event_id).execute(fixture.store.pool()).await.unwrap();
    assert_eq!(fixture.dispatcher.run_once().await.unwrap(), 1);
    assert_eq!(
        fixture.deliveries().await,
        vec![
            ("a".into(), "delivered".into(), 1),
            ("b".into(), "delivered".into(), 2)
        ]
    );
    assert_eq!(fixture.completions.0.lock().unwrap().len(), 3);
    fixture.assert_final_effects_without_new_charge().await;
    assert_eq!(fixture.dispatcher.run_once().await.unwrap(), 0);
}

#[tokio::test]
async fn typed_credit_notification_effect_before_ack_loss_retries_idempotently_without_new_charge()
{
    let fixture = Fixture::new(false, true).await;
    assert_eq!(fixture.dispatcher.run_once().await.unwrap(), 2);
    assert_eq!(
        fixture.effects().await.len(),
        2,
        "both effects committed before the lost B ACK"
    );
    assert_eq!(
        fixture.deliveries().await,
        vec![
            ("a".into(), "delivered".into(), 1),
            ("b".into(), "claimed".into(), 1)
        ]
    );
    assert_eq!(
        fixture.transport_body().await.0,
        fixture.input.canonical_payload,
        "effect without fenced ACK cannot release transport body"
    );
    let old_claim: Uuid = sqlx::query_scalar(
        "select claim_id from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id='b'",
    )
    .bind(fixture.event_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(fixture.dispatcher.run_once().await.unwrap(), 1);
    assert_eq!(
        fixture.deliveries().await,
        vec![
            ("a".into(), "delivered".into(), 1),
            ("b".into(), "delivered".into(), 2)
        ]
    );
    let completions = fixture.completions.0.lock().unwrap();
    assert_eq!(completions.len(), 3);
    assert!(completions
        .iter()
        .any(|outcome| outcome.payload().claim_id == old_claim));
    assert!(completions
        .iter()
        .any(|outcome| outcome.payload().attempt_count == 2
            && outcome.payload().claim_id != old_claim));
    drop(completions);
    fixture.assert_final_effects_without_new_charge().await;
    assert_eq!(fixture.dispatcher.run_once().await.unwrap(), 0);
}
