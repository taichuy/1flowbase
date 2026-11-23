use crate::_tests::support::{
    default_test_config, login_and_capture_cookie, test_api_state_with_database_url,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use control_plane::auth::WorkspacePolicyPort;
use control_plane_contracts::ports::{
    GrantWorkspaceMembershipInput, WorkspaceGovernanceRepository,
};
use extension_contracts::*;
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU8, Ordering},
};
use storage_durable_postgres::MainDurableStore;
use tower::ServiceExt;
use uuid::Uuid;

const INVITE: &str = "inv_fixture";

struct Policy {
    mode: AtomicU8,
    store: MainDurableStore,
    target: Mutex<Option<Uuid>>,
    finalizations: Mutex<Vec<Option<String>>>,
}

#[async_trait::async_trait]
impl WorkspacePolicyPort for Policy {
    async fn decide(
        &self,
        request: WorkspacePolicyRequest,
    ) -> anyhow::Result<Option<WorkspacePolicyResponse>> {
        let mode = self.mode.load(Ordering::SeqCst);
        if mode == 0 {
            return Ok(None);
        }
        let target = *self.target.lock().unwrap();
        use WorkspacePolicyRequest as Request;
        use WorkspacePolicyResponse as Response;
        use WorkspaceRegistrationDecisionDto as Registration;
        Ok(Some(match request {
            Request::InspectPublicPolicy => Response::PublicPolicy {
                onboarding_mode: if mode == 1 {
                    WorkspaceOnboardingModeDto::PersonalWorkspace
                } else {
                    WorkspaceOnboardingModeDto::InviteOnly
                },
            },
            Request::ResolveRegistration {
                account_hint,
                invitation_token,
                ..
            } => Response::RegistrationDecision {
                decision: if invitation_token.as_deref() == Some(INVITE) {
                    Registration::JoinExistingWorkspace {
                        workspace_id: target.unwrap().to_string(),
                        role_codes: vec!["member".into()],
                        redemption_id: Some("reservation".into()),
                    }
                } else if mode == 1 {
                    let name = account_hint.unwrap();
                    Registration::ProvisionPersonalWorkspace {
                        tenant_code: name.clone(),
                        tenant_name: name.clone(),
                        workspace_name: name,
                    }
                } else {
                    Registration::Deny {
                        reason_code: "invitation_required".into(),
                    }
                },
            },
            Request::FinalizeInvitationRedemption {
                redemption_id,
                committed_user_id,
            } => {
                assert_eq!(redemption_id, "reservation");
                self.finalizations.lock().unwrap().push(committed_user_id);
                Response::RedemptionFinalized
            }
            Request::RedeemInvitationForUser {
                user_id,
                invitation_token,
                ..
            } => {
                assert_eq!(invitation_token, INVITE);
                let workspace_id = target.unwrap();
                let grant = self
                    .store
                    .grant_or_activate_workspace_membership(&GrantWorkspaceMembershipInput {
                        workspace_id,
                        user_id: user_id.parse()?,
                        role_codes: vec!["member".into()],
                        actor_user_id: None,
                    })
                    .await?;
                Response::UserInvitationRedemption {
                    redemption: WorkspaceUserInvitationRedemptionDto::Redeemed {
                        workspace_id: workspace_id.to_string(),
                        already_active_member: grant.already_active_member,
                    },
                }
            }
            Request::AdmitOperation { interface_id, .. } => {
                let reason = match interface_id.as_str() {
                    "members.create" => "member_quota_exceeded",
                    "applications.create" => "application_quota_exceeded",
                    other => panic!("unexpected canonical interface_id in AdmitOperation: {other}"),
                };
                Response::OperationAdmission {
                    admission: WorkspaceOperationAdmissionDto::Deny {
                        reason_code: reason.into(),
                    },
                }
            }
        }))
    }
}

async fn call(
    app: &Router,
    method: Method,
    path: &str,
    body: Value,
    headers: &[(&str, &str)],
) -> (StatusCode, Value, Option<String>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::to_owned);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes))),
        cookie,
    )
}

async fn register(
    app: &Router,
    account: &str,
    token: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    call(app, Method::POST, "/api/public/auth/sign-up", json!({
        "login_entry_id": domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID, "account": account,
        "email": format!("{account}@example.com"), "password": "fixture-password", "invitation_token": token,
    }), &[]).await
}

#[tokio::test]
async fn workspace_policy_controls_registration_redemption_and_fallback() {
    let (mut state, _) = test_api_state_with_database_url().await;
    let default_workspace = state.bootstrap_workspace_id;
    let store = state.store.clone();
    let policy = Arc::new(Policy {
        mode: AtomicU8::new(1),
        store: store.clone(),
        target: Mutex::new(None),
        finalizations: Mutex::new(Vec::new()),
    });
    let runtime = state
        .provider_runtime
        .as_ref()
        .clone()
        .with_workspace_policy_fixture(policy.clone());
    Arc::get_mut(&mut state).unwrap().provider_runtime = Arc::new(runtime);
    let app = crate::app_with_state_and_config(Arc::clone(&state), &default_test_config());
    const REDEEM: &str = "/api/console/workspaces/invitations/redeem";
    use api_host_kernel::external_endpoint_catalog::{
        ExternalEndpointClassification, ExternalEndpointIdentity,
    };
    use utoipa::OpenApi;
    let catalog = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .external_endpoint_catalog()
        .unwrap();
    let endpoint = catalog
        .row(&ExternalEndpointIdentity::http("POST", REDEEM))
        .unwrap();
    assert_eq!(
        endpoint.binding_id(),
        Some("http.console.workspaces.invitations.redeem.v1")
    );
    assert_eq!(
        endpoint.classification(),
        ExternalEndpointClassification::CanonicalBusinessInterface
    );
    let spec = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    let operation = &spec["paths"][REDEEM]["post"];
    assert_eq!(operation["summary"], "Redeem a workspace invitation");
    assert!(operation["description"].is_string());
    assert_eq!(
        spec["components"]["schemas"]["RedeemWorkspaceInvitationBody"]["required"],
        json!(["invitation_token"])
    );
    let (root, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let quota_headers = [
        ("cookie", root.as_str()),
        ("x-csrf-token", root_csrf.as_str()),
    ];
    let (status, body, _) = call(&app, Method::POST, "/api/console/settings/members", json!({
        "account": "quota-test", "email": "quota-test@example.com", "password": "fixture-password",
        "name": "Quota test", "nickname": "Quota", "introduction": "", "email_login_enabled": true, "phone_login_enabled": false,
    }), &quota_headers).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "member_quota_exceeded");
    let (status, body, _) = call(
        &app, Method::POST, "/api/console/applications",
        json!({"application_type":"agent_flow","name":"Quota test","description":"Must not be created"}),
        &quota_headers,
    ).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "application_quota_exceeded");
    let (status, entries, _) = call(
        &app,
        Method::GET,
        "/api/public/auth/login-entries",
        Value::Null,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{entries}");
    assert_eq!(entries["data"]["onboarding_mode"], "personal_workspace");
    assert_eq!(
        entries["data"]["login_entries"][0]["public_variables"]["self_registration_enabled"],
        true
    );
    let (status, personal, _) = register(&app, "personal-host", None).await;
    assert_eq!(status, StatusCode::OK, "{personal}");
    let workspace: Uuid = personal["data"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(workspace, default_workspace);
    assert_eq!(personal["data"]["effective_display_role"], "admin");
    *policy.target.lock().unwrap() = Some(workspace);
    let (status, second, second_cookie) = register(&app, "second-host", None).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    policy.mode.store(2, Ordering::SeqCst);
    let (status, denied, _) = register(&app, "invite-required", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "invitation_required");
    let (status, invited, _) = register(&app, "invited-user", Some(INVITE)).await;
    assert_eq!(status, StatusCode::OK, "{invited}");
    assert_eq!(
        invited["data"]["current_workspace_id"],
        workspace.to_string()
    );
    let (status, duplicate, _) = register(&app, "invited-user", Some(INVITE)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{duplicate}");
    let finalizations = policy.finalizations.lock().unwrap().clone();
    assert!(finalizations[0].is_some());
    assert_eq!(finalizations[1], None);

    // Remove the old user's only membership: login must not require a default workspace
    // before redeeming an invitation for verified credentials.
    sqlx::query("delete from workspace_memberships where user_id = (select id from users where account = 'invited-user')")
        .execute(store.pool()).await.unwrap();
    let (status, logged_in, _) = call(&app, Method::POST, "/api/public/auth/sign-in", json!({
        "identifier": "invited-user", "password": "fixture-password", "invitation_token": INVITE,
    }), &[]).await;
    assert_eq!(status, StatusCode::OK, "{logged_in}");
    assert_eq!(
        logged_in["data"]["current_workspace_id"],
        workspace.to_string()
    );

    let cookie = second_cookie.unwrap();
    let csrf = second["data"]["csrf_token"].as_str().unwrap();
    let endpoint = REDEEM;
    let (status, _, _) = call(
        &app,
        Method::POST,
        endpoint,
        json!({"invitation_token":INVITE}),
        &[("cookie", &cookie), ("x-csrf-token", "invalid-csrf")],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let headers = [("cookie", cookie.as_str()), ("x-csrf-token", csrf)];
    let (status, redeemed, _) = call(
        &app,
        Method::POST,
        endpoint,
        json!({"invitation_token":INVITE}),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{redeemed}");
    assert_eq!(
        redeemed["data"]["actor"]["current_workspace_id"],
        workspace.to_string()
    );
    let next_csrf = redeemed["data"]["csrf_token"].as_str().unwrap();
    assert_ne!(next_csrf, csrf);
    assert!(redeemed["data"]["available_roles"].is_array());
    assert!(redeemed["data"]["active_role_permissions"].is_array());
    let (status, redeemed_again, _) = call(
        &app,
        Method::POST,
        endpoint,
        json!({"invitation_token":INVITE}),
        &[("cookie", &cookie), ("x-csrf-token", next_csrf)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{redeemed_again}");
    let rotated = redeemed_again["data"]["csrf_token"].as_str().unwrap();
    assert_ne!(rotated, next_csrf);
    let next_csrf = rotated;

    policy.mode.store(0, Ordering::SeqCst);
    let (_, entries, _) = call(
        &app,
        Method::GET,
        "/api/public/auth/login-entries",
        Value::Null,
        &[],
    )
    .await;
    assert!(entries["data"].get("onboarding_mode").is_none());
    let (status, denied, _) = register(&app, "unsupported", Some(INVITE)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "invitation_unsupported");
    let (status, denied, _) = register(&app, "fallback", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "self_registration_disabled");
    let (status, denied, _) = call(
        &app,
        Method::POST,
        endpoint,
        json!({"invitation_token":INVITE}),
        &[("cookie", &cookie), ("x-csrf-token", next_csrf)],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "invitation_unsupported");
}

#[tokio::test]
async fn disabled_membership_and_suspended_scopes_immediately_block_cookie_and_pat() {
    let (mut state, _) = test_api_state_with_database_url().await;
    let store = state.store.clone();
    let policy = Arc::new(Policy {
        mode: AtomicU8::new(1),
        store: store.clone(),
        target: Mutex::new(None),
        finalizations: Mutex::new(Vec::new()),
    });
    let runtime = state
        .provider_runtime
        .as_ref()
        .clone()
        .with_workspace_policy_fixture(policy);
    Arc::get_mut(&mut state).unwrap().provider_runtime = Arc::new(runtime);
    let app = crate::app_with_state_and_config(state, &default_test_config());
    let (status, body, cookie) = register(&app, "gated-user", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cookie = cookie.unwrap();
    let csrf = body["data"]["csrf_token"].as_str().unwrap();
    let workspace: Uuid = body["data"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let (status, key, _) = call(
        &app,
        Method::POST,
        "/api/console/user-api-keys",
        json!({"name":"status test","expiration_policy":"never"}),
        &[("cookie", &cookie), ("x-csrf-token", csrf)],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    let bearer = format!("Bearer {}", key["data"]["token"].as_str().unwrap());
    for credentials in [
        [("cookie", cookie.as_str())],
        [("authorization", bearer.as_str())],
    ] {
        let (status, body, _) = call(
            &app,
            Method::GET,
            "/api/console/me",
            Value::Null,
            &credentials,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    for (membership, tenant, workspace_status, reason) in [
        ("disabled", "active", "active", "workspace_access_denied"),
        ("active", "suspended", "active", "tenant_suspended"),
        ("active", "active", "suspended", "workspace_suspended"),
    ] {
        sqlx::query("update workspace_memberships set status = $1 where workspace_id = $2")
            .bind(membership)
            .bind(workspace)
            .execute(store.pool())
            .await
            .unwrap();
        sqlx::query("update tenants set status = $1 where id = (select tenant_id from workspaces where id = $2)").bind(tenant).bind(workspace).execute(store.pool()).await.unwrap();
        sqlx::query("update workspaces set status = $1 where id = $2")
            .bind(workspace_status)
            .bind(workspace)
            .execute(store.pool())
            .await
            .unwrap();
        for credentials in [
            [("cookie", cookie.as_str())],
            [("authorization", bearer.as_str())],
        ] {
            let (status, body, _) = call(
                &app,
                Method::GET,
                "/api/console/me",
                Value::Null,
                &credentials,
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
            assert_eq!(body["code"], reason);
        }
    }
}

#[tokio::test]
async fn external_oauth_callback_registration_rejected_has_no_signup_effects() {
    use control_plane::ports::SessionStore;
    use std::sync::atomic::AtomicUsize;

    #[derive(Default)]
    struct RejectRegistration(AtomicUsize);

    #[async_trait::async_trait]
    impl WorkspacePolicyPort for RejectRegistration {
        async fn decide(
            &self,
            request: WorkspacePolicyRequest,
        ) -> anyhow::Result<Option<WorkspacePolicyResponse>> {
            Ok(Some(match request {
                WorkspacePolicyRequest::InspectPublicPolicy => {
                    WorkspacePolicyResponse::PublicPolicy {
                        onboarding_mode: WorkspaceOnboardingModeDto::Reject,
                    }
                }
                WorkspacePolicyRequest::ResolveRegistration {
                    email,
                    invitation_token,
                    ..
                } => {
                    assert_eq!(email, "new-external@example.com");
                    assert!(invitation_token.is_none());
                    self.0.fetch_add(1, Ordering::SeqCst);
                    WorkspacePolicyResponse::RegistrationDecision {
                        decision: WorkspaceRegistrationDecisionDto::Deny {
                            reason_code: "registration_rejected".into(),
                        },
                    }
                }
                _ => panic!("unexpected workspace policy request"),
            }))
        }
    }

    struct SessionWrites {
        inner: Arc<dyn SessionStore>,
        puts: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl SessionStore for SessionWrites {
        async fn put(&self, record: domain::SessionRecord) -> anyhow::Result<()> {
            self.puts.fetch_add(1, Ordering::SeqCst);
            self.inner.put(record).await
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

    async fn signup_counts(store: &MainDurableStore) -> (i64, i64) {
        sqlx::query_as(
            "select (select count(*) from users), (select count(*) from workspace_memberships)",
        )
        .fetch_one(store.pool())
        .await
        .unwrap()
    }

    let (initial, _host, entry, _) =
        api_server::test_support::external_auth_verifier::installed_verifier_fixture().await;
    let mut state = (*initial).clone();
    let policy = Arc::new(RejectRegistration::default());
    state.provider_runtime = Arc::new(
        state
            .provider_runtime
            .as_ref()
            .clone()
            .with_workspace_policy_fixture(policy.clone()),
    );
    let sessions = Arc::new(SessionWrites {
        inner: state.session_store.clone(),
        puts: AtomicUsize::new(0),
    });
    state.session_store = sessions.clone();
    let state = Arc::new(state);
    let app = crate::app_with_state_and_config(state.clone(), &default_test_config());
    let before = signup_counts(&state.store).await;

    let start = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/public/auth/external/start?login_entry_id={}",
                    entry.id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::SEE_OTHER);
    let location = start.headers()["location"].to_str().unwrap();
    assert!(
        location.starts_with("https://provider.example/"),
        "{location}"
    );
    let provider_url = url::Url::parse(location).unwrap();
    let transaction_state = provider_url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1;
    let transaction_cookie = start.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("state", &transaction_state)
        .append_pair("code", "new_external_identity")
        .finish();
    let callback = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/public/auth/external/callback?{query}"))
                .header("cookie", transaction_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(callback.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        callback.headers()["location"],
        "/sign-in?auth_error=registration_disabled"
    );
    let session_prefix = format!("{}=", state.cookie_name);
    assert!(
        callback
            .headers()
            .get_all("set-cookie")
            .iter()
            .all(|header| { !header.to_str().unwrap().starts_with(&session_prefix) })
    );
    assert_eq!(policy.0.load(Ordering::SeqCst), 1);
    assert_eq!(sessions.puts.load(Ordering::SeqCst), 0);
    assert_eq!(signup_counts(&state.store).await, before);
}
