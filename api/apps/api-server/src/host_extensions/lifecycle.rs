#[cfg(test)]
use anyhow::Result;
pub use api_host_kernel::host_extensions::lifecycle::*;
#[cfg(test)]
use async_trait::async_trait;
#[cfg(test)]
use control_plane_contracts::ports::{LifecycleDeliveryBlocked, LifecycleDeliveryPauseReason};
#[cfg(test)]
use control_plane_contracts::ports::{LifecycleOutboxRecord, LifecycleSubscriberTarget};
#[cfg(test)]
use control_plane_contracts::ports::{
    ModelDefinitionIntegrationNotification, OptionalNotificationContract,
    OptionalNotificationPublicationPort, PublishOptionalNotificationInput,
};
#[cfg(test)]
use plugin_framework::extension_bus::{
    compile_lifecycle_handler_registry, EffectiveLifecycleHandlerRegistry,
    EffectiveLifecycleSubscriber, EffectiveLifecycleSubscriberPlan, LifecycleHandlerBinding,
    LifecycleHandlerError, LifecycleHandlerFuture, TypedLifecycleSubscriberHandler,
};
#[cfg(test)]
use std::{collections::BTreeMap, sync::Arc};
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
