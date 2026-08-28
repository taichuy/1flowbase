use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use control_plane::ports::EventBus;
use control_plane_contracts::ports::{LifecycleOutboxRecord, LifecycleSubscriberTarget};
use plugin_framework::extension_bus::{
    compile_lifecycle_handler_registry, EffectiveLifecycleHandlerRegistry,
    EffectiveLifecycleSubscriberPlan, LifecycleHandlerBinding, LifecycleHandlerError,
    LifecycleHandlerFuture, TypedLifecycleSubscriberHandler,
};

pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_ID: &str =
    "official.plugin-host.model-definition-committed";
pub(crate) const MODEL_DEFINITION_COMMITTED_HANDLER_VERSION: &str = "v1";
const MODEL_DEFINITION_COMMITTED_TOPIC: &str =
    "official.plugin-host.lifecycle.model-definition-committed";

struct ModelDefinitionCommittedHandler {
    event_bus: Arc<dyn EventBus>,
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
            self.event_bus
                .publish(
                    MODEL_DEFINITION_COMMITTED_TOPIC,
                    serde_json::to_value(fact).map_err(|error| {
                        LifecycleHandlerError::new(format!(
                            "failed to encode plugin lifecycle fact: {error}"
                        ))
                    })?,
                )
                .await
                .map_err(|error| LifecycleHandlerError::new(error.to_string()))
        })
    }
}

pub(crate) fn builtin_lifecycle_handler_bindings(
    event_bus: Arc<dyn EventBus>,
) -> Vec<LifecycleHandlerBinding> {
    vec![LifecycleHandlerBinding::typed::<
        control_plane_contracts::ports::ModelDefinitionCommittedFact,
        _,
    >(
        MODEL_DEFINITION_COMMITTED_HANDLER_ID,
        MODEL_DEFINITION_COMMITTED_HANDLER_VERSION,
        Arc::new(ModelDefinitionCommittedHandler { event_bus }),
    )]
}

pub(crate) struct ApiLifecycleFactDelivery {
    registry: EffectiveLifecycleHandlerRegistry,
}

impl ApiLifecycleFactDelivery {
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
        let registry = compile_lifecycle_handler_registry(plan, handler_bindings)?;
        Ok((Self { registry }, catalog))
    }
}

#[async_trait]
impl control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort
    for ApiLifecycleFactDelivery
{
    async fn deliver(&self, fact: &LifecycleOutboxRecord) -> Result<()> {
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

    #[tokio::test]
    async fn real_host_extension_subscriber_receives_typed_after_commit_fact() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            &root,
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            Vec::new(),
        )
        .unwrap();
        let graph = assembly.compile_graph().unwrap();
        let plan = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let event_bus = Arc::new(storage_ephemeral::MemoryEventBus::new());
        let (delivery, _) = ApiLifecycleFactDelivery::bind(
            &plan,
            builtin_lifecycle_handler_bindings(event_bus.clone() as Arc<dyn EventBus>),
        )
        .unwrap();
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
            delivered_at: None,
        };
        delivery.deliver(&record).await.unwrap();
        assert!(event_bus
            .poll(MODEL_DEFINITION_COMMITTED_TOPIC)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn independent_plugin_commit_delivery_retries_before_marking_delivered() {
        let (assembly, fixture_root) = independent_plugin_fixture_assembly();
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
        let (delivery, publication_catalog) = ApiLifecycleFactDelivery::bind(
            &plan,
            vec![LifecycleHandlerBinding::typed::<
                ModelDefinitionCommittedFact,
                _,
            >(
                "acme.lifecycle-subscriber.model-definition-committed",
                "v1",
                Arc::clone(&plugin_handler),
            )],
        )
        .unwrap();

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
            Arc::new(crate::ApiLifecycleDeliveryCompletion),
        );
        assert_eq!(dispatcher.run_once().await.unwrap(), 1);
        assert_eq!(
            lifecycle_status(store.pool(), event_id).await,
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
            lifecycle_status(store.pool(), event_id).await,
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

        std::fs::remove_dir_all(fixture_root).unwrap();
    }

    async fn lifecycle_status(pool: &sqlx::PgPool, event_id: Uuid) -> (&'static str, &'static str) {
        let parent: String =
            sqlx::query_scalar("select status from lifecycle_outbox where event_id = $1")
                .bind(event_id)
                .fetch_one(pool)
                .await
                .unwrap();
        let subscriber: String = sqlx::query_scalar(
            "select status from lifecycle_outbox_deliveries where event_id = $1",
        )
        .bind(event_id)
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

    fn independent_plugin_fixture_assembly() -> (
        crate::extension_bus::ExtensionGraphInputAssembly,
        std::path::PathBuf,
    ) {
        let root = std::env::temp_dir().join(format!(
            "1flowbase-lifecycle-plugin-fixture-{}",
            Uuid::now_v7().simple()
        ));
        let plugin = root.join("plugins/host-extensions/acme.lifecycle-subscriber-host");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::create_dir_all(root.join("plugins/sets")).unwrap();
        std::fs::write(
            plugin.join("manifest.yaml"),
            include_str!(
                "../../../../plugins/fixtures/acme.lifecycle-subscriber-host/manifest.yaml"
            ),
        )
        .unwrap();
        std::fs::write(
            plugin.join("host-extension.yaml"),
            include_str!(
                "../../../../plugins/fixtures/acme.lifecycle-subscriber-host/host-extension.yaml"
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("plugins/sets/fixture.yaml"),
            r#"schema_version: 1flowbase.plugin-set/v1
set_id: fixture
host_extensions:
  - acme.lifecycle-subscriber-host
runtime_extensions: []
capability_plugins: []
"#,
        )
        .unwrap();
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            &root,
            "plugins/sets/fixture.yaml",
            Vec::new(),
        )
        .unwrap();
        (assembly, root)
    }
}
