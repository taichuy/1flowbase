use control_plane_contracts::ports::{LifecycleDeliveryBlocked, LifecycleDeliveryPauseReason};
use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{LifecycleOutboxRecord, LifecycleSubscriberTarget};
use control_plane_contracts::ports::{
    ModelDefinitionIntegrationNotification, OptionalNotificationContract,
    OptionalNotificationPublicationPort, PublishOptionalNotificationInput,
};
use plugin_framework::extension_bus::{
    compile_lifecycle_handler_registry, EffectiveLifecycleHandlerRegistry,
    EffectiveLifecycleSubscriber, EffectiveLifecycleSubscriberPlan, LifecycleHandlerBinding,
    LifecycleHandlerError, LifecycleHandlerFuture, TypedLifecycleSubscriberHandler,
};

use super::lifecycle_activation::HostExtensionLifecycleFactoryCatalog;

pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_ID: &str =
    "official.plugin-host.model-definition-committed";
pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_VERSION: &str = "v1";
struct ModelDefinitionCommittedHandler {
    publication: Arc<dyn OptionalNotificationPublicationPort>,
}

impl TypedLifecycleSubscriberHandler<control_plane_contracts::ports::ModelDefinitionCommittedFact>
    for ModelDefinitionCommittedHandler
{
    fn handle<'a>(
        &'a self,
        fact: &'a extension_contracts::AfterCommitFact<
            control_plane_contracts::ports::ModelDefinitionCommittedFact,
        >,
    ) -> LifecycleHandlerFuture<'a> {
        Box::pin(async move {
            let publish = async {
                // Preserve the source's timestamp; retry must not create a new fact body.
                let source = serde_json::to_value(fact)?;
                let occurred_at_unix_ms = source["occurred_at_unix_ms"]
                    .as_i64()
                    .ok_or_else(|| anyhow::anyhow!("source fact timestamp is missing"))?;
                let payload = fact.payload();
                let notification = extension_contracts::AfterCommitFact::new(
                    fact.fact_id().clone(),
                    fact.transaction_id().clone(),
                    occurred_at_unix_ms,
                    ModelDefinitionIntegrationNotification {
                        model_definition_id: payload.model_definition_id,
                        scope_kind: payload.scope_kind,
                        scope_id: payload.scope_id,
                    },
                );
                self.publication
                    .publish_optional_notification(&PublishOptionalNotificationInput {
                        source_event_id: uuid::Uuid::parse_str(fact.fact_id().as_str())?,
                        transaction_id: uuid::Uuid::parse_str(fact.transaction_id().as_str())?,
                        workspace_id: (payload.scope_kind == domain::DataModelScopeKind::Workspace)
                            .then_some(payload.scope_id),
                        occurred_at: time::OffsetDateTime::from_unix_timestamp_nanos(
                            i128::from(occurred_at_unix_ms) * 1_000_000,
                        )?,
                        contract: OptionalNotificationContract::ModelDefinition,
                        canonical_payload: serde_json::to_vec(&notification)?,
                    })
                    .await?;
                // Completion here means optional publication adjudication or durable
                // stage handoff, never completion of downstream target effects.
                Ok::<(), anyhow::Error>(())
            }
            .await;
            publish.map_err(|error| LifecycleHandlerError::new(error.to_string()))
        })
    }
}

fn official_plugin_host_lifecycle_bindings(
    publication: Arc<dyn OptionalNotificationPublicationPort>,
) -> Vec<LifecycleHandlerBinding> {
    vec![LifecycleHandlerBinding::typed::<
        control_plane_contracts::ports::ModelDefinitionCommittedFact,
        _,
    >(
        MODEL_DEFINITION_COMMITTED_HANDLER_ID,
        MODEL_DEFINITION_COMMITTED_HANDLER_VERSION,
        Arc::new(ModelDefinitionCommittedHandler { publication }),
    )]
}

pub(crate) fn production_lifecycle_handler_factories(
    publication: Arc<dyn OptionalNotificationPublicationPort>,
) -> Result<HostExtensionLifecycleFactoryCatalog> {
    let mut factories = HostExtensionLifecycleFactoryCatalog::default();
    factories.register(
        "builtin://official.plugin-host",
        "builtin_plugin_host",
        Arc::new(move || {
            Ok(official_plugin_host_lifecycle_bindings(Arc::clone(
                &publication,
            )))
        }),
    )?;
    Ok(factories)
}

pub(crate) struct ApiLifecycleFactDelivery {
    registry: EffectiveLifecycleHandlerRegistry,
    native_targets: Vec<EffectiveLifecycleSubscriber>,
    managed: Option<std::sync::Weak<crate::extension_bus::ManagedExtensionComposition>>,
}

impl ApiLifecycleFactDelivery {
    pub(crate) fn with_managed(
        mut self,
        composition: &Arc<crate::extension_bus::ManagedExtensionComposition>,
    ) -> Self {
        self.managed = Some(Arc::downgrade(composition));
        self
    }

    pub(crate) fn bind(
        plan: &EffectiveLifecycleSubscriberPlan,
        handler_bindings: Vec<LifecycleHandlerBinding>,
    ) -> Result<(
        Self,
        control_plane_contracts::ports::LifecyclePublicationCatalog,
    )> {
        let mut publications: BTreeMap<(String, String), Vec<LifecycleSubscriberTarget>> =
            BTreeMap::new();
        for subscriber in plan.subscribers() {
            publications
                .entry((
                    subscriber.fact_contract_id.clone(),
                    subscriber.fact_contract_version.clone(),
                ))
                .or_default()
                .push(LifecycleSubscriberTarget {
                    subscriber_id: subscriber.subscriber_id.clone(),
                    handler_id: subscriber.handler_id.clone(),
                    handler_version: subscriber.handler_version.clone(),
                });
        }
        let catalog = control_plane_contracts::ports::LifecyclePublicationCatalog::new(
            publications.into_iter().map(|(contract, subscribers)| {
                (
                    contract,
                    control_plane_contracts::ports::LifecyclePublicationPlan {
                        graph_fingerprint: plan.graph_fingerprint().to_string(),
                        subscribers,
                    },
                )
            }),
        )?;
        let native_targets = plan.subscribers().to_vec();
        let registry = compile_lifecycle_handler_registry(plan, handler_bindings)?;
        Ok((
            Self {
                registry,
                native_targets,
                managed: None,
            },
            catalog,
        ))
    }
}

#[async_trait]
impl control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort
    for ApiLifecycleFactDelivery
{
    async fn terminate_command(
        &self,
        fact: &LifecycleOutboxRecord,
        termination: control_plane_contracts::ports::CommunicationCommandTermination,
    ) -> Result<()> {
        let owner = self
            .managed
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| anyhow::anyhow!("command termination composition unavailable"))?;
        owner.terminate_command_delivery(fact, termination).await
    }
    async fn deliver(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
        control_plane_contracts::ports::with_lifecycle_workflow_ancestors(
            fact.observation_context
                .as_ref()
                .map(|context| context.workflow_ancestor_ids.clone())
                .unwrap_or_default(),
            self.deliver_owned(fact),
        )
        .await
    }
}

impl ApiLifecycleFactDelivery {
    async fn deliver_owned(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
        if let Some(owner) = &self.managed {
            let owner = owner.upgrade().ok_or({
                LifecycleDeliveryBlocked(LifecycleDeliveryPauseReason::FrozenGraphUnavailable)
            })?;
            if let Some(snapshot) = owner.event_snapshot_for_graph(fact).await? {
                if owner.deliver_event_record(&snapshot, fact).await? {
                    return Ok(());
                }
                // Exact native target was validated against the combined frozen plan above.
                return self
                    .registry
                    .deliver(
                        self.registry.graph_fingerprint(),
                        &fact.handler_id,
                        &fact.handler_version,
                        &fact.contract_id,
                        &fact.contract_version,
                        &fact.canonical_payload,
                    )
                    .await
                    .map_err(anyhow::Error::from);
            }
        }
        if fact.graph_fingerprint != self.registry.graph_fingerprint() {
            return Err(LifecycleDeliveryBlocked(
                LifecycleDeliveryPauseReason::FrozenGraphUnavailable,
            )
            .into());
        }
        if !self.native_targets.iter().any(|target| {
            target.subscriber_id == fact.subscriber_id
                && target.handler_id == fact.handler_id
                && target.handler_version == fact.handler_version
                && target.fact_contract_id == fact.contract_id
                && target.fact_contract_version == fact.contract_version
        }) {
            return Err(LifecycleDeliveryBlocked(
                LifecycleDeliveryPauseReason::FrozenHandlerUnavailable,
            )
            .into());
        }
        self.registry
            .deliver(
                &fact.graph_fingerprint,
                &fact.handler_id,
                &fact.handler_version,
                &fact.contract_id,
                &fact.contract_version,
                &fact.canonical_payload,
            )
            .await
            .map_err(anyhow::Error::from)
    }
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
mod tests {
    use super::*;
    use control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort;
    use control_plane_contracts::ports::{
        CreateModelDefinitionInput, LifecycleOutboxStatus, ModelDefinitionCommittedFact,
        ModelDefinitionRepository,
    };
    use extension_contracts::{AfterCommitFact, LifecycleFactId, LifecycleTransactionId};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    struct IndependentPluginHandler {
        fail: AtomicBool,
        received: Mutex<Vec<Uuid>>,
    }

    impl TypedLifecycleSubscriberHandler<ModelDefinitionCommittedFact> for IndependentPluginHandler {
        fn handle<'a>(
            &'a self,
            fact: &'a AfterCommitFact<ModelDefinitionCommittedFact>,
        ) -> LifecycleHandlerFuture<'a> {
            Box::pin(async move {
                if self.fail.load(Ordering::SeqCst) {
                    return Err(LifecycleHandlerError::new(
                        "independent plugin fixture rejected delivery",
                    ));
                }
                self.received
                    .lock()
                    .expect("received lifecycle facts mutex poisoned")
                    .push(fact.payload().model_definition_id);
                Ok(())
            })
        }
    }

    struct RecordingPublication {
        fail: AtomicBool,
        received: Mutex<Vec<PublishOptionalNotificationInput>>,
    }
    #[async_trait]
    impl OptionalNotificationPublicationPort for RecordingPublication {
        async fn publish_optional_notification(
            &self,
            input: &PublishOptionalNotificationInput,
        ) -> anyhow::Result<control_plane_contracts::ports::OptionalNotificationPublicationOutcome>
        {
            if self.fail.load(Ordering::SeqCst) {
                anyhow::bail!("declared notification publication unavailable");
            }
            self.received.lock().unwrap().push(input.clone());
            Ok(control_plane_contracts::ports::OptionalNotificationPublicationOutcome::NoTargets)
        }
    }

    #[tokio::test]
    async fn real_host_extension_subscriber_adjudicates_optional_typed_notification() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            &root,
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            Vec::new(),
        )
        .unwrap();
        let graph = assembly.compile_graph().unwrap();
        let plan = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let publication = Arc::new(RecordingPublication {
            fail: AtomicBool::new(true),
            received: Mutex::new(Vec::new()),
        });
        let bindings = production_lifecycle_handler_factories(publication.clone())
            .unwrap()
            .activate(assembly.host_extension_manifests())
            .unwrap();
        let (delivery, _) = ApiLifecycleFactDelivery::bind(&plan, bindings).unwrap();
        let event_id = Uuid::now_v7();
        let transaction_id = Uuid::now_v7();
        let fact = AfterCommitFact::new(
            LifecycleFactId::new(event_id.to_string()).unwrap(),
            LifecycleTransactionId::new(transaction_id.to_string()).unwrap(),
            1_700_000_000_000,
            ModelDefinitionCommittedFact {
                model_definition_id: Uuid::now_v7(),
                scope_kind: domain::DataModelScopeKind::System,
                scope_id: Uuid::nil(),
            },
        );
        let subscriber = &plan.subscribers()[0];
        let record = LifecycleOutboxRecord {
            observation_context: None,
            event_id,
            transaction_id,
            contract_id: subscriber.fact_contract_id.clone(),
            contract_version: subscriber.fact_contract_version.clone(),
            canonical_payload: serde_json::to_vec(&fact).unwrap(),
            occurred_at: OffsetDateTime::now_utc(),
            graph_fingerprint: plan.graph_fingerprint().to_string(),
            subscriber_id: subscriber.subscriber_id.clone(),
            handler_id: subscriber.handler_id.clone(),
            handler_version: subscriber.handler_version.clone(),
            status: LifecycleOutboxStatus::Claimed,
            attempt_count: 1,
            available_at: OffsetDateTime::now_utc(),
            claimed_by: Some(Uuid::now_v7()),
            claimed_at: Some(OffsetDateTime::now_utc()),
            claim_id: Some(Uuid::now_v7()),
            claim_expires_at: Some(OffsetDateTime::now_utc() + time::Duration::seconds(30)),
            pause_reason: None,
            paused_at: None,
            delivered_at: None,
        };
        assert!(delivery.deliver(&record).await.is_err());
        assert!(publication.received.lock().unwrap().is_empty());
        publication.fail.store(false, Ordering::SeqCst);
        delivery.deliver(&record).await.unwrap();
        let received = publication.received.lock().unwrap();
        assert_eq!(received.len(), 1);
        let staged = &received[0];
        assert_eq!(staged.source_event_id, event_id);
        assert_eq!(staged.transaction_id, transaction_id);
        assert_eq!(staged.workspace_id, None);
        assert_eq!(
            staged.contract,
            OptionalNotificationContract::ModelDefinition
        );
        let notification: AfterCommitFact<ModelDefinitionIntegrationNotification> =
            serde_json::from_slice(&staged.canonical_payload).unwrap();
        assert_eq!(
            notification.payload().model_definition_id,
            fact.payload().model_definition_id
        );
        assert_eq!(notification.fact_id(), fact.fact_id());
        assert_eq!(
            staged.occurred_at.unix_timestamp_nanos(),
            1_700_000_000_000_000_000
        );
    }

    #[tokio::test]
    async fn independent_plugin_commit_delivery_retries_before_marking_delivered() {
        let assembly = independent_plugin_fixture_assembly();
        let graph = assembly.compile_graph().unwrap();
        let plan = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let subscriber = &plan.subscribers()[0];
        assert_eq!(
            subscriber.contributor_module_id,
            "acme.lifecycle-subscriber-host"
        );
        assert_eq!(
            subscriber.contributor_module_kind,
            plugin_framework::extension_bus::ModuleKind::TrustedHost
        );

        let plugin_handler = Arc::new(IndependentPluginHandler {
            fail: AtomicBool::new(true),
            received: Mutex::new(Vec::new()),
        });
        let activated = Arc::new(AtomicBool::new(false));
        let publication = Arc::new(
            super::super::notification_publication::HostOptionalNotificationPublication::default(),
        );
        let mut factories = production_lifecycle_handler_factories(publication.clone()).unwrap();
        let factory_handler = Arc::clone(&plugin_handler);
        let factory_activated = Arc::clone(&activated);
        factories
            .register(
                "builtin://acme.lifecycle-subscriber-host",
                "acme_lifecycle_subscriber_fixture",
                Arc::new(move || {
                    factory_activated.store(true, Ordering::SeqCst);
                    Ok(vec![LifecycleHandlerBinding::typed::<
                        ModelDefinitionCommittedFact,
                        _,
                    >(
                        "acme.lifecycle-subscriber.model-definition-committed",
                        "v1",
                        Arc::clone(&factory_handler),
                    )])
                }),
            )
            .unwrap();
        let bindings = factories
            .activate(assembly.host_extension_manifests())
            .unwrap();
        assert!(activated.load(Ordering::SeqCst));
        let (delivery, publication_catalog) =
            ApiLifecycleFactDelivery::bind(&plan, bindings).unwrap();

        let base_database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".to_string()
        });
        let database = postgres_test_support::PostgresTestSchema::create(&base_database_url)
            .await
            .unwrap();
        let pool = database.connect().await.unwrap();
        storage_durable_postgres::run_migrations(&pool)
            .await
            .unwrap();
        let store = storage_durable_postgres::PgControlPlaneStore::new(pool)
            .with_lifecycle_publication_catalog(publication_catalog);
        publication.activate(Arc::new(store.clone())).unwrap();
        let tenant = store.upsert_root_tenant().await.unwrap();
        let workspace = store
            .upsert_workspace(
                tenant.id,
                &format!("Lifecycle fixture {}", Uuid::now_v7().simple()),
            )
            .await
            .unwrap();
        let model = ModelDefinitionRepository::create_model_definition(
            &store,
            &CreateModelDefinitionInput {
                bound_storage_id: None,
                actor_user_id: Uuid::nil(),
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: workspace.id,
                data_source_instance_id: None,
                source_kind: domain::DataModelSourceKind::MainSource,
                external_resource_key: None,
                external_table_id: None,
                external_capability_snapshot: None,
                template_provider: "acme-fixture".to_string(),
                template_code: "independent-lifecycle".to_string(),
                template_version: "v1".to_string(),
                code: format!("lifecycle_fixture_{}", Uuid::now_v7().simple()),
                title: "Independent lifecycle fixture".to_string(),
                description: None,
                status: domain::DataModelStatus::Published,
                protection: domain::DataModelProtection::default(),
            },
        )
        .await
        .unwrap();
        let event_id: Uuid = sqlx::query_scalar(
            "select event_id from lifecycle_outbox where contract_id = 'model_definition.committed'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();

        let dispatcher = control_plane::lifecycle_outbox_dispatcher::LifecycleOutboxDispatcher::new(
            store.clone(),
            Arc::new(delivery),
            Arc::new(crate::ApiLifecycleDeliveryCompletion::default()),
        );
        assert_eq!(dispatcher.run_once().await.unwrap(), 2);
        assert_eq!(
            lifecycle_status(store.pool(), event_id, &subscriber.subscriber_id).await,
            ("pending", "pending")
        );
        assert!(plugin_handler
            .received
            .lock()
            .expect("received lifecycle facts mutex poisoned")
            .is_empty());

        plugin_handler.fail.store(false, Ordering::SeqCst);
        sqlx::query(
            "update lifecycle_outbox_deliveries set available_at = now() where event_id = $1",
        )
        .bind(event_id)
        .execute(store.pool())
        .await
        .unwrap();
        assert_eq!(dispatcher.run_once().await.unwrap(), 1);
        assert_eq!(
            lifecycle_status(store.pool(), event_id, &subscriber.subscriber_id).await,
            ("delivered", "delivered")
        );
        assert_eq!(
            plugin_handler
                .received
                .lock()
                .expect("received lifecycle facts mutex poisoned")
                .as_slice(),
            [model.id]
        );
    }

    async fn lifecycle_status(
        pool: &sqlx::PgPool,
        event_id: Uuid,
        subscriber_id: &str,
    ) -> (&'static str, &'static str) {
        let parent: String =
            sqlx::query_scalar("select status from lifecycle_outbox where event_id = $1")
                .bind(event_id)
                .fetch_one(pool)
                .await
                .unwrap();
        let subscriber: String = sqlx::query_scalar(
            "select status from lifecycle_outbox_deliveries where event_id = $1 and subscriber_id = $2",
        )
        .bind(event_id)
        .bind(subscriber_id)
        .fetch_one(pool)
        .await
        .unwrap();
        (
            match parent.as_str() {
                "pending" => "pending",
                "delivered" => "delivered",
                other => panic!("unexpected parent status {other}"),
            },
            match subscriber.as_str() {
                "pending" => "pending",
                "delivered" => "delivered",
                other => panic!("unexpected subscriber status {other}"),
            },
        )
    }

    fn independent_plugin_fixture_assembly() -> crate::extension_bus::ExtensionGraphInputAssembly {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = plugin_framework::parse_plugin_manifest(include_str!(
            "../../../../plugins/fixtures/acme.lifecycle-subscriber-host/manifest.yaml"
        ))
        .unwrap();
        let contribution =
            plugin_framework::parse_host_extension_contribution_manifest(include_str!(
                "../../../../plugins/fixtures/acme.lifecycle-subscriber-host/host-extension.yaml"
            ))
            .unwrap();
        let mut assembly = crate::extension_bus::assemble_extension_graph_input(
            &root,
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            Vec::new(),
        )
        .unwrap();
        assembly
            .extend_active_host_extensions(&[(manifest, contribution)])
            .unwrap();
        assembly
    }
    #[test]
    fn native_definition_projection_has_no_unbound_phases_or_system_credit_source() {
        use domain::event_center::*;
        use extension_contracts::LifecycleContract;
        let assembly = independent_plugin_fixture_assembly();
        let graph = assembly.compile_graph().unwrap();
        let plan = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let declarations = native_lifecycle_source_descriptors(&plan, EventSourceScope::Unbound);
        assert_eq!(declarations.len(), 3);
        assert!(declarations
            .iter()
            .all(|d| d.binding == EventSourceBinding::Declaration
                && d.coverage == EventSourceCoverage::Unavailable
                && d.supported_phases.is_empty()));
        let system = native_lifecycle_source_descriptors(&plan, EventSourceScope::System);
        assert_eq!(system.len(), 2);
        assert!(system.iter().all(|d| d.contract_id
            != control_plane_contracts::ports::CreditNotificationFact::CONTRACT_ID));
    }
    #[test]
    fn credit_catalog_leaves_select_the_same_original_source_without_extra_publication() {
        use control_plane_contracts::ports::{CreditNotificationEventType, CreditNotificationFact};
        use domain::event_center::*;
        use extension_contracts::LifecycleContract;
        let assembly = independent_plugin_fixture_assembly();
        let graph = assembly.compile_graph().unwrap();
        let plan = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let scope = EventSourceScope::Workspace {
            workspace_id: uuid::Uuid::now_v7(),
        };
        let entries = native_lifecycle_catalog_entries(&plan, scope);
        let credit = entries
            .iter()
            .filter(|e| e.contract_id == CreditNotificationFact::CONTRACT_ID)
            .collect::<Vec<_>>();
        assert_eq!(credit.len(), 10);
        let parent = credit
            .iter()
            .find(|e| e.source.as_ref().unwrap().selection.filters.is_empty())
            .unwrap();
        for kind in CreditNotificationEventType::ALL {
            let leaf = credit.iter().find(|e| e.name == kind.wire_name()).unwrap();
            let selection = &leaf.source.as_ref().unwrap().selection;
            assert_eq!(
                selection.scope,
                parent.source.as_ref().unwrap().selection.scope
            );
            assert_eq!(
                selection.source_id,
                parent.source.as_ref().unwrap().selection.source_id
            );
            assert_eq!(selection.filters.len(), 1);
            assert_eq!(selection.filters["/payload/event_type"], kind.wire_name());
            assert_eq!(
                leaf.payload_schema["properties"]["payload"]["properties"]["event_type"]["const"],
                kind.wire_name()
            );
            assert_eq!(leaf.supported_phases, parent.supported_phases);
        }
        assert!(
            native_lifecycle_catalog_entries(&plan, EventSourceScope::System)
                .iter()
                .all(|e| e.contract_id != CreditNotificationFact::CONTRACT_ID)
        );
    }
}

/// Pure typed definition projection. It does not create a lease or a subscriber queue.
pub(crate) fn native_lifecycle_source_descriptors(
    _plan: &EffectiveLifecycleSubscriberPlan,
    scope: domain::event_center::EventSourceScope,
) -> Vec<domain::event_center::EventSourceDescriptor> {
    use control_plane::lifecycle_outbox_dispatcher::lifecycle_source_id;
    use control_plane_contracts::ports::{CreditNotificationFact, ModelDefinitionCommittedFact};
    use domain::event_center::*;
    use extension_contracts::LifecycleContract;
    [
        (ModelDefinitionCommittedFact::CONTRACT_ID, ModelDefinitionCommittedFact::event_source_schema(), extension_contracts::MANAGED_CREATE_EVENT_POINT),
        (CreditNotificationFact::CONTRACT_ID, CreditNotificationFact::event_source_schema(), "1flowbase.credit.integration-notification"),
        (ModelDefinitionIntegrationNotification::CONTRACT_ID, ModelDefinitionIntegrationNotification::event_source_schema(), "1flowbase.model-definition.integration-notification"),
    ].into_iter().filter(|(contract, _, _)| scope != EventSourceScope::System || *contract != CreditNotificationFact::CONTRACT_ID)
    .map(|(contract, schema, point)| EventSourceDescriptor {
        source_id: lifecycle_source_id(&scope, contract), owner_id: "control-plane.lifecycle".into(),
        point_id: point.into(), contract_id: contract.into(),
        name: contract.into(),
        description: "Native committed lifecycle fact. Scoped queue counts cover trusted metadata only; historical unbound deliveries are listed separately and cannot be assigned to a workspace.".into(),
        payload_schema: schema, origin: EventDefinitionOrigin::Core, scope: scope.clone(),
        binding: if scope == EventSourceScope::Unbound { EventSourceBinding::Declaration } else { EventSourceBinding::Active },
        coverage: if scope == EventSourceScope::Unbound { EventSourceCoverage::Unavailable } else { EventSourceCoverage::Live },
        supported_phases: if scope == EventSourceScope::Unbound { Vec::new() } else { vec![EventTriggerPhase::Published, EventTriggerPhase::Consumed] },
    }).collect()
}

/// Install native templates once at boot; never instantiate every possible workspace/queue.
/// The scope argument filters the definition family only. Actual leases are activated from
/// trusted publication/consumption context or from existing scoped durable queue rows.
pub(crate) fn register_native_lifecycle_sources(
    observer: &control_plane::lifecycle_outbox_dispatcher::LifecycleSourceObserver,
    plan: &EffectiveLifecycleSubscriberPlan,
    scope: domain::event_center::EventSourceScope,
) {
    use control_plane::lifecycle_outbox_dispatcher::lifecycle_source_id;
    use domain::event_center::*;
    for mut descriptor in native_lifecycle_source_descriptors(plan, scope) {
        descriptor.scope = EventSourceScope::Unbound;
        descriptor.source_id = lifecycle_source_id(&descriptor.scope, &descriptor.contract_id);
        descriptor.binding = EventSourceBinding::Declaration;
        descriptor.coverage = EventSourceCoverage::Unavailable;
        descriptor.supported_phases.clear();
        use control_plane_contracts::ports::{
            CreditNotificationFact, ModelDefinitionCommittedFact,
        };
        use extension_contracts::LifecycleContract;
        let version = match descriptor.contract_id.as_str() {
            ModelDefinitionCommittedFact::CONTRACT_ID => {
                ModelDefinitionCommittedFact::CONTRACT_VERSION
            }
            CreditNotificationFact::CONTRACT_ID => CreditNotificationFact::CONTRACT_VERSION,
            ModelDefinitionIntegrationNotification::CONTRACT_ID => {
                ModelDefinitionIntegrationNotification::CONTRACT_VERSION
            }
            _ => continue,
        };
        let contract = descriptor.contract_id.clone();
        observer.register(descriptor, (&contract, version), false, &[]);
    }
}

/// Logical subtype leaves select one existing fact; they do not create publications or leases.
pub(crate) fn native_lifecycle_catalog_entries(
    plan: &EffectiveLifecycleSubscriberPlan,
    scope: domain::event_center::EventSourceScope,
) -> Vec<domain::event_center::EventCatalogEntry> {
    use control_plane_contracts::ports::{CreditNotificationEventType, CreditNotificationFact};
    use domain::event_center::EventCatalogEntry;
    use extension_contracts::LifecycleContract;
    let mut entries = Vec::new();
    for descriptor in native_lifecycle_source_descriptors(plan, scope) {
        let entry: EventCatalogEntry = descriptor.into();
        if entry.contract_id == CreditNotificationFact::CONTRACT_ID {
            let mut parent = entry.clone();
            parent.group_path = vec!["credits".into()];
            entries.push(parent);
            for kind in CreditNotificationEventType::ALL {
                let mut leaf = entry.clone();
                leaf.group_path =
                    vec!["credits".into(), CreditNotificationFact::CONTRACT_ID.into()];
                leaf.name = kind.wire_name().into();
                leaf.source
                    .as_mut()
                    .expect("source projection")
                    .selection
                    .filters
                    .insert("/payload/event_type".into(), kind.wire_name().into());
                leaf.payload_schema["properties"]["payload"]["properties"]["event_type"] =
                    serde_json::json!({"type":"string","const":kind.wire_name()});
                entries.push(leaf);
            }
        } else {
            entries.push(entry);
        }
    }
    entries
}
