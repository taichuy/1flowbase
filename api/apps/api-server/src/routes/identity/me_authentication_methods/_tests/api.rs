//! Actual router + PostgreSQL + installed managed verifier. No fake business repository.
use super::*;
use axum::{
    body::{to_bytes, Body},
    http::Request,
    Router,
};
use control_plane::{
    auth::{AuthKernel, CompleteExternalLoginCommand, SessionIssuer},
    ports::SessionStore,
};
use serde_json::{json, Value};
use tower::ServiceExt;

const BASE: &str = "/api/console/me/authentication-methods";

struct Fixture {
    state: Arc<ApiState>,
    host: Arc<runtime_extension_host::RuntimeExtensionHost>,
    app: Router,
    entry: domain::LoginEntryRecord,
    actor: domain::ActorContext,
    root: domain::ActorContext,
    identity: domain::VerifiedExternalIdentity,
    session: domain::SessionRecord,
    identity_id: Uuid,
    cookie: String,
}
impl Fixture {
    async fn new() -> Self {
        let (state, host, entry, root) =
            crate::routes::external_auth::authority_tests::installed_verifier_fixture().await;
        let provider = crate::authentication_runtime::provider(&state, &entry.auth_type)
            .await
            .unwrap();
        let identity = domain::VerifiedExternalIdentity::try_from(domain::ExternalIdentityClaim {
            connection_id:entry.connection_id,subject_type:"provider-user-id".into(),subject_value:"42".into(),
            issuer:Some("https://provider.example".into()),realm:None,verified:true,
            profile:json!({"email":"external-fixture@example.com","email_verified":true,"account":"fixture-member-42",
                "nickname":"Actual provider user","avatar_url":"https://provider.example/avatar/42",
                "profile_url":"https://provider.example/users/42","access_token":"private-provider-access-token",
                "provider_private_key":"private-profile-marker"}),
            metadata:json!({"installation_id":provider.installation_id}),
        }).unwrap().with_authority_namespace(provider.authority_namespace).unwrap();
        let kernel = AuthKernel::new(
            state.store.clone(),
            SessionIssuer::new(state.session_store.clone(), 7),
        );
        let login = kernel
            .complete_external_login(CompleteExternalLoginCommand {
                login_entry_id: entry.id,
                identity: identity.clone(),
                registration_eligible: true,
            })
            .await
            .unwrap();
        assert!(!login.actor.is_root);
        let identity_id = state
            .store
            .find_external_identity_binding(&identity)
            .await
            .unwrap()
            .unwrap()
            .identity_id;
        let mut root_identity = domain::ExternalIdentityClaim {
            connection_id: entry.connection_id,
            subject_type: "provider-user-id".into(),
            subject_value: "root-owned-only".into(),
            issuer: Some("https://provider.example".into()),
            realm: None,
            verified: true,
            profile: json!({"account":"Root-private-provider-account","nickname":"Root private nickname"}),
            metadata: json!({}),
        };
        root_identity.metadata = json!({"installation_id":provider.installation_id});
        let root_identity = domain::VerifiedExternalIdentity::try_from(root_identity)
            .unwrap()
            .with_authority_namespace(identity.authority_namespace().into())
            .unwrap();
        kernel
            .bind_external_identity(&root, root_identity)
            .await
            .unwrap();
        let app = crate::app_with_state(state.clone());
        let cookie = format!("{}={}", state.cookie_name, login.session.session_id);
        Self {
            state,
            host,
            app,
            entry,
            actor: login.actor,
            root,
            identity,
            identity_id,
            session: login.session,
            cookie,
        }
    }
    async fn request(&self, method: &str, path: &str, body: Option<Value>, csrf: bool) -> Response {
        request(
            &self.app,
            method,
            path,
            Some(&self.cookie),
            csrf.then_some(self.session.csrf_token.as_str()),
            body,
        )
        .await
    }
    async fn backup(&self) {
        let identity = domain::VerifiedExternalIdentity::try_from(domain::ExternalIdentityClaim {
            connection_id: self.entry.connection_id,
            subject_type: "provider-user-id".into(),
            subject_value: "backup-owned".into(),
            issuer: Some("https://provider.example".into()),
            realm: None,
            verified: true,
            profile: json!({"account":"backup-owned"}),
            metadata: json!({}),
        })
        .unwrap()
        .with_authority_namespace(self.identity.authority_namespace().into())
        .unwrap();
        AuthKernel::new(
            self.state.store.clone(),
            SessionIssuer::new(self.state.session_store.clone(), 7),
        )
        .bind_external_identity(&self.actor, identity)
        .await
        .unwrap();
    }
    async fn verify_password(&self) {
        let response = self
            .request(
                "POST",
                &format!("{BASE}/password/authorize"),
                Some(json!({"login_entry_id":self.entry.id})),
                true,
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let browser = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|header| {
                header
                    .to_str()
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("; ");
        let body = payload(response).await;
        let authorization_url = body["data"]["authorization_url"].as_str().unwrap();
        assert!(!authorization_url.contains(&self.session.session_id));
        let query = callback_query(authorization_url);
        let cookies = format!("{}; {browser}", self.cookie);
        let response = request(
            &self.app,
            "GET",
            &format!("/api/public/auth/external/callback?{query}"),
            Some(&cookies),
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers()[header::LOCATION],
            "/me/security?auth_verified=true&security_tab=password"
        );
        assert!(
            !response
                .headers()
                .get_all("set-cookie")
                .iter()
                .any(|header| header
                    .to_str()
                    .unwrap()
                    .starts_with(&format!("{}=", self.state.cookie_name))),
            "step-up must not mint a replacement login session"
        );
    }
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<Value>,
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    let body = match body {
        Some(value) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    app.clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
}
async fn payload(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
fn callback_query(authorization_url: &str) -> String {
    let url = url::Url::parse(authorization_url).unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("state", &state)
        .append_pair("code", "accepted")
        .finish()
}

#[tokio::test]
async fn authentication_methods_ordinary_member_reads_only_owned_safe_provider_details() {
    let fixture = Fixture::new().await;
    let unauthorized = request(&fixture.app, "GET", BASE, None, None, None).await;
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let response = fixture.request("GET", BASE, None, false).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let data = payload(response).await;
    assert_eq!(data["data"]["password_configured"], false);
    assert_eq!(data["data"]["password_usable"], false);
    let providers = data["data"]["providers"].as_array().unwrap();
    let provider = providers
        .iter()
        .find(|provider| provider["auth_type"] == "external-fixture")
        .unwrap();
    assert_eq!(provider["display_name"], "Controlled verifier");
    assert_eq!(provider["provider_available"], true);
    let bindings = provider["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);
    let binding = &bindings[0];
    assert_eq!(binding["identity_id"], fixture.identity_id.to_string());
    assert_eq!(binding["account"], "fixture-member-42");
    assert_eq!(binding["nickname"], "Actual provider user");
    assert_eq!(binding["avatar_url"], "https://provider.example/avatar/42");
    assert_eq!(binding["profile_url"], "https://provider.example/users/42");
    assert_eq!(binding["subject_value"], "42");
    assert_eq!(binding["issuer"], "https://provider.example");
    assert_eq!(binding["state"], "bound");
    assert!(binding["bound_at"].as_str().is_some());
    assert_eq!(
        binding["unlink_blocked_reason"],
        "last_usable_authentication_method"
    );
    assert_eq!(binding["can_unlink"], false);
    let text = data.to_string();
    for secret in [
        "private-provider-access-token",
        "private-profile-marker",
        "Root-private-provider-account",
        "Root private nickname",
        "password_hash",
        "provider_private_key",
        "access_token",
    ] {
        assert!(
            !text.contains(secret),
            "personal methods must not leak another owner or raw credential/profile data"
        );
    }
    assert!(binding.get("profile").is_none() && binding.get("metadata").is_none());
    fixture.host.stop().await.unwrap();
}

#[tokio::test]
async fn authentication_methods_unlink_requires_csrf_preserves_last_method_and_rejects_anonymous_reregistration(
) {
    let fixture = Fixture::new().await;
    let path = format!("{BASE}/external-identities/{}/unlink", fixture.identity_id);
    let denied = fixture.request("POST", &path, None, false).await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let last = fixture.request("POST", &path, None, true).await;
    assert_eq!(last.status(), StatusCode::CONFLICT);
    assert_eq!(
        payload(last).await["code"],
        "last_usable_authentication_method"
    );
    let root_binding: Uuid = sqlx::query_scalar(
        "select id from user_auth_identities where user_id=$1 and connection_id=$2",
    )
    .bind(fixture.root.user_id)
    .bind(fixture.entry.connection_id)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    let foreign = fixture
        .request(
            "POST",
            &format!("{BASE}/external-identities/{root_binding}/unlink"),
            None,
            true,
        )
        .await;
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
    fixture.backup().await;
    let response = fixture.request("POST", &path, None, true).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let binding = fixture
        .state
        .store
        .find_external_identity_binding(&fixture.identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.user_id, fixture.actor.user_id);
    assert_eq!(binding.state, "unlinked");
    let before:(i64,i64) = sqlx::query_as("select (select count(*) from users),(select count(*) from audit_logs where event_code='member.external_registered')")
        .fetch_one(fixture.state.store.pool()).await.unwrap();
    // Signup remains enabled. The real callback must retain the unlinked subject's owner.
    let (browser, url) = crate::routes::external_auth::begin_for_purpose(
        &fixture.state,
        fixture.entry.clone(),
        None,
        Purpose::Bind,
    )
    .await
    .unwrap();
    let cookies = browser
        .iter()
        .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
        .collect::<Vec<_>>()
        .join("; ");
    let response = request(
        &fixture.app,
        "GET",
        &format!(
            "/api/public/auth/external/callback?{}",
            callback_query(&url)
        ),
        Some(&cookies),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/sign-in?auth_error=identity_unlinked"
    );
    let after:(i64,i64) = sqlx::query_as("select (select count(*) from users),(select count(*) from audit_logs where event_code='member.external_registered')")
        .fetch_one(fixture.state.store.pool()).await.unwrap();
    assert_eq!(before, after);
    fixture.host.stop().await.unwrap();
}

#[tokio::test]
async fn authentication_methods_password_setup_requires_proof_and_success_invalidates_old_session()
{
    let fixture = Fixture::new().await;
    let path = format!("{BASE}/password");
    let denied = fixture
        .request(
            "POST",
            &path,
            Some(json!({"new_password":"first-password-2026"})),
            true,
        )
        .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        payload(denied).await["code"],
        "password_setup_verification_required"
    );
    fixture.verify_password().await;
    let methods = payload(fixture.request("GET", BASE, None, false).await).await;
    assert_eq!(methods["data"]["password_setup_verified"], true);
    let response = fixture
        .request(
            "POST",
            &path,
            Some(json!({"new_password":"first-password-2026"})),
            true,
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(response
        .headers()
        .get_all("set-cookie")
        .iter()
        .any(|header| header
            .to_str()
            .unwrap()
            .starts_with(&format!("{}=", fixture.state.cookie_name))));
    assert!(fixture
        .state
        .session_store
        .get(&fixture.session.session_id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        fixture.request("GET", BASE, None, false).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request(
                "POST",
                &path,
                Some(json!({"new_password":"replayed-password"})),
                true
            )
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // Reuse the project's real password-login fixture, rather than bypassing authentication.
    let (cookie, _) = crate::_tests::support::login_and_capture_cookie(
        &fixture.app,
        "fixture-member-42",
        "first-password-2026",
    )
    .await;
    let response = request(&fixture.app, "GET", BASE, Some(&cookie), None, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let methods = payload(response).await;
    assert_eq!(methods["data"]["password_configured"], true);
    assert_eq!(methods["data"]["password_usable"], true);
    assert_eq!(methods["data"]["password_setup_verified"], false);
    fixture.host.stop().await.unwrap();
}

#[tokio::test]
async fn authentication_methods_stepup_rechecks_owned_bound_identity_and_does_not_rebind() {
    let fixture = Fixture::new().await;
    let before: i64 = sqlx::query_scalar("select count(*) from user_auth_identities")
        .fetch_one(fixture.state.store.pool())
        .await
        .unwrap();
    let error = issue_password_proof(
        &fixture.state,
        &fixture.root,
        transaction::digest("root-session"),
        fixture.entry.id,
        &fixture.identity,
    )
    .await
    .unwrap_err();
    assert!(error
        .0
        .to_string()
        .contains("password_setup_identity_required"));
    fixture.backup().await;
    AuthenticationMethodsService::new(fixture.state.store.clone())
        .unlink(
            &fixture.actor,
            fixture.identity_id,
            &[fixture.identity.authority_namespace().to_owned()],
        )
        .await
        .unwrap();
    let error = issue_password_proof(
        &fixture.state,
        &fixture.actor,
        transaction::digest(&fixture.session.session_id),
        fixture.entry.id,
        &fixture.identity,
    )
    .await
    .unwrap_err();
    assert!(error
        .0
        .to_string()
        .contains("password_setup_identity_required"));
    let response = fixture
        .request(
            "POST",
            &format!("{BASE}/password/authorize"),
            Some(json!({"login_entry_id":fixture.entry.id})),
            true,
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "backup identity can authorize the same provider, but callback subject still must be bound"
    );
    let browser = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|header| {
            header
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("; ");
    let data = payload(response).await;
    let query = callback_query(data["data"]["authorization_url"].as_str().unwrap());
    let response = request(
        &fixture.app,
        "GET",
        &format!("/api/public/auth/external/callback?{query}"),
        Some(&format!("{}; {browser}", fixture.cookie)),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/me/security?auth_error=verification_failed"
    );
    let binding = fixture
        .state
        .store
        .find_external_identity_binding(&fixture.identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.state, "unlinked");
    assert_eq!(binding.user_id, fixture.actor.user_id);
    let after: i64 = sqlx::query_scalar("select count(*) from user_auth_identities")
        .fetch_one(fixture.state.store.pool())
        .await
        .unwrap();
    assert_eq!(
        after,
        before + 1,
        "the only added identity was the explicit backup"
    );
    assert!(proof::read(
        &fixture.state,
        fixture.actor.user_id,
        &fixture.session.session_id
    )
    .await
    .unwrap()
    .is_none());
    fixture.host.stop().await.unwrap();
}

#[tokio::test]
async fn authentication_methods_password_setup_rejects_stale_entry_connection_and_provider_fingerprints(
) {
    let fixture = Fixture::new().await;
    let body = || Some(json!({"new_password":"first-password-2026"}));
    let path = format!("{BASE}/password");
    fixture.verify_password().await;
    sqlx::query("update login_entries set title='Changed after verification' where id=$1")
        .bind(fixture.entry.id)
        .execute(fixture.state.store.pool())
        .await
        .unwrap();
    let response = fixture.request("POST", &path, body(), true).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        payload(response).await["code"],
        "password_setup_verification_required"
    );
    sqlx::query("update login_entries set title=$2 where id=$1")
        .bind(fixture.entry.id)
        .bind(&fixture.entry.title)
        .execute(fixture.state.store.pool())
        .await
        .unwrap();
    fixture.verify_password().await;
    let old_config: Value =
        sqlx::query_scalar("select config from authentication_connections where id=$1")
            .bind(fixture.entry.connection_id)
            .fetch_one(fixture.state.store.pool())
            .await
            .unwrap();
    sqlx::query("update authentication_connections set config=config || '{\"configuration_revision\":2}'::jsonb where id=$1").bind(fixture.entry.connection_id).execute(fixture.state.store.pool()).await.unwrap();
    let response = fixture.request("POST", &path, body(), true).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        payload(response).await["code"],
        "password_setup_verification_required"
    );
    sqlx::query("update authentication_connections set config=$2 where id=$1")
        .bind(fixture.entry.connection_id)
        .bind(old_config)
        .execute(fixture.state.store.pool())
        .await
        .unwrap();
    fixture.verify_password().await;
    let mut stale = proof::read(
        &fixture.state,
        fixture.actor.user_id,
        &fixture.session.session_id,
    )
    .await
    .unwrap()
    .unwrap();
    stale.provider_fingerprint = "previous-installed-provider-fingerprint".into();
    proof::issue(&fixture.state, stale).await.unwrap();
    let response = fixture.request("POST", &path, body(), true).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        payload(response).await["code"],
        "password_setup_verification_required"
    );
    let user = fixture
        .state
        .store
        .find_user_by_id(fixture.actor.user_id)
        .await
        .unwrap()
        .unwrap();
    assert!(user.password_hash.starts_with('!'));
    assert_eq!(user.session_version, 1);
    fixture.host.stop().await.unwrap();
}

#[tokio::test]
async fn authentication_methods_all_four_routes_publish_typed_plans_and_bounded_schemas() {
    use extension_contracts::ManagedProjectionContract;
    use interface_runtime::{BindingId, PrincipalProfile};
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let _app = crate::app_with_state(state.clone());
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    let paths = [
        BASE,
        "/api/console/me/authentication-methods/external-identities/:identity_id/unlink",
        "/api/console/me/authentication-methods/password/authorize",
        "/api/console/me/authentication-methods/password",
    ];
    for (index, id) in interface::BINDING_IDS.iter().enumerate() {
        let plan = registry
            .plan(&BindingId::new(*id).unwrap())
            .expect("actual full catalog contains each personal-authentication route");
        assert_eq!(
            plan.definition().principal_profile(),
            PrincipalProfile::User
        );
        assert_eq!(
            plan.binding().projection().http_route().unwrap().path(),
            paths[index]
        );
        assert_eq!(
            plan.definition().input_contract().contract_id(),
            "personal-authentication-methods-input"
        );
    }
    let mut bounded = 0;
    for descriptor in registry.managed_contracts().filter(|descriptor| {
        descriptor
            .contract
            .contract_id()
            .starts_with("personal-authentication-methods-")
    }) {
        let schema = ManagedProjectionContract {
            contract_id: descriptor.contract.contract_id().into(),
            contract_version: descriptor.contract.version().into(),
            schema: descriptor
                .schema
                .clone()
                .expect("typed managed hook schema is explicit"),
        };
        crate::extension_bus::FrozenManagedProjection::compile(schema)
            .expect("BoundedSchema accepts the actual published contract");
        bounded += 1;
    }
    assert_eq!(bounded, 2);
    let projection = interface_runtime::ManagedInterfaceProjection::from_contract(
        &interface::Input::InitializePassword("private-password-hash".into()),
    )
    .unwrap();
    assert_eq!(
        projection.value(),
        &json!({}),
        "Host credential/proof carrier remains opaque to hooks"
    );
}
