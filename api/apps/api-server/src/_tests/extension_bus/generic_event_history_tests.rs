//! Root #2014 AC008: generic identities survive install/version/runtime changes conservatively.
use super::{
    generic_event_tests::{manifest, Fixture},
    managed_event_authority_tests::package,
    managed_snapshot_tests::RuntimeFixture,
};
use crate::provider_runtime::ApiProviderRuntime;
use control_plane::{lifecycle_outbox_dispatcher::LifecycleFactDeliveryPort, plugin_management::*};
use control_plane_contracts::ports::*;
use uuid::Uuid;

#[tokio::test]
async fn root_2014_ac_008_generic_history_and_f01() {
    let fixture = Fixture::new().await;
    fixture.create().await;
    let worker = Uuid::now_v7();
    fixture.advance(worker).await;
    fixture.advance(worker).await;
    let pending = fixture.claim(worker).await;
    let old = pending
        .iter()
        .find(|r| r.subscriber_id.ends_with("lyra.audit-log.store"))
        .unwrap();
    let id = fixture.installations[1];
    let workspace = fixture.actor.current_workspace_id;
    let composition = fixture._services.managed_composition().unwrap();
    let management = PluginManagementService::new(
        fixture.store.clone(),
        ApiProviderRuntime::new(fixture._services.clone()),
        fixture._state.official_plugin_source.clone(),
        &fixture._state.provider_install_root,
    )
    .with_node_id(&fixture._state.api_node_id);
    let governance = ManagedExecutionService::new(fixture.store.clone(), composition.governance());
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: fixture.actor.user_id,
            installation_id: id,
        })
        .await
        .unwrap();
    let history = governance.query(&fixture.actor, id).await.unwrap();
    let row = history
        .deliveries
        .iter()
        .find(|r| r.event_id == old.event_id && r.subscriber_id == old.subscriber_id)
        .unwrap();
    assert_eq!(row.contract_id, "lyra.audit-log.recorded");
    assert_eq!(row.contract_version, "1");
    assert_eq!(row.ownership, "verified");
    assert_eq!(row.status, "paused");
    assert!(fixture
        .store
        .managed_installation_has_backlog(id, Some(workspace), None)
        .await
        .unwrap());
    assert!(composition
        .guard_managed_artifact_removal(&[id])
        .await
        .is_err());
    let exact = ResumeManagedLifecycleDelivery {
        event_id: old.event_id,
        subscriber_id: old.subscriber_id.clone(),
        expected: row.target.clone(),
    };
    assert!(
        governance
            .resume(&fixture.actor, id, exact.clone())
            .await
            .is_err(),
        "disabled installation cannot resume"
    );
    let mut wrong = exact.clone();
    wrong.expected.handler_version.push_str("-forged");
    assert!(governance.resume(&fixture.actor, id, wrong).await.is_err());
    assert!(
        governance
            .retire(&fixture.actor, id, exact.expected.clone())
            .await
            .is_err(),
        "pending old delivery prevents retirement"
    );
    // F01 uses exact original bytes, not a generated replacement at the same semantic version.
    let original = package(&manifest("lyra.audit-log"));
    let reinstalled = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: fixture.actor.user_id,
            file_name: "lyra.1flowbasepkg".into(),
            package_bytes: original,
        })
        .await
        .unwrap();
    assert_eq!(reinstalled.installation.id, id);
    let mut changed = manifest("lyra.audit-log");
    changed["description"] = "different immutable same-version bytes".into();
    assert!(management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: fixture.actor.user_id,
            file_name: "lyra.1flowbasepkg".into(),
            package_bytes: package(&changed)
        })
        .await
        .is_err());
    let after = fixture
        .store
        .managed_lifecycle_delivery(id, workspace, &exact)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.handler_version, old.handler_version);
    assert_eq!(after.canonical_payload, old.canonical_payload);
    // A new installed/assigned/authorized version must not retarget old facts or clear their history.
    let mut upgraded = manifest("lyra.audit-log");
    upgraded["version"] = "2.0.0".into();
    upgraded["managed"]["module"]["module_version"] = "2.0.0".into();
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: fixture.actor.user_id,
            file_name: "lyra-v2.1flowbasepkg".into(),
            package_bytes: package(&upgraded),
        })
        .await
        .unwrap();
    let new_id = installed.installation.id;
    assert_ne!(new_id, id);
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: fixture.actor.user_id,
            installation_id: new_id,
        })
        .await
        .unwrap();
    let declared =
        plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&upgraded).unwrap())
            .unwrap();
    let dependencies = crate::routes::plugins::extension_center::ExtensionCenterDependencies {
        store: fixture.store.clone(),
        provider_runtime: fixture._services.clone(),
        official_plugin_source: fixture._state.official_plugin_source.clone(),
        official_mcp_bundle_source: fixture._state.official_mcp_bundle_source.clone(),
        official_extension_catalog_source: fixture._state.official_extension_catalog_source.clone(),
        cache_store: fixture._state.infrastructure.cache_store(),
        provider_install_root: fixture._state.provider_install_root.clone(),
        api_node_id: fixture._state.api_node_id.clone(),
        allow_uploaded_host_extensions: fixture._state.allow_uploaded_host_extensions,
    };
    crate::routes::plugins::extension_center::apply_fixture_managed_schema(
        &dependencies,
        workspace,
        &declared,
    )
    .await
    .unwrap();
    let authority = PluginContributionAuthorityService::new(
        fixture.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    for contribution in &declared.managed.as_ref().unwrap().module.contributions {
        for permission in &contribution.required_permissions {
            let write = permission.as_str() == "plugin_data.owned.write";
            authority
                .grant(
                    &fixture.actor,
                    new_id,
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
            actor_user_id: fixture.actor.user_id,
            installation_id: new_id,
        })
        .await
        .unwrap();
    let history = governance.query(&fixture.actor, id).await.unwrap();
    assert!(history
        .deliveries
        .iter()
        .any(|r| r.event_id == old.event_id && r.target == exact.expected));
    assert!(!governance
        .query(&fixture.actor, new_id)
        .await
        .unwrap()
        .deliveries
        .iter()
        .any(|r| r.event_id == old.event_id));
    let restarted = RuntimeFixture::new(&fixture._state);
    restarted
        .composition
        .rebuild_installation(fixture.installations[0])
        .await
        .unwrap();
    restarted
        .composition
        .rebuild_installation(new_id)
        .await
        .unwrap();
    let restarted_governance =
        ManagedExecutionService::new(restarted.store.clone(), restarted.composition.governance());
    assert!(restarted_governance
        .query(&fixture.actor, id)
        .await
        .unwrap()
        .deliveries
        .iter()
        .any(|r| r.event_id == old.event_id && r.target == exact.expected));
    assert!(
        restarted.delivery.deliver(old).await.is_err(),
        "fresh runtime cannot invent old executable identity"
    );
    assert!(restarted_governance
        .resume(&fixture.actor, id, exact)
        .await
        .is_err());
    // Unknown legacy identity cannot be treated as empty backlog even for an unrelated target set.
    sqlx::query("update lifecycle_outbox_deliveries set handler_version='unknown-legacy' where event_id=$1 and subscriber_id=$2").bind(old.event_id).bind(&old.subscriber_id).execute(fixture.store.pool()).await.unwrap();
    assert!(fixture
        .store
        .managed_installation_has_backlog(id, Some(workspace), Some(&[]))
        .await
        .unwrap());
}
