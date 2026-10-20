//! Real installed verifier process + PostgreSQL grant/revoke serialization. The
//! finite transport fixture pauses after its provider response is computed, not
//! during grant admission. No live GitHub or new SDK build is required.
use super::*;
use crate::provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices};
use async_trait::async_trait;
use control_plane::{
    auth::{AuthKernel, SessionIssuer},
    plugin_management::{
        EnablePluginCommand, HostContributionGrantPolicy, InstallUploadedPluginCommand,
        PluginContributionAuthorityService, PluginManagementService, RevokeContributionPermission,
    },
    ports::SessionStore,
};
use interface_runtime::{BindingId, InterfaceInvocationKernel, InterfaceProtocol, PublicPrincipal};
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tower::ServiceExt;

const CONTRIBUTION: &str = "acme.authentication-barrier.verify";

struct CountingSessions {
    inner: Arc<dyn SessionStore>,
    puts: AtomicUsize,
    authority_pool: sqlx::PgPool,
}
#[async_trait]
impl SessionStore for CountingSessions {
    async fn put(&self, session: domain::SessionRecord) -> anyhow::Result<()> {
        assert!(
            !authority_lock_available(&self.authority_pool).await?,
            "the actual session write must retain grant/revoke serialization"
        );
        self.inner.put(session).await?;
        self.puts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn get(&self, id: &str) -> anyhow::Result<Option<domain::SessionRecord>> {
        self.inner.get(id).await
    }
    async fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.inner.delete(id).await
    }
    async fn touch(&self, id: &str, expiry: i64) -> anyhow::Result<()> {
        self.inner.touch(id, expiry).await
    }
}

/// Probe the concrete PostgreSQL authority owner's serialization lock from an
/// independent connection, without waiting or mutating any grant history.
async fn authority_lock_available(pool: &sqlx::PgPool) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let available = sqlx::query_scalar(
        "select pg_try_advisory_xact_lock(hashtextextended('managed-workspace:' || $1::text, 0))",
    )
    .bind(domain::SYSTEM_SCOPE_ID.to_string())
    .fetch_one(&mut *tx)
    .await?;
    tx.rollback().await?;
    Ok(available)
}

fn package() -> Vec<u8> {
    package_with_second_provider(false)
}
fn package_with_second_provider(second_provider: bool) -> Vec<u8> {
    let mut manifest: Value = serde_yaml::from_str(include_str!(
        "../../../../../../../plugins/fixtures/acme.composition-a/manifest.yaml"
    ))
    .unwrap();
    manifest["plugin_id"] = json!("acme.authentication-barrier");
    manifest["binding_targets"] = json!(["system"]);
    manifest["data_models"] = json!([]);
    manifest["runtime"]["protocol"] = json!("stdio_json_multiplex_v1");
    manifest["authentication_providers"] = json!([{
        "auth_type":"external-fixture", "display_name":"Controlled verifier", "config_schema":[],
        "default_public_ui_block":"external-fixture", "public_variable_keys":[], "contribution_id":CONTRIBUTION
    }]);
    manifest["managed"] = json!({
        "module":{"bus_version":"v1","module_id":"acme.authentication-barrier","module_version":"1.0.0","module_kind":"runtime", "contributions":[{
            "contribution_id":CONTRIBUTION,"contributor_module_id":"acme.authentication-barrier","point_id":"1flowbase.authentication.provider",
            "contract_version":"1","required_permissions":["authentication.verify"],"mode":"append"
        }]},
        "execution_bindings":[{"contribution_id":CONTRIBUTION,"execution_mode":"process_per_call","runtime":{"protocol":"stdio_json_multiplex_v1","entry":"bin/worker.py"},"handler":"authenticate","payload":{"kind":"authentication_provider","auth_type":"external-fixture"}}]
    });
    if second_provider {
        let mut descriptor = manifest["authentication_providers"][0].clone();
        descriptor["auth_type"] = json!("external-second");
        descriptor["display_name"] = json!("Second verifier");
        descriptor["contribution_id"] = json!("acme.authentication-barrier.second");
        manifest["authentication_providers"]
            .as_array_mut()
            .unwrap()
            .push(descriptor);
        let mut contribution = manifest["managed"]["module"]["contributions"][0].clone();
        contribution["contribution_id"] = json!("acme.authentication-barrier.second");
        manifest["managed"]["module"]["contributions"]
            .as_array_mut()
            .unwrap()
            .push(contribution);
        let mut binding = manifest["managed"]["execution_bindings"][0].clone();
        binding["contribution_id"] = json!("acme.authentication-barrier.second");
        binding["payload"]["auth_type"] = json!("external-second");
        manifest["managed"]["execution_bindings"]
            .as_array_mut()
            .unwrap()
            .push(binding);
    }
    let manifest = serde_yaml::to_string(&manifest).unwrap();
    plugin_framework::parse_plugin_manifest(&manifest).unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for (path, bytes, mode) in [
        ("manifest.yaml", manifest.as_bytes(), 0o644),
        (
            "bin/worker.py",
            include_bytes!("callback_worker.py").as_slice(),
            0o755,
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        archive.append_data(&mut header, path, bytes).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

fn callback_query(url: &str, code: &str) -> String {
    let url = url::Url::parse(url).unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("state", &state)
        .append_pair("code", code)
        .finish()
}

async fn effects(state: &ApiState) -> (i64, i64, i64) {
    sqlx::query_as("select (select count(*) from users), (select count(*) from user_auth_identities), (select count(*) from audit_logs where event_code in ('user.external_identity_bound','member.external_registered'))")
        .fetch_one(state.store.pool()).await.unwrap()
}

#[tokio::test]
async fn external_callback_revocation_after_worker_response_prevents_bound_login_effects() {
    // Central QA supplies a trusted origin; the fixture never changes process-global env.
    crate::authentication_runtime::callback_url()
        .expect("set API_AUTH_PUBLIC_ORIGIN for the external callback fixture");
    let (initial, database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut owned = (*initial).clone();
    // The final authority lease and business owner deliberately use independent real connections.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .unwrap();
    owned.store = storage_durable_postgres::MainDurableStore::new(pool);
    let sessions = Arc::new(CountingSessions {
        inner: owned.session_store.clone(),
        puts: AtomicUsize::new(0),
        authority_pool: owned.store.pool().clone(),
    });
    owned.session_store = sessions.clone();
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        crate::api_workspace_root().unwrap(),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        vec![],
    )
    .unwrap();
    let host = Arc::new(
        runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
            time::OffsetDateTime::now_utc(),
            Arc::new(ApiRuntimeArtifactResolver::new(
                owned.store.clone(),
                &owned.api_node_id,
                &owned.provider_install_root,
            )),
        )
        .unwrap(),
    );
    host.mark_ready().unwrap();
    owned.provider_runtime = Arc::new(
        ApiRuntimeServices::new_with_runtime_backend(
            host.clone(),
            Arc::new(assembly.compile_graph().unwrap()),
        )
        .unwrap()
        .with_managed_composition(
            owned.store.clone(),
            owned.api_node_id.clone(),
            assembly.module_descriptors().to_vec(),
        ),
    );
    let state = Arc::new(owned);
    let root_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = state
        .store
        .load_actor_context_for_user(root_id)
        .await
        .unwrap();
    let management = PluginManagementService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: root_id,
            file_name: "authentication-barrier.1flowbasepkg".into(),
            package_bytes: package(),
        })
        .await
        .unwrap();
    let installation = installed.installation.id;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: root_id,
            installation_id: installation,
        })
        .await
        .unwrap();
    let entry = LoginEntryRecord {
        id: Uuid::now_v7(),
        connection_id: Uuid::now_v7(),
        auth_type: "external-fixture".into(),
        title: "Verifier fixture".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 1,
        public_ui_block: "fixture".into(),
        options: json!({"extension_config":{"self_registration_enabled":false}}),
    };
    state.store.create_login_entry(&entry).await.unwrap();
    let identity: domain::VerifiedExternalIdentity = domain::ExternalIdentityClaim {
        connection_id: entry.connection_id,
        subject_type: "provider-user-id".into(),
        subject_value: "42".into(),
        issuer: Some("https://provider.example".into()),
        realm: None,
        profile: json!({"email":"external-fixture@example.com","email_verified":true}),
        verified: true,
        metadata: json!({}),
    }
    .try_into()
    .unwrap();
    let identity = identity
        .with_authority_namespace(format!(
            "managed:{}/{}:{}",
            installed.installation.organization,
            installed.installation.provider_code,
            entry.auth_type
        ))
        .unwrap();
    AuthKernel::new(
        state.store.clone(),
        SessionIssuer::new(state.session_store.clone(), state.session_ttl_days),
    )
    .bind_external_identity(&actor, identity)
    .await
    .unwrap();
    let app = crate::app_with_state(state.clone());
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let registry = boot.interface_registry().unwrap().snapshot();

    // Positive control: execute the actual BuiltIn factory, consume its sealed side
    // port exactly once, and preserve its attempt identity through business/session issuance.
    let (jar, url) = begin(&state, entry.clone(), None).await.unwrap();
    let admission = CallbackAdmission::default();
    let authenticated = boot
        .authenticate_invocation::<_, PublicPrincipal>(
            registry.clone(),
            &BindingId::new(interface::CALLBACK_BINDING).unwrap(),
            InterfaceProtocol::Http,
            ExternalCallbackCredential {
                state: state.clone(),
                headers: HeaderMap::new(),
                jar,
                raw_query: Some(callback_query(&url, "accepted")),
                admission: admission.clone(),
            },
        )
        .await
        .unwrap();
    let invocation_id = authenticated.lineage().invocation_id();
    let verified = admission.take().unwrap();
    assert!(
        admission.take().is_err(),
        "sealed factory result is single-use"
    );
    let input = PublicInput::Complete(verified);
    assert_eq!(
        interface_runtime::ManagedInterfaceProjection::from_contract(&input)
            .unwrap()
            .value(),
        &json!({}),
        "verified identity and final authority lease stay opaque to ordinary hooks"
    );
    let outcome = InterfaceInvocationKernel::new(Arc::new(
        super::super::public_residual_interface::PublicResidualAuthorization,
    ))
    .invoke::<PublicInput, PublicOutput, interface::TargetError>(
        registry,
        authenticated.into_envelope(input),
    )
    .await
    .ok()
    .expect("bound identity must sign in with registration disabled");
    assert_eq!(outcome.receipt().invocation_id(), invocation_id);
    let output = outcome.into_value();
    assert_eq!(
        interface_runtime::ManagedInterfaceProjection::from_contract(&output)
            .unwrap()
            .value(),
        &json!({}),
        "issued session and actor facts stay opaque to ordinary hooks"
    );
    let PublicOutput::Login(login) = output else {
        panic!("expected actual session issuance");
    };
    assert_eq!(login.actor.user_id, root_id);
    assert_eq!(sessions.puts.load(Ordering::SeqCst), 1);
    assert!(
        authority_lock_available(state.store.pool()).await.unwrap(),
        "successful business completion explicitly releases final authority"
    );

    let authority = PluginContributionAuthorityService::new(
        state.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id(&state.api_node_id);
    let granted = authority.query(&actor, installation).await.unwrap();
    let grant = granted
        .authorizations
        .iter()
        .find(|grant| {
            grant.contribution_id == CONTRIBUTION && grant.permission == "authentication.verify"
        })
        .unwrap();
    let provider_before = crate::authentication_runtime::provider(&state, &entry.auth_type)
        .await
        .unwrap();
    let (jar, url) = begin(&state, entry, None).await.unwrap();
    let cookie = jar
        .iter()
        .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
        .collect::<Vec<_>>()
        .join("; ");
    let barrier = std::env::temp_dir().join(format!("external-auth-response-{}", Uuid::now_v7()));
    let query = callback_query(&url, &format!("pause_after_response:{}", barrier.display()));
    let before = effects(&state).await;
    let pending = tokio::spawn(async move {
        app.oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/public/auth/external/callback?{query}"))
                .header("cookie", cookie)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while !barrier.with_extension("started").is_file() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("real worker must reach its response barrier");
    tokio::time::timeout(
        Duration::from_secs(5),
        authority.revoke(
            &actor,
            installation,
            RevokeContributionPermission {
                authorization_id: grant.id,
                expected_revision: granted.revision,
            },
        ),
    )
    .await
    .expect("revocation must commit while old worker response is paused")
    .unwrap();
    assert!(!pending.is_finished());
    let provider_after = crate::authentication_runtime::provider(&state, "external-fixture")
        .await
        .unwrap();
    assert_eq!(
        provider_before.fingerprint, provider_after.fingerprint,
        "revocation must leave the old frozen worker fingerprint unchanged"
    );
    std::fs::write(barrier.with_extension("release"), "release").unwrap();
    let response = tokio::time::timeout(Duration::from_secs(10), pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()["location"],
        "/sign-in?auth_error=provider_unavailable"
    );
    assert!(!response
        .headers()
        .get_all("set-cookie")
        .iter()
        .any(|value| value
            .to_str()
            .unwrap()
            .starts_with(&format!("{}=", state.cookie_name))));
    assert_eq!(
        sessions.puts.load(Ordering::SeqCst),
        1,
        "revoked old result cannot issue another session"
    );
    assert_eq!(
        effects(&state).await,
        before,
        "revoked old result cannot register or bind an identity"
    );
    host.stop().await.unwrap();
    std::fs::remove_file(barrier.with_extension("started")).unwrap();
    std::fs::remove_file(barrier.with_extension("release")).unwrap();
}

/// Shared real installed verifier fixture for the personal authentication API.
/// Every repository, grant and worker call uses the production implementations.
pub(crate) async fn installed_verifier_fixture() -> (
    Arc<ApiState>,
    Arc<runtime_extension_host::RuntimeExtensionHost>,
    LoginEntryRecord,
    domain::ActorContext,
) {
    installed_verifier_fixture_with_second_provider(false).await
}

pub(crate) async fn installed_verifier_fixture_with_second_provider(
    second_provider: bool,
) -> (
    Arc<ApiState>,
    Arc<runtime_extension_host::RuntimeExtensionHost>,
    LoginEntryRecord,
    domain::ActorContext,
) {
    crate::authentication_runtime::callback_url()
        .expect("set API_AUTH_PUBLIC_ORIGIN for callback fixtures");
    let (initial, database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let mut owned = (*initial).clone();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .unwrap();
    owned.store = storage_durable_postgres::MainDurableStore::new(pool);
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        crate::api_workspace_root().unwrap(),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        vec![],
    )
    .unwrap();
    let host = Arc::new(
        runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
            time::OffsetDateTime::now_utc(),
            Arc::new(ApiRuntimeArtifactResolver::new(
                owned.store.clone(),
                &owned.api_node_id,
                &owned.provider_install_root,
            )),
        )
        .unwrap(),
    );
    host.mark_ready().unwrap();
    owned.provider_runtime = Arc::new(
        ApiRuntimeServices::new_with_runtime_backend(
            host.clone(),
            Arc::new(assembly.compile_graph().unwrap()),
        )
        .unwrap()
        .with_managed_composition(
            owned.store.clone(),
            owned.api_node_id.clone(),
            assembly.module_descriptors().to_vec(),
        ),
    );
    let state = Arc::new(owned);
    let root_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = state
        .store
        .load_actor_context_for_user(root_id)
        .await
        .unwrap();
    let management = PluginManagementService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: root_id,
            file_name: "authentication-methods.1flowbasepkg".into(),
            package_bytes: package_with_second_provider(second_provider),
        })
        .await
        .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: root_id,
            installation_id: installed.installation.id,
        })
        .await
        .unwrap();
    let entry = LoginEntryRecord {
        id: Uuid::now_v7(),
        connection_id: Uuid::now_v7(),
        auth_type: "external-fixture".into(),
        title: "Controlled verifier".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 1,
        public_ui_block: "external-fixture".into(),
        options: json!({"extension_config":{"self_registration_enabled":true}}),
    };
    state.store.create_login_entry(&entry).await.unwrap();
    if second_provider {
        let mut second = entry.clone();
        second.id = Uuid::now_v7();
        second.connection_id = Uuid::now_v7();
        second.auth_type = "external-second".into();
        second.title = "Second verifier".into();
        state.store.create_login_entry(&second).await.unwrap();
    }
    (state, host, entry, actor)
}
