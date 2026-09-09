//! Root AC-007/AUTH-04/AUTH-10: real installed SDK workers and PostgreSQL owners.
//! Kept in API assembly because the CP integration crate cannot import the real runtime resolver.
use super::managed_event_authority_tests::package;
use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices};
use control_plane::{plugin_management::*, ports::AuthRepository};
use control_plane_contracts::ports::{
    LifecycleOutboxRepository, PluginContributionAuthorityRepository,
};
use extension_contracts::extension_bus::*;
use extension_contracts::*;
use std::sync::Arc;
use uuid::Uuid;

fn manifest_source(name: &str) -> &'static str {
    match name {
        "a" => {
            include_str!("../../../../../plugins/fixtures/acme.composition-a/event-manifest.yaml")
        }
        "b" => include_str!("../../../../../plugins/fixtures/acme.composition-b/manifest.yaml"),
        "c" => include_str!("../../../../../plugins/fixtures/acme.composition-c/manifest.yaml"),
        _ => panic!("finite composition fixture"),
    }
}
fn grant(name: &str, suffix: &str, permission: &str) -> GrantContributionPermission {
    let write = permission == "plugin_data.owned.write";
    GrantContributionPermission {
        contribution_id: format!("acme.composition-{name}.{suffix}"),
        permission: permission.into(),
        resource_scope: if write {
            domain::ContributionResourceScope::OwnedCollection {
                collection_code: "processed_models".into(),
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
    }
}
fn subject(installation_id: Uuid, workspace_id: Uuid, name: &str) -> ManagedContributionSubject {
    ManagedContributionSubject::new(
        ManagedInstallationId::new(installation_id.to_string()).unwrap(),
        ManagedWorkspaceId::new(workspace_id.to_string()).unwrap(),
        ContributionId::new(format!("acme.composition-{name}.events")).unwrap(),
    )
}
fn deadline() -> i64 {
    ((time::OffsetDateTime::now_utc() + time::Duration::seconds(10)).unix_timestamp_nanos()
        / 1_000_000) as i64
}
async fn count(pool: &sqlx::PgPool, table: &str) -> i64 {
    assert!(table
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn root_2007_ac_007_ack_loss_and_claim_fencing() {
    use control_plane::lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort;
    let (state, database_url) = crate::_tests::support::test_api_state_with_database_url().await;
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
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect(&database_url)
        .await
        .unwrap();
    let store = storage_durable_postgres::PgControlPlaneStore::new(pool.clone())
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
    for name in ["a", "b", "c"] {
        let source = manifest_source(name);
        let declared = plugin_framework::parse_plugin_manifest(source).unwrap();
        let author_document: serde_json::Value = serde_yaml::from_str(source).unwrap();
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: actor_id,
                file_name: format!("acme.composition-{name}.1flowbasepkg"),
                package_bytes: package(&author_document),
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
        crate::routes::plugins::extension_center::apply_fixture_managed_schema(
            &schema_dependencies,
            actor.current_workspace_id,
            &declared,
        )
        .await
        .unwrap();
        authority
            .grant(&actor, id, grant(name, "events", "event.subscribe"))
            .await
            .unwrap();
        if name == "a" {
            authority
                .grant(&actor, id, grant(name, "events", "event.publish"))
                .await
                .unwrap();
        } else {
            authority
                .grant(&actor, id, grant(name, "data", "plugin_data.owned.write"))
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
                "AUTH-04: a .data grant cannot supply the event contribution write"
            );
            let denied_subject = subject(id, actor.current_workspace_id, name);
            let denied = store
                .lock_contribution_authority(&denied_subject)
                .await
                .unwrap()
                .commit_owned_event_effect(
                    denied_subject,
                    Uuid::now_v7(),
                    MANAGED_PROCESSED_EVENT_ID.into(),
                    effect_operations(&serde_json::json!({"model_id":Uuid::now_v7().to_string(),"status":"processed","result_reference":null})),
                    deadline(),
                )
                .await;
            assert!(denied.is_err());
            authority
                .grant(&actor, id, grant(name, "events", "plugin_data.owned.write"))
                .await
                .unwrap();
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
    let committed = store
        .claim_lifecycle_facts(worker, 32, time::Duration::seconds(120))
        .await
        .unwrap();
    let a = committed
        .iter()
        .find(|r| r.subscriber_id.ends_with("acme.composition-a.events"))
        .unwrap();
    // Barrier starts two A replays together while a single pooled connection serves all commits.
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let first = async {
        barrier.wait().await;
        delivery.deliver(a).await
    };
    let second = async {
        barrier.wait().await;
        delivery.deliver(a).await
    };
    let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        tokio::join!(first, second)
    })
    .await
    .unwrap();
    first.unwrap();
    second.unwrap();
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from lifecycle_outbox where contract_id='acme.composition-a.processed'").fetch_one(&pool).await.unwrap(),1);
    for record in &committed {
        if record.subscriber_id != a.subscriber_id {
            delivery.deliver(record).await.unwrap();
        }
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
        .claim_lifecycle_facts(worker, 32, time::Duration::seconds(120))
        .await
        .unwrap();
    assert_eq!(processed.len(), 2);
    let b = processed
        .iter()
        .find(|r| r.subscriber_id.ends_with("acme.composition-b.events"))
        .unwrap();
    let c = processed
        .iter()
        .find(|r| r.subscriber_id.ends_with("acme.composition-c.events"))
        .unwrap();
    let fact: ManagedEventFact = serde_json::from_slice(&b.canonical_payload).unwrap();
    assert_eq!(fact.payload["model_id"], model.id.to_string());
    let tables: Vec<(String,String)> = sqlx::query_as("select owner_id,physical_table from plugin_schema_ownership where object_kind='owned_collection' and logical_name='processed_models' order by owner_id").fetch_all(&pool).await.unwrap();
    assert_eq!(tables.len(), 2);
    assert_ne!(tables[0].1, tables[1].1);
    // Deferred receipt failure happens after SQL effects; the entire same transaction must roll back.
    sqlx::query("create function fail_b_receipt() returns trigger language plpgsql as $$ begin if new.owner_id='acme/acme.composition-b' then raise exception 'fixture B receipt commit rejection'; end if; return new; end $$").execute(&pool).await.unwrap();
    sqlx::query("create constraint trigger fail_b_receipt after insert on plugin_data_idempotency_receipts deferrable initially deferred for each row execute function fail_b_receipt()").execute(&pool).await.unwrap();
    let (b_failed, c_ok) = tokio::join!(delivery.deliver(b), delivery.deliver(c));
    assert!(b_failed.is_err());
    c_ok.unwrap();
    assert_eq!(count(&pool, &tables[0].1).await, 0);
    assert_eq!(count(&pool, &tables[1].1).await, 1);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from plugin_data_idempotency_receipts where owner_id='acme/acme.composition-b'").fetch_one(&pool).await.unwrap(),0);
    store
        .mark_lifecycle_fact_delivered(c.event_id, &c.subscriber_id, worker, c.claim_id.unwrap())
        .await
        .unwrap();
    store
        .retry_lifecycle_fact(
            b.event_id,
            &b.subscriber_id,
            worker,
            b.claim_id.unwrap(),
            time::OffsetDateTime::now_utc() - time::Duration::seconds(1),
            "B commit rejected",
        )
        .await
        .unwrap();
    sqlx::query("drop trigger fail_b_receipt on plugin_data_idempotency_receipts")
        .execute(&pool)
        .await
        .unwrap();
    let retried = store
        .claim_lifecycle_facts(worker, 32, time::Duration::seconds(120))
        .await
        .unwrap();
    assert_eq!(retried.len(), 1);
    let b_new = &retried[0];
    assert_ne!(b.claim_id, b_new.claim_id);
    assert!(store
        .mark_lifecycle_fact_delivered(b.event_id, &b.subscriber_id, worker, b.claim_id.unwrap())
        .await
        .unwrap_err()
        .is::<control_plane_contracts::ports::LifecycleClaimLost>());
    // Mutation counter distinguishes receipt replay from another upsert of the same values.
    sqlx::query("create table effect_mutations (owner_table text not null)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("create function count_effect_mutation() returns trigger language plpgsql as $$ begin insert into effect_mutations values (tg_table_name); return new; end $$").execute(&pool).await.unwrap();
    for (_, table) in &tables {
        assert!(table
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        sqlx::query(&format!("create trigger count_effect after insert or update on {table} for each row execute function count_effect_mutation()")).execute(&pool).await.unwrap();
    }
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let first = async {
        barrier.wait().await;
        delivery.deliver(b_new).await
    };
    let second = async {
        barrier.wait().await;
        delivery.deliver(b_new).await
    };
    let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        tokio::join!(first, second)
    })
    .await
    .unwrap();
    first.unwrap();
    second.unwrap();
    // Lost ACK after effect commit, then restart-style stable subject replay has no second mutation.
    delivery.deliver(b_new).await.unwrap();
    delivery.deliver(c).await.unwrap();
    assert_eq!(count(&pool, "effect_mutations").await, 1);
    assert_eq!(count(&pool, &tables[0].1).await, 1);
    assert_eq!(count(&pool, &tables[1].1).await, 1);
    assert_eq!(count(&pool, "plugin_data_idempotency_receipts").await, 2);
    for (_, table) in &tables {
        let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(&format!(
            "select model_id,status,result_reference from {table}"
        ))
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![(
                model.id,
                "processed".into(),
                fact.payload["result_reference"]
                    .as_str()
                    .map(str::to_string)
            )]
        );
    }

    let b_subject = subject(
        installations[1].installation.id,
        actor.current_workspace_id,
        "b",
    );
    let c_subject = subject(
        installations[2].installation.id,
        actor.current_workspace_id,
        "c",
    );
    let lease = store.lock_contribution_authority(&b_subject).await.unwrap();
    assert!(
        lease
            .commit_owned_event_effect(
                c_subject,
                b.event_id,
                MANAGED_PROCESSED_EVENT_ID.into(),
                effect_operations(&fact.payload),
                deadline()
            )
            .await
            .is_err(),
        "cross-owner subject cannot use B's lock"
    );
    // AUTH-10 uses a second connection so the waiter is the actual authority lock, not pool admission.
    let revoker_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .unwrap();
    let revoker_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&revoker_pool)
        .await
        .unwrap();
    let revoker = PluginContributionAuthorityService::new(
        storage_durable_postgres::PgControlPlaneStore::new(revoker_pool.clone()),
        HostContributionGrantPolicy::root_composition(),
    );
    let current = authority
        .query(&actor, installations[1].installation.id)
        .await
        .unwrap();
    let write = current
        .authorizations
        .iter()
        .find(|g| {
            g.contribution_id.ends_with(".events") && g.permission == "plugin_data.owned.write"
        })
        .unwrap();
    let lease = store.lock_contribution_authority(&b_subject).await.unwrap();
    let revoke = revoker.revoke(
        &actor,
        installations[1].installation.id,
        RevokeContributionPermission {
            authorization_id: write.id,
            expected_revision: current.revision,
        },
    );
    tokio::pin!(revoke);
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            tokio::select! {
                result=&mut revoke=>panic!("revoke crossed held authority lock: {result:?}"),
                _=tokio::time::sleep(std::time::Duration::from_millis(10))=>{}
            }
            let blocked: bool = sqlx::query_scalar(
                "select coalesce(wait_event_type='Lock',false) from pg_stat_activity where pid=$1",
            )
            .bind(revoker_pid)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
            if blocked {
                break;
            }
        }
    })
    .await
    .unwrap();
    let mut last_effect = fact.payload.clone();
    last_effect.model_id = Uuid::now_v7().to_string();
    let last_event = Uuid::now_v7();
    assert!(
        !lease
            .commit_owned_event_effect(
                b_subject.clone(),
                last_event,
                MANAGED_PROCESSED_EVENT_ID.into(),
                effect_operations(&last_effect),
                deadline()
            )
            .await
            .unwrap()
            .replayed
    );
    revoke.await.unwrap();
    assert!(delivery.deliver(b_new).await.is_err());
    assert!(store
        .lock_contribution_authority(&b_subject)
        .await
        .unwrap()
        .commit_owned_event_effect(
            b_subject,
            b.event_id,
            MANAGED_PROCESSED_EVENT_ID.into(),
            effect_operations(&fact.payload),
            deadline()
        )
        .await
        .is_err());
    assert!(store
        .mark_lifecycle_fact_delivered(
            b_new.event_id,
            &b_new.subscriber_id,
            worker,
            b_new.claim_id.unwrap(),
        )
        .await
        .unwrap_err()
        .is::<control_plane_contracts::ports::LifecycleClaimLost>());
    let revoked_and_unclaimed: bool = sqlx::query_scalar(
        "select status='paused' and pause_reason='authority_revoked' and paused_at is not null
            and claimed_by is null and claimed_at is null and claim_id is null
            and claim_expires_at is null and delivered_at is null
         from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2",
    )
    .bind(b_new.event_id)
    .bind(&b_new.subscriber_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        revoked_and_unclaimed,
        "committed effects do not revive a revoked delivery claim"
    );
    let c_delivered: bool = sqlx::query_scalar(
        "select status='delivered' and delivered_at is not null
         from lifecycle_outbox_deliveries where event_id=$1 and subscriber_id=$2",
    )
    .bind(c.event_id)
    .bind(&c.subscriber_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        c_delivered,
        "B revocation preserves C's independently acknowledged delivery"
    );
    assert_eq!(count(&pool, "plugin_data_idempotency_receipts").await, 3);
    assert_eq!(count(&pool, "effect_mutations").await, 2);
    assert_eq!(count(&pool, &tables[0].1).await, 2);
    revoker_pool.close().await;
    pool.close().await;
}

fn effect_operations(payload: &serde_json::Value) -> Vec<PluginDataOperation> {
    vec![PluginDataOperation::Upsert {
        target: PluginDataTarget::OwnedCollection {
            collection_code: "processed_models".into(),
        },
        identity: [(
            "model_id".into(),
            PluginDataValue::Uuid(payload["model_id"].as_str().unwrap().into()),
        )]
        .into_iter()
        .collect(),
        values: [
            ("status".into(), PluginDataValue::String("processed".into())),
            (
                "result_reference".into(),
                payload["result_reference"]
                    .as_str()
                    .map(|value| PluginDataValue::String(value.into()))
                    .unwrap_or(PluginDataValue::Null),
            ),
        ]
        .into_iter()
        .collect(),
    }]
}
