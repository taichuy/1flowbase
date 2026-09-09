//! Root #2014 AC005–007: formal packages, actual SDK processes and transactional host effects.
use super::managed_event_authority_tests::package;
use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices};
use control_plane::{
    lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort, plugin_management::*,
    ports::AuthRepository,
};
use control_plane_contracts::ports::*;
use extension_contracts::*;
use std::sync::Arc;
use uuid::Uuid;
const SHIPMENT: &str = "orion.shipments.created";
const AUDIT: &str = "lyra.audit-log.recorded";
fn manifest(owner: &str) -> serde_json::Value {
    serde_yaml::from_str(match owner {
        "orion.shipments" => {
            include_str!("../../../../../plugins/fixtures/orion.shipments/manifest.yaml")
        }
        "lyra.audit-log" => {
            include_str!("../../../../../plugins/fixtures/lyra.audit-log/manifest.yaml")
        }
        _ => panic!("finite fixture"),
    })
    .unwrap()
}
struct Fixture {
    _state: Arc<crate::app_state::ApiState>,
    _services: Arc<ApiRuntimeServices>,
    store: storage_durable_postgres::MainDurableStore,
    delivery: crate::host_extensions::lifecycle::ApiLifecycleFactDelivery,
    actor: domain::ActorContext,
    installations: Vec<Uuid>,
}
impl Fixture {
    async fn new() -> Self {
        let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        let actor = state
            .store
            .load_actor_context_for_user(actor_id)
            .await
            .unwrap();
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            vec![],
        )
        .unwrap();
        let graph = Arc::new(assembly.compile_graph().unwrap());
        let native_plan = assembly.compile_lifecycle_subscriber_plan(&graph).unwrap();
        let native_bindings =
            crate::host_extensions::lifecycle::production_lifecycle_handler_factories(Arc::new(
                storage_ephemeral::MemoryEventBus::new(),
            ))
            .unwrap()
            .activate(assembly.host_extension_manifests())
            .unwrap();
        let (delivery, catalog) =
            crate::host_extensions::lifecycle::ApiLifecycleFactDelivery::bind(
                &native_plan,
                native_bindings,
            )
            .unwrap();
        let store = state
            .store
            .clone()
            .with_lifecycle_publication_catalog(catalog.clone());
        let resolver = Arc::new(ApiRuntimeArtifactResolver::new(
            store.clone(),
            &state.api_node_id,
            &state.provider_install_root,
        ));
        let host = Arc::new(
            runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
                time::OffsetDateTime::now_utc(),
                resolver,
            )
            .unwrap(),
        );
        host.mark_ready().unwrap();
        let services = Arc::new(
            ApiRuntimeServices::new_with_runtime_backend(host, graph)
                .unwrap()
                .with_managed_composition(
                    store.clone(),
                    state.api_node_id.clone(),
                    assembly.module_descriptors().to_vec(),
                ),
        );
        let composition = services.managed_composition().unwrap();
        composition
            .attach_native_lifecycle_plan(native_plan.clone())
            .unwrap();
        let source = Arc::new(crate::extension_bus::ManagedWorkspacePublicationSource(
            Arc::downgrade(&composition),
        ));
        catalog.attach_workspace_source(source.clone()).unwrap();
        let delivery = delivery.with_managed(&composition);
        let management = PluginManagementService::new(
            store.clone(),
            ApiProviderRuntime::new(services.clone()),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id);
        let authority = PluginContributionAuthorityService::new(
            store.clone(),
            HostContributionGrantPolicy::root_composition(),
        );

        let schema_dependencies =
            crate::routes::plugins::extension_center::ExtensionCenterDependencies {
                store: store.clone(),
                provider_runtime: services.clone(),
                official_plugin_source: state.official_plugin_source.clone(),
                official_mcp_bundle_source: state.official_mcp_bundle_source.clone(),
                official_extension_catalog_source: state.official_extension_catalog_source.clone(),
                cache_store: state.infrastructure.cache_store(),
                provider_install_root: state.provider_install_root.clone(),
                api_node_id: state.api_node_id.clone(),
                allow_uploaded_host_extensions: state.allow_uploaded_host_extensions,
            };

        let mut installations = Vec::new();
        for owner in ["orion.shipments", "lyra.audit-log"] {
            let raw = manifest(owner);
            let installed = management
                .install_uploaded_plugin(InstallUploadedPluginCommand {
                    actor_user_id: actor_id,
                    file_name: format!("{owner}.1flowbasepkg"),
                    package_bytes: package(&raw),
                })
                .await
                .unwrap();
            let id = installed.installation.id;
            management
                .assign_plugin(AssignPluginCommand {
                    actor_user_id: actor_id,
                    installation_id: id,
                })
                .await
                .unwrap();
            assert!(
                management
                    .enable_plugin(EnablePluginCommand {
                        actor_user_id: actor_id,
                        installation_id: id
                    })
                    .await
                    .is_err(),
                "ungranted activation"
            );
            let declared =
                plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&raw).unwrap())
                    .unwrap();
            if !declared.data_models.is_empty() {
                crate::routes::plugins::extension_center::apply_fixture_managed_schema(
                    &schema_dependencies,
                    actor.current_workspace_id,
                    &declared,
                )
                .await
                .unwrap();
            }
            for contribution in &declared.managed.as_ref().unwrap().module.contributions {
                for permission in &contribution.required_permissions {
                    let write = permission.as_str() == "plugin_data.owned.write";
                    authority
                        .grant(
                            &actor,
                            id,
                            GrantContributionPermission {
                                contribution_id: contribution.contribution_id.as_str().into(),
                                permission: permission.as_str().into(),
                                resource_scope: if write {
                                    domain::ContributionResourceScope::OwnedCollection {
                                        collection_code: "audit_records".into(),
                                    }
                                } else {
                                    domain::ContributionResourceScope::Workspace
                                },
                                permission_contract_id: if write {
                                    "plugin-data"
                                } else {
                                    "managed-event"
                                }
                                .into(),
                                permission_contract_version: "1".into(),
                            },
                        )
                        .await
                        .unwrap();
                }
            }
            management
                .enable_plugin(EnablePluginCommand {
                    actor_user_id: actor_id,
                    installation_id: id,
                })
                .await
                .unwrap();
            installations.push(id);
        }
        Self {
            _state: state,
            _services: services,
            store,
            delivery,
            actor,
            installations,
        }
    }
    async fn create(&self) -> Uuid {
        let model =
            control_plane_contracts::ports::ModelDefinitionRepository::create_model_definition(
                &self.store,
                &control_plane_contracts::ports::CreateModelDefinitionInput {
                    actor_user_id: self.actor.user_id,
                    scope_kind: domain::DataModelScopeKind::Workspace,
                    scope_id: self.actor.current_workspace_id,
                    data_source_instance_id: None,
                    source_kind: domain::DataModelSourceKind::MainSource,
                    external_resource_key: None,
                    external_table_id: None,
                    external_capability_snapshot: None,
                    template_provider: "generic-fixture".into(),
                    template_code: "event".into(),
                    template_version: "1".into(),
                    code: format!("event_{}", Uuid::now_v7().simple()),
                    title: "Managed event".into(),
                    description: None,
                    status: domain::DataModelStatus::Published,
                    protection: domain::DataModelProtection::default(),
                },
            )
            .await
            .unwrap();
        model.id
    }
    async fn claim(&self, worker: Uuid) -> Vec<LifecycleOutboxRecord> {
        self.store
            .claim_lifecycle_facts(worker, 64, time::Duration::seconds(120))
            .await
            .unwrap()
    }
    async fn ack(&self, worker: Uuid, record: &LifecycleOutboxRecord) {
        self.store
            .mark_lifecycle_fact_delivered(
                record.event_id,
                &record.subscriber_id,
                worker,
                record.claim_id.unwrap(),
            )
            .await
            .unwrap();
    }
    async fn advance(&self, worker: Uuid) -> Vec<LifecycleOutboxRecord> {
        let records = self.claim(worker).await;
        for record in &records {
            self.delivery.deliver(record).await.unwrap();
            self.ack(worker, record).await;
        }
        records
    }
    async fn owned_count(&self) -> i64 {
        let table:String=sqlx::query_scalar("select physical_table from plugin_schema_ownership where owner_id='lyra/lyra.audit-log' and object_kind='owned_collection' and logical_name='audit_records'").fetch_one(self.store.pool()).await.unwrap();
        assert!(table
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        sqlx::query_scalar(&format!("select count(*) from {table}"))
            .fetch_one(self.store.pool())
            .await
            .unwrap()
    }
}
#[tokio::test]
async fn root_2014_ac_005_installed_generic_event_exchange() {
    let fixture = Fixture::new().await;
    let model = fixture.create().await;
    let worker = Uuid::now_v7();
    fixture.advance(worker).await;
    let shipments = fixture.advance(worker).await;
    assert_eq!(shipments.len(), 1);
    assert_eq!(shipments[0].contract_id, SHIPMENT);
    let shipment: ManagedEventFact =
        serde_json::from_slice(&shipments[0].canonical_payload).unwrap();
    assert_eq!(
        shipment.payload,
        serde_json::json!({"shipment_id":model.to_string(),"destination_code":"NYC","item_count":3})
    );
    let audits = fixture.advance(worker).await;
    assert_eq!(audits.len(), 2);
    let audit: ManagedEventFact = serde_json::from_slice(&audits[0].canonical_payload).unwrap();
    assert_eq!(audit.contract_id, AUDIT);
    assert_eq!(
        audit.payload,
        serde_json::json!({"audit_ref":model.to_string(),"accepted":true,"route_code":"NYC"})
    );
    assert_eq!(audit.causation_id, shipments[0].event_id.to_string());
    assert_eq!(fixture.owned_count().await, 1);
}
#[tokio::test]
async fn root_2014_ac_006_event_authority_schema_negatives() {
    let mut foreign = manifest("orion.shipments");
    foreign["managed"]["module"]["extension_points"][0]["point_id"] =
        "foreign.shipments.created".into();
    assert!(
        plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&foreign).unwrap()).is_err()
    );
    let fixture = Fixture::new().await;
    fixture.create().await;
    let worker = Uuid::now_v7();
    fixture.advance(worker).await;
    let records = fixture.claim(worker).await;
    let record = &records[0];
    for (field, value) in [
        (
            "workspace_id",
            serde_json::json!(Uuid::now_v7().to_string()),
        ),
        ("contract_version", serde_json::json!("wrong")),
    ] {
        let mut forged = record.clone();
        let mut fact: serde_json::Value =
            serde_json::from_slice(&forged.canonical_payload).unwrap();
        fact[field] = value;
        forged.canonical_payload = serde_json::to_vec(&fact).unwrap();
        assert!(fixture.delivery.deliver(&forged).await.is_err());
    }
    let mut forged = record.clone();
    let mut fact: serde_json::Value = serde_json::from_slice(&forged.canonical_payload).unwrap();
    fact["payload"]["actor_id"] = "forged".into();
    forged.canonical_payload = serde_json::to_vec(&fact).unwrap();
    assert!(fixture.delivery.deliver(&forged).await.is_err());
    let authority = PluginContributionAuthorityService::new(
        fixture.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let grants = authority
        .query(&fixture.actor, fixture.installations[1])
        .await
        .unwrap();
    let publish = grants
        .authorizations
        .iter()
        .find(|g| g.contribution_id == "lyra.audit-log.publish" && g.permission == "event.publish")
        .unwrap();
    authority
        .revoke(
            &fixture.actor,
            fixture.installations[1],
            RevokeContributionPermission {
                authorization_id: publish.id,
                expected_revision: grants.revision,
            },
        )
        .await
        .unwrap();
    assert!(fixture.delivery.deliver(record).await.is_err());
    assert_eq!(fixture.owned_count().await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from lifecycle_outbox where contract_id=$1")
            .bind(AUDIT)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn root_2014_ac_007_transaction_retry_claim_fencing() {
    let fixture = Fixture::new().await;
    fixture.create().await;
    let worker = Uuid::now_v7();
    fixture.advance(worker).await;
    let shipments = fixture.claim(worker).await;
    let shipment = &shipments[0];
    sqlx::query("create function reject_generic_fact() returns trigger language plpgsql as $$ begin if new.contract_id='lyra.audit-log.recorded' then raise exception 'fixture transaction abort'; end if; return new; end $$").execute(fixture.store.pool()).await.unwrap();
    sqlx::query("create trigger reject_generic_fact before insert on lifecycle_outbox for each row execute function reject_generic_fact()").execute(fixture.store.pool()).await.unwrap();
    assert!(fixture.delivery.deliver(shipment).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from lifecycle_outbox where contract_id=$1")
            .bind(AUDIT)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap(),
        0
    );
    sqlx::query("drop trigger reject_generic_fact on lifecycle_outbox")
        .execute(fixture.store.pool())
        .await
        .unwrap();
    fixture.delivery.deliver(shipment).await.unwrap();
    fixture.delivery.deliver(shipment).await.unwrap();
    fixture.ack(worker, shipment).await;
    let audits = fixture.claim(worker).await;
    assert_eq!(audits.len(), 2);
    let effect = audits
        .iter()
        .find(|r| r.subscriber_id.ends_with("lyra.audit-log.store"))
        .unwrap();
    let observer = audits
        .iter()
        .find(|r| r.subscriber_id.ends_with("lyra.audit-log.observe"))
        .unwrap();
    fixture.delivery.deliver(observer).await.unwrap();
    fixture.ack(worker, observer).await;
    let (a, b) = tokio::join!(
        fixture.delivery.deliver(effect),
        fixture.delivery.deliver(effect)
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(fixture.owned_count().await, 1);
    fixture
        .store
        .retry_lifecycle_fact(
            effect.event_id,
            &effect.subscriber_id,
            worker,
            effect.claim_id.unwrap(),
            time::OffsetDateTime::now_utc() - time::Duration::seconds(1),
            "lost ACK",
        )
        .await
        .unwrap();
    let retry = fixture.claim(worker).await;
    assert_eq!(retry.len(), 1);
    assert_ne!(retry[0].claim_id, effect.claim_id);
    assert!(fixture
        .store
        .mark_lifecycle_fact_delivered(
            effect.event_id,
            &effect.subscriber_id,
            worker,
            effect.claim_id.unwrap()
        )
        .await
        .is_err());
    fixture.delivery.deliver(&retry[0]).await.unwrap();
    fixture.ack(worker, &retry[0]).await;
    assert_eq!(fixture.owned_count().await, 1);
}
