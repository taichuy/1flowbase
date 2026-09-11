use super::*;
use control_plane::{
    network_egress::{
        CreateNetworkEgressProxyCommand, NetworkEgressProviderService,
        UpdateNetworkEgressProxyCommand,
    },
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
    ports::{NetworkEgressRuntimePort, NetworkEgressSecretMaterial},
};

struct NoExtensionRuntime;
#[async_trait::async_trait]
impl NetworkEgressRuntimePort for NoExtensionRuntime {
    async fn unload_network_egress_provider(&self, _: Uuid) -> anyhow::Result<()> {
        panic!("built-in proxies do not use an extension runtime")
    }
    async fn preflight_network_egresses(
        &self,
        _: Uuid,
        _: &domain::LocalPluginInstallationRecord,
        _: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<()> {
        panic!("built-in proxies do not use an extension runtime")
    }
    async fn sync_network_egresses(
        &self,
        _: Uuid,
        _: &domain::LocalPluginInstallationRecord,
        _: NetworkEgressSecretMaterial,
    ) -> anyhow::Result<Vec<extension_contracts::EgressDescriptor>> {
        panic!("built-in proxies do not use an extension runtime")
    }
}

#[tokio::test]
async fn ac_002_proxy_edit_preserves_secret_member_and_route_and_rejects_invalid_updates() {
    let (store, actor) = store().await;
    let key = "proxy-edit-test-key".to_string();
    let service = NetworkEgressProviderService::new(
        store.clone(),
        NoExtensionRuntime,
        ProviderRegistryNetworkEgressSecretResolver::new(store.clone(), key.clone()),
        key.clone(),
        "test".into(),
    );
    let created = service.create_proxy(CreateNetworkEgressProxyCommand {
        actor_user_id: actor.id, provider_code: "builtin_static_http".into(), display_name: "Original".into(),
        description: "Original description".into(), config: json!({"host":"127.0.0.1","port":"3128","username":"alice","password":"keep-me"}),
    }).await.unwrap();
    let id = created.provider.id;
    let original = store
        .list_network_egress_pool_members(GLOBAL_NETWORK_EGRESS_POOL_ID)
        .await
        .unwrap()
        .remove(0);
    store
        .update_network_egress_pool_member(
            &control_plane::ports::UpdateNetworkEgressPoolMemberInput {
                pool_id: original.pool_id,
                member_id: original.id,
                enabled: false,
                sequence: 7,
                actor_user_id: actor.id,
            },
        )
        .await
        .unwrap();
    sqlx::query("update network_egress_pool_members set probe_status='succeeded',probe_http_status='succeeded',probe_https_status='succeeded',last_probed_at=now(),probe_exit_ip='203.0.113.1' where id=$1")
        .bind(original.id).execute(store.pool()).await.unwrap();
    let workspace_id =
        sqlx::query_scalar::<_, Uuid>("select id from workspaces where name='network-egress'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    let route = store
        .create_network_egress_route(&CreateNetworkEgressRouteInput {
            route_id: Uuid::now_v7(),
            workspace_id,
            selector: domain::NetworkEgressConsumerSelector::GithubOfficialSources,
            pool_id: original.pool_id,
            pool_member_ids: vec![original.id],
            enabled: true,
            actor_user_id: actor.id,
        })
        .await
        .unwrap();
    let view = service.get_proxy(id).await.unwrap();
    assert_eq!(
        view.config,
        json!({"host":"127.0.0.1","port":"3128","username":"alice"})
    );
    assert_eq!(view.configured_secret_fields, vec!["password"]);
    for password in [None, Some("")] {
        let mut config = json!({"host":"203.0.113.20","port":"8080","username":"bob"});
        if let Some(password) = password {
            config["password"] = json!(password);
        }
        service
            .update_proxy(UpdateNetworkEgressProxyCommand {
                actor_user_id: actor.id,
                provider_id: id,
                provider_code: "builtin_static_http".into(),
                display_name: "Updated".into(),
                description: "Updated description".into(),
                config,
            })
            .await
            .unwrap();
        let secret = store
            .resolve_network_egress_provider_secret_json(id, &created.provider.secret_ref, &key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            secret,
            json!({"host":"203.0.113.20","port":8080,"username":"bob","password":"keep-me"})
        );
    }
    let view = service.get_proxy(id).await.unwrap();
    assert_eq!(view.display_name, "Updated");
    assert_eq!(view.description, "Updated description");
    let member = store
        .list_network_egress_pool_members(original.pool_id)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(member.id, original.id);
    assert_eq!(member.provider_id, id);
    assert!(!member.enabled);
    assert_eq!(member.sequence, 7);
    assert_eq!(
        member.probe_status,
        domain::NetworkEgressPoolMemberProbeStatus::NotTested
    );
    assert!(member.last_probed_at.is_none());
    assert!(member.probe_exit_ip.is_none());
    let routes = store
        .list_network_egress_routes(workspace_id)
        .await
        .unwrap();
    assert_eq!(routes[0].id, route.id);
    assert_eq!(routes[0].pool_member_ids, vec![original.id]);
    for (provider_code, config) in [
        (
            "builtin_static_http",
            json!({"host":"bad/host","port":"8080"}),
        ),
        (
            "builtin_static_http",
            json!({"host":"127.0.0.1","port":"0"}),
        ),
        ("other-type", json!({"host":"127.0.0.1","port":"8080"})),
    ] {
        assert!(service
            .update_proxy(UpdateNetworkEgressProxyCommand {
                actor_user_id: actor.id,
                provider_id: id,
                provider_code: provider_code.into(),
                display_name: "Must not save".into(),
                description: "".into(),
                config,
            })
            .await
            .is_err());
        assert_eq!(service.get_proxy(id).await.unwrap().display_name, "Updated");
    }
    service
        .update_proxy(UpdateNetworkEgressProxyCommand {
            actor_user_id: actor.id,
            provider_id: id,
            provider_code: "builtin_static_http".into(),
            display_name: "Updated".into(),
            description: "Updated description".into(),
            config: json!({"host":"203.0.113.20","port":"8080","password":"replacement"}),
        })
        .await
        .unwrap();
    let secret = store
        .resolve_network_egress_provider_secret_json(id, &created.provider.secret_ref, &key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(secret["password"], "replacement");
    assert!(service
        .get_proxy(id)
        .await
        .unwrap()
        .config
        .get("password")
        .is_none());
}
