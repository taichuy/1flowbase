//! Root #2007 AC-005: installed SDK workers, static/activation/execution authority boundaries.
//! AC-006/007 transaction fault injection remains in P06/P07; this establishes the production path.
use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices};
use control_plane::{plugin_management::*, ports::AuthRepository};
use control_plane_contracts::ports::{
    LifecycleOutboxRepository, WorkspaceLifecyclePublicationSource,
};
use extension_contracts::*;
use plugin_framework::extension_bus::*;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

pub(super) fn manifest(name: &str) -> plugin_framework::PluginManifestV1 {
    let base = plugin_framework::parse_plugin_manifest(include_str!(
        "../../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
    ))
    .unwrap();
    let mut value = serde_json::to_value(base).unwrap();
    let module = format!("acme.composition-{name}");
    let contribution = format!("{module}.events");
    value["plugin_id"] = module.clone().into();
    value["data_models"] = json!([]);
    value["managed"] = json!({
        "module":{"bus_version":"v1","module_id":module,"module_version":"1.0.0","module_kind":"runtime","contributions":[{
            "contribution_id":contribution,"contributor_module_id":module,"point_id":if name=="a" {MANAGED_CREATE_EVENT_POINT} else {MANAGED_PROCESSED_EVENT_ID},"contract_version":"1",
            "required_permissions":if name=="a" {vec!["event.subscribe","event.publish"]} else {vec!["event.subscribe"]},"mode":"append"
        }]},
        "execution_bindings":[{"contribution_id":contribution,"execution_mode":"process_per_call","runtime":{"protocol":"stdio_json","entry":"bin/worker.py"},"handler":if name=="a" {"publish"} else {"ack"}}]
    });
    if name == "a" {
        value["managed"]["module"]["extension_points"] = json!([{
            "point_id":MANAGED_PROCESSED_EVENT_ID,"owner_module_id":module,"point_kind":"event_stream","contract":{"contract_id":MANAGED_PROCESSED_EVENT_ID,"contract_version":"1"},
            "scope":"workspace","cardinality":"many","ordering":"lexicographic","failure":"isolate_contribution","delivery":"after_commit_durable","lifecycle":"workspace_assignment","allowed_permissions":["event.subscribe","event.publish"],"override_policy":"sealed"
        }]);
    }
    serde_json::from_value(value).unwrap()
}
pub(super) fn package(manifest: &plugin_framework::PluginManifestV1) -> Vec<u8> {
    let bytes = serde_yaml::to_string(manifest).unwrap().into_bytes();
    // The exact author artifact must pass the official parser before archive intake.
    plugin_framework::parse_plugin_manifest(std::str::from_utf8(&bytes).unwrap()).unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", bytes.as_slice())
        .unwrap();
    let source = PathBuf::from(
        std::env::var_os("MANAGED_EVENT_WORKER_FIXTURE")
            .expect("prebuild runtime-extension-sdk --example managed_event_worker"),
    );
    archive
        .append_path_with_name(source, "bin/worker.py")
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}
pub(super) fn grant(name: &str, permission: &str) -> GrantContributionPermission {
    GrantContributionPermission {
        contribution_id: format!("acme.composition-{name}.events"),
        permission: permission.into(),
        resource_scope: domain::ContributionResourceScope::Workspace,
        permission_contract_id: "managed-event".into(),
        permission_contract_version: "1".into(),
    }
}

#[test]
fn root_2007_ac_005_event_authority_static_namespace_and_contract() {
    let a = manifest("a");
    let managed = a.managed.clone().unwrap();
    assert!(
        managed.module.extension_points[0].is_managed_composition_event(&managed.module.module_id)
    );
    let mut wrong = a.clone();
    wrong.managed.as_mut().unwrap().module.extension_points[0].point_id =
        ExtensionPointId::new("acme.other.processed").unwrap();
    assert!(
        plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&wrong).unwrap()).is_err()
    );
    let mut wrong = a.clone();
    wrong.managed.as_mut().unwrap().module.extension_points[0].point_kind =
        ExtensionPointKind::Pipeline;
    assert!(
        plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&wrong).unwrap()).is_err()
    );
    let mut wrong = a;
    wrong.managed.as_mut().unwrap().module.extension_points[0]
        .contract
        .contract_version = ContractVersion::new("2").unwrap();
    assert!(
        plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&wrong).unwrap()).is_err()
    );
    assert!(
        compile_extension_graph(vec![managed.module]).is_err(),
        "namespace declaration alone is not host authority"
    );
}

#[tokio::test]
async fn root_2007_ac_005_event_authority_installed_publisher_and_subscribers() {
    use control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort;
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
    let (delivery, catalog) = crate::host_extensions::lifecycle::ApiLifecycleFactDelivery::bind(
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
    let mut installations = Vec::new();
    for name in ["a", "b", "c"] {
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor_id,
                file_name: format!("acme.composition-{name}.1flowbasepkg"),
                package_bytes: package(&manifest(name)),
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
        assert!(management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor_id,
                installation_id: id
            })
            .await
            .is_err());
        authority
            .grant(&actor, id, grant(name, "event.subscribe"))
            .await
            .unwrap();
        if name == "a" {
            assert!(
                management
                    .enable_plugin(EnablePluginCommand {
                        actor_user_id: actor_id,
                        installation_id: id
                    })
                    .await
                    .is_err(),
                "subscribe alone cannot grant publish"
            );
            authority
                .grant(&actor, id, grant(name, "event.publish"))
                .await
                .unwrap();
        } else {
            assert!(
                authority
                    .grant(&actor, id, grant(name, "event.publish"))
                    .await
                    .is_err(),
                "B/C cannot impersonate publisher A"
            );
        }
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: actor_id,
                installation_id: id,
            })
            .await
            .unwrap();
        installations.push(installed);
    }
    let snapshot = composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    let plan = source
        .plan_for_workspace(actor.current_workspace_id, MANAGED_CREATE_EVENT_ID, "v1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        plan.graph_fingerprint,
        snapshot.graph.fingerprint().as_str()
    );
    assert_eq!(
        plan.subscribers.len(),
        native_plan.subscribers().len() + 1,
        "native subscribers survive managed activation"
    );
    assert!(source
        .plan_for_workspace(Uuid::now_v7(), MANAGED_CREATE_EVENT_ID, "v1")
        .await
        .unwrap()
        .is_none());
    assert!(source
        .plan_for_workspace(actor.current_workspace_id, MANAGED_CREATE_EVENT_ID, "wrong")
        .await
        .unwrap()
        .is_none());
    // Actual Create owner freezes the same plan and writes fact + targets in its existing transaction.
    let model = control_plane_contracts::ports::ModelDefinitionRepository::create_model_definition(
        &store,
        &control_plane_contracts::ports::CreateModelDefinitionInput {
            actor_user_id: actor_id,
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: actor.current_workspace_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            template_provider: "acme-fixture".into(),
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
    let worker = Uuid::now_v7();
    let facts = store
        .claim_lifecycle_facts(worker, 32, time::Duration::seconds(30))
        .await
        .unwrap();
    let committed = facts
        .iter()
        .find(|f| f.subscriber_id.ends_with("acme.composition-a.events"))
        .unwrap();
    assert_eq!(committed.graph_fingerprint, plan.graph_fingerprint);
    let mut wrong = committed.clone();
    wrong.contract_version = "2".into();
    assert!(delivery.deliver(&wrong).await.is_err());
    let mut wrong = committed.clone();
    wrong.handler_id = "managed.forged".into();
    assert!(delivery.deliver(&wrong).await.is_err());
    for fact in &facts {
        delivery.deliver(fact).await.unwrap();
    }
    // A completed its publication but lost ACK: concurrent redelivery cannot create a second E2.
    let (first_replay, second_replay) =
        tokio::join!(delivery.deliver(committed), delivery.deliver(committed));
    first_replay.unwrap();
    second_replay.unwrap();
    let e2_count: i64 = sqlx::query_scalar(
        "select count(*) from lifecycle_outbox where contract_id = 'acme.composition-a.processed'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(e2_count, 1);
    for record in &facts {
        store
            .mark_lifecycle_fact_delivered(
                record.event_id,
                &record.subscriber_id,
                worker,
                record.claim_id.unwrap(),
            )
            .await
            .unwrap();
    }
    let processed = store
        .claim_lifecycle_facts(worker, 32, time::Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(processed.len(), 2);
    assert_ne!(
        processed[0].transaction_id, committed.transaction_id,
        "E2 has its own host transaction"
    );
    let fact: ManagedEventFact = serde_json::from_slice(&processed[0].canonical_payload).unwrap();
    assert_eq!(fact.causation_id, committed.event_id.to_string());
    assert_eq!(fact.correlation_id, committed.event_id.to_string());
    assert_eq!(fact.payload.model_id, model.id.to_string());
    for record in &processed {
        delivery.deliver(record).await.unwrap();
    }
    // The Outbox keeps the first frozen targets/time across replay; changed content fails closed.
    let mut replay = control_plane_contracts::ports::RecordLifecycleFactInput {
        event_id: processed[0].event_id,
        transaction_id: processed[0].transaction_id,
        contract_id: processed[0].contract_id.clone(),
        contract_version: processed[0].contract_version.clone(),
        canonical_payload: processed[0].canonical_payload.clone(),
        occurred_at: time::OffsetDateTime::now_utc(),
        publication: control_plane_contracts::ports::LifecyclePublicationPlan {
            graph_fingerprint: "later-candidate".into(),
            subscribers: vec![],
        },
    };
    let replayed = control_plane_contracts::ports::DerivedLifecyclePublicationRepository::record_derived_lifecycle_fact(&store, &replay).await.unwrap();
    assert_eq!(replayed.graph_fingerprint, processed[0].graph_fingerprint);
    assert_eq!(replayed.occurred_at, processed[0].occurred_at);
    let mut changed = fact.clone();
    changed.payload.result_reference = Some("processed_models/changed".into());
    replay.canonical_payload = serde_json::to_vec(&changed).unwrap();
    assert!(control_plane_contracts::ports::DerivedLifecyclePublicationRepository::record_derived_lifecycle_fact(&store, &replay).await.is_err());

    // Cross-workspace fact spoofing is rejected before runtime admission.
    let mut foreign = processed[0].clone();
    let mut payload = fact.clone();
    payload.workspace_id = Uuid::now_v7().to_string();
    foreign.canonical_payload = serde_json::to_vec(&payload).unwrap();
    assert!(delivery.deliver(&foreign).await.is_err());
    // Revocation is checked against the original frozen subject on every new admission.
    let b = installations[1].installation.id;
    let grants = authority.query(&actor, b).await.unwrap();
    authority
        .revoke(
            &actor,
            b,
            RevokeContributionPermission {
                authorization_id: grants.authorizations[0].id,
                expected_revision: grants.revision,
            },
        )
        .await
        .unwrap();
    let b_record = processed
        .iter()
        .find(|f| f.subscriber_id.ends_with("acme.composition-b.events"))
        .unwrap();
    assert_eq!(
        delivery
            .deliver(b_record)
            .await
            .unwrap_err()
            .downcast_ref::<control_plane_contracts::ports::LifecycleDeliveryBlocked>()
            .unwrap()
            .0,
        control_plane_contracts::ports::LifecycleDeliveryPauseReason::AuthorityRevoked
    );
    let c_record = processed
        .iter()
        .find(|f| f.subscriber_id.ends_with("acme.composition-c.events"))
        .unwrap();
    delivery.deliver(c_record).await.unwrap();
    let a = installations[0].installation.id;
    let grants = authority.query(&actor, a).await.unwrap();
    let publish = grants
        .authorizations
        .iter()
        .find(|g| g.permission == "event.publish")
        .unwrap();
    authority
        .revoke(
            &actor,
            a,
            RevokeContributionPermission {
                authorization_id: publish.id,
                expected_revision: grants.revision,
            },
        )
        .await
        .unwrap();
    assert!(
        delivery.deliver(committed).await.is_err(),
        "frozen graph cannot restore a revoked publication grant"
    );
    // P06: actual dispatcher persists explicit unavailability/revocation without blind retries.
    store
        .retry_lifecycle_fact(
            b_record.event_id,
            &b_record.subscriber_id,
            worker,
            b_record.claim_id.unwrap(),
            time::OffsetDateTime::now_utc() - time::Duration::seconds(1),
            "fixture redelivery",
        )
        .await
        .unwrap();
    // Persisted disable precedes replacement graph assembly; the frozen binding must still deny.
    control_plane_contracts::ports::PluginRepository::update_desired_state(
        &store,
        &control_plane_contracts::ports::UpdatePluginDesiredStateInput {
            installation_id: installations[2].installation.id,
            desired_state: domain::PluginDesiredState::Disabled,
            actor_user_id: actor_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        delivery
            .deliver(c_record)
            .await
            .unwrap_err()
            .downcast_ref::<control_plane_contracts::ports::LifecycleDeliveryBlocked>()
            .unwrap()
            .0,
        control_plane_contracts::ports::LifecycleDeliveryPauseReason::InstallationInactive
    );
    store
        .retry_lifecycle_fact(
            c_record.event_id,
            &c_record.subscriber_id,
            worker,
            c_record.claim_id.unwrap(),
            time::OffsetDateTime::now_utc() - time::Duration::seconds(1),
            "fixture disabled delivery",
        )
        .await
        .unwrap();
    let unavailable_id = Uuid::now_v7();
    store
        .record_lifecycle_fact(&control_plane_contracts::ports::RecordLifecycleFactInput {
            event_id: unavailable_id,
            transaction_id: Uuid::now_v7(),
            contract_id: MANAGED_PROCESSED_EVENT_ID.into(),
            contract_version: "1".into(),
            canonical_payload: processed[0].canonical_payload.clone(),
            occurred_at: time::OffsetDateTime::now_utc(),
            publication: control_plane_contracts::ports::LifecyclePublicationPlan {
                graph_fingerprint: "retired-graph".into(),
                subscribers: vec![control_plane_contracts::ports::LifecycleSubscriberTarget {
                    subscriber_id: "unavailable".into(),
                    handler_id: "retired-handler".into(),
                    handler_version: "retired-version".into(),
                }],
            },
        })
        .await
        .unwrap();
    let dispatcher = control_plane::lifecycle_outbox_dispatcher::LifecycleOutboxDispatcher::new(
        store.clone(),
        Arc::new(delivery),
        Arc::new(crate::ApiLifecycleDeliveryCompletion),
    );
    assert_eq!(dispatcher.run_once().await.unwrap(), 3);
    let paused: Vec<(String, String)> = sqlx::query_as("select subscriber_id, pause_reason from lifecycle_outbox_deliveries where status='paused' order by subscriber_id").fetch_all(store.pool()).await.unwrap();
    assert!(paused.contains(&(b_record.subscriber_id.clone(), "authority_revoked".into())));
    assert!(paused.contains(&("unavailable".into(), "frozen_graph_unavailable".into())));
    assert!(paused.contains(&(
        c_record.subscriber_id.clone(),
        "installation_inactive".into()
    )));
    assert_eq!(dispatcher.run_once().await.unwrap(), 0);
    for installed in installations {
        let trace =
            PathBuf::from(installed.local_artifact.local_path.unwrap()).join("bin/worker.trace");
        let frame: ManagedEventHostFrame = serde_json::from_str(
            std::fs::read_to_string(trace)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            frame.execution_identity.workspace_id().as_str(),
            actor.current_workspace_id.to_string()
        );
        assert!(serde_json::to_value(frame)
            .unwrap()
            .get("actor_id")
            .is_none());
    }
}
