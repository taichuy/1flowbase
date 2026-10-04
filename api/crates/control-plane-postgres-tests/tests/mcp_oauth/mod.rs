//! Narrow service + real PostgreSQL lifecycle evidence; HTTP middleware is covered separately.
#[path = "../support/mod.rs"]
mod support;
use control_plane::{
    auth::{
        ApiKeyService, CreateUserApiKeyCommand, RevokeUserApiKeyCommand, UserApiKeyExpirationPolicy,
    },
    mcp_management::{CreateMcpInstanceCommand, McpManagementService},
    mcp_oauth::{
        self as oauth, AuthorizationRequest, Client, McpOAuthService, Registration, TokenRequest,
        TokenResponse,
    },
};
use control_plane_contracts::ports::{
    mcp_oauth::McpOAuthRepository, CreateMcpInstanceInput, CreateMemberInput,
    McpManagementRepository, MemberRepository, RoleRepository,
};
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

const ISSUER: &str = "https://oauth.example.test";
const CALLBACK: &str = "https://chatgpt.com/connector_platform_oauth_redirect";
// RFC7636 Appendix B vector; the verifier has valid length and syntax.
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const INSTANCE: &str = "team?with#reserved/name";
const BROWSER: &str = "browser-secret-for-real-service-test";
const PERMISSION: &str = "mcp_management.view.all";

struct Fixture {
    store: PgControlPlaneStore,
    service: McpOAuthService<PgControlPlaneStore>,
    root: Uuid,
    user: Uuid,
    workspace: domain::WorkspaceRecord,
    key: String,
    key_id: Uuid,
    client: Client,
}
impl Fixture {
    async fn new() -> Self {
        let (store, workspace, root) = support::seed_store().await;
        McpManagementService::new(store.clone())
            .create_instance(CreateMcpInstanceCommand {
                actor_user_id: root.id,
                instance_id: INSTANCE.into(),
                name: "Original workspace MCP".into(),
                description_short: None,
                status: domain::McpInstanceStatus::Enabled,
                default_entry_path: "/".into(),
                webmcp_exposure: domain::WebMcpExposure::Disabled,
            })
            .await
            .unwrap();
        let user = store
            .create_member_with_default_role(&CreateMemberInput {
                actor_user_id: root.id,
                workspace_id: workspace.id,
                account: "oauth-member".into(),
                email: "oauth@example.com".into(),
                phone: None,
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$test$test".into(),
                name: "OAuth Member".into(),
                nickname: "OAuth Member".into(),
                introduction: String::new(),
                email_login_enabled: true,
                phone_login_enabled: false,
            })
            .await
            .unwrap();
        RoleRepository::replace_role_permissions(
            &store,
            root.id,
            workspace.id,
            "member",
            &[PERMISSION.into()],
        )
        .await
        .unwrap();
        let key = ApiKeyService::new(store.clone())
            .create_user_api_key(CreateUserApiKeyCommand {
                actor_user_id: user.id,
                tenant_id: workspace.tenant_id,
                current_workspace_id: workspace.id,
                name: "OAuth original key".into(),
                role_code: "member".into(),
                expiration_policy: UserApiKeyExpirationPolicy::Never,
            })
            .await
            .unwrap();
        let service = McpOAuthService::new(store.clone(), ISSUER.into());
        let client = service.register(registration(CALLBACK)).await.unwrap();
        Self {
            store,
            service,
            root: root.id,
            user: user.id,
            workspace,
            key: key.token,
            key_id: key.api_key.id,
            client,
        }
    }
    fn request(&self) -> AuthorizationRequest {
        AuthorizationRequest {
            client_id: self.client.client_id.clone(),
            redirect_uri: CALLBACK.into(),
            response_type: "code".into(),
            code_challenge: CHALLENGE.into(),
            code_challenge_method: "S256".into(),
            state: "caller-state".into(),
            resource: oauth::resource_url(ISSUER, INSTANCE).unwrap(),
            scope: Some(oauth::SCOPE.into()),
        }
    }
    async fn code(&self) -> String {
        let id = self.service.begin(self.request(), BROWSER).await.unwrap();
        let verified = self.service.verify(&id, BROWSER, &self.key).await.unwrap();
        assert_eq!(verified.workspace_name, self.workspace.name);
        assert_eq!(verified.instance_name, "Original workspace MCP");
        let redirect = self
            .service
            .decision(&id, BROWSER, Some(&verified.approval_token), true)
            .await
            .unwrap();
        assert!(redirect.contains("state=caller-state"));
        redirect
            .split('?')
            .nth(1)
            .unwrap()
            .split('&')
            .find_map(|p| p.strip_prefix("code="))
            .unwrap()
            .to_owned()
    }
    fn exchange(&self, code: &str) -> TokenRequest {
        TokenRequest {
            grant_type: "authorization_code".into(),
            client_id: self.client.client_id.clone(),
            code: Some(code.into()),
            redirect_uri: Some(CALLBACK.into()),
            code_verifier: Some(VERIFIER.into()),
            refresh_token: None,
            resource: Some(oauth::resource_url(ISSUER, INSTANCE).unwrap()),
        }
    }
    fn refresh(&self, token: &str) -> TokenRequest {
        TokenRequest {
            grant_type: "refresh_token".into(),
            client_id: self.client.client_id.clone(),
            code: None,
            redirect_uri: None,
            code_verifier: None,
            refresh_token: Some(token.into()),
            resource: Some(oauth::resource_url(ISSUER, INSTANCE).unwrap()),
        }
    }
    async fn tokens(&self) -> TokenResponse {
        self.service
            .token(self.exchange(&self.code().await))
            .await
            .unwrap()
    }
    async fn rejected(&self, token: &str) {
        let error = self
            .service
            .authenticate(token, INSTANCE)
            .await
            .expect_err("grant must be rejected");
        assert_eq!(error.error, "invalid_grant");
    }
}
fn registration(callback: &str) -> Registration {
    Registration {
        redirect_uris: vec![callback.into()],
        client_name: Some("ChatGPT".into()),
        token_endpoint_auth_method: Some("none".into()),
        grant_types: Some(vec!["authorization_code".into(), "refresh_token".into()]),
        response_types: Some(vec!["code".into()]),
    }
}

#[tokio::test]
async fn pkce_code_binding_expiry_and_concurrent_consumption() {
    let f = Fixture::new().await;
    let code = f.code().await;
    let other = f
        .service
        .register(registration(
            "https://chatgpt.com/connector/oauth/other-client",
        ))
        .await
        .unwrap();
    let mut wrong = f.exchange(&code);
    wrong.code_verifier =
        Some("0123456789012345678901234567890123456789012345678901234567890123".into());
    assert_eq!(
        f.service.token(wrong).await.err().unwrap().error,
        "invalid_grant"
    );
    let mut wrong = f.exchange(&code);
    wrong.client_id = other.client_id;
    assert_eq!(
        f.service.token(wrong).await.err().unwrap().error,
        "invalid_grant"
    );
    let mut wrong = f.exchange(&code);
    wrong.resource = Some(oauth::resource_url(ISSUER, "another").unwrap());
    assert_eq!(
        f.service.token(wrong).await.err().unwrap().error,
        "invalid_grant"
    );
    let mut wrong = f.exchange(&code);
    wrong.redirect_uri = Some("https://chatgpt.com/connector/oauth/other-client".into());
    assert_eq!(
        f.service.token(wrong).await.err().unwrap().error,
        "invalid_grant"
    );
    let mut wrong = f.exchange(&code);
    wrong.code = Some("unknown-code".into());
    assert_eq!(
        f.service.token(wrong).await.err().unwrap().error,
        "invalid_grant"
    );
    // Negative exchanges leave the original bound code usable exactly once.
    let (a, b) = tokio::join!(
        f.service.token(f.exchange(&code)),
        f.service.token(f.exchange(&code))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let tokens = match (a, b) {
        (Ok(tokens), Err(error)) | (Err(error), Ok(tokens)) => {
            assert_eq!(error.error, "invalid_grant");
            tokens
        }
        _ => unreachable!(),
    };
    let actor = f
        .service
        .authenticate(&tokens.access_token, INSTANCE)
        .await
        .unwrap();
    assert_eq!(actor.actor.current_workspace_id, f.workspace.id);
    assert_eq!(actor.actor.user_id, f.user);
    let code = f.code().await;
    sqlx::query("update mcp_oauth_state set expires_at=now()-interval '1 second' where kind='code' and token_hash=$1")
        .bind(oauth::hash(&code)).execute(f.store.pool()).await.unwrap();
    assert_eq!(
        f.service
            .token(f.exchange(&code))
            .await
            .err()
            .unwrap()
            .error,
        "invalid_grant"
    );
}

#[tokio::test]
async fn refresh_rotation_replay_and_concurrent_reuse_revoke_family() {
    let f = Fixture::new().await;
    let tokens = f.tokens().await;
    let mut wrong = f.refresh(&tokens.refresh_token);
    wrong.resource = Some(oauth::resource_url(ISSUER, "other").unwrap());
    assert_eq!(
        f.service.token(wrong).await.err().unwrap().error,
        "invalid_grant"
    );
    let next = f
        .service
        .token(f.refresh(&tokens.refresh_token))
        .await
        .unwrap();
    assert_ne!(tokens.refresh_token, next.refresh_token);
    f.service
        .authenticate(&next.access_token, INSTANCE)
        .await
        .unwrap();
    assert_eq!(
        f.service
            .token(f.refresh(&tokens.refresh_token))
            .await
            .err()
            .unwrap()
            .error,
        "invalid_grant"
    );
    f.rejected(&tokens.access_token).await;
    f.rejected(&next.access_token).await;
    assert_eq!(
        f.service
            .token(f.refresh(&next.refresh_token))
            .await
            .err()
            .unwrap()
            .error,
        "invalid_grant"
    );
    let tokens = f.tokens().await;
    let (a, b) = tokio::join!(
        f.service.token(f.refresh(&tokens.refresh_token)),
        f.service.token(f.refresh(&tokens.refresh_token))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let winner = match (a, b) {
        (Ok(tokens), Err(error)) | (Err(error), Ok(tokens)) => {
            assert_eq!(error.error, "invalid_grant");
            tokens
        }
        _ => unreachable!(),
    };
    f.rejected(&winner.access_token).await;
    assert_eq!(
        f.service
            .token(f.refresh(&winner.refresh_token))
            .await
            .err()
            .unwrap()
            .error,
        "invalid_grant"
    );
}

#[tokio::test]
async fn grants_revalidate_key_expiry_revocation_disabled_user_and_real_permission_removal() {
    let f = Fixture::new().await;
    let tokens = f.tokens().await;
    sqlx::query("update api_keys set expires_at=now()-interval '1 second' where id=$1")
        .bind(f.key_id)
        .execute(f.store.pool())
        .await
        .unwrap();
    f.rejected(&tokens.access_token).await;
    sqlx::query("update api_keys set expires_at=null where id=$1")
        .bind(f.key_id)
        .execute(f.store.pool())
        .await
        .unwrap();
    f.rejected(&tokens.access_token).await; // Reinstating a key cannot revive a revoked grant.
    let tokens = f.tokens().await;
    MemberRepository::disable_member(&f.store, f.root, f.user)
        .await
        .unwrap();
    f.rejected(&tokens.access_token).await;
    MemberRepository::enable_member(&f.store, f.root, f.user)
        .await
        .unwrap();
    let tokens = f.tokens().await;
    assert!(ApiKeyService::new(f.store.clone())
        .authenticate_user_api_key(&f.key)
        .await
        .unwrap()
        .actor
        .permissions
        .contains(PERMISSION));
    RoleRepository::replace_role_permissions(&f.store, f.root, f.workspace.id, "member", &[])
        .await
        .unwrap();
    assert!(!ApiKeyService::new(f.store.clone())
        .authenticate_user_api_key(&f.key)
        .await
        .unwrap()
        .actor
        .permissions
        .contains(PERMISSION));
    f.rejected(&tokens.access_token).await;
    let tokens = f.tokens().await;
    ApiKeyService::new(f.store.clone())
        .revoke_user_api_key(RevokeUserApiKeyCommand {
            actor_user_id: f.user,
            tenant_id: f.workspace.tenant_id,
            current_workspace_id: f.workspace.id,
            api_key_id: f.key_id,
        })
        .await
        .unwrap();
    assert_eq!(
        f.service
            .token(f.refresh(&tokens.refresh_token))
            .await
            .err()
            .unwrap()
            .error,
        "invalid_grant"
    );
    f.rejected(&tokens.access_token).await;
}

#[tokio::test]
async fn workspace_and_instance_binding_do_not_follow_another_namespace_or_role() {
    let f = Fixture::new().await;
    let secondary = f
        .store
        .upsert_workspace(f.workspace.tenant_id, "Other namespace")
        .await
        .unwrap();
    f.store
        .create_mcp_instance(&CreateMcpInstanceInput {
            id: Uuid::now_v7(),
            actor_user_id: f.root,
            workspace_id: secondary.id,
            instance_id: INSTANCE.into(),
            name: "Other namespace MCP".into(),
            description_short: None,
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
            webmcp_exposure: domain::WebMcpExposure::Disabled,
        })
        .await
        .unwrap();
    let tokens = f.tokens().await;
    assert_eq!(
        f.service
            .authenticate(&tokens.access_token, INSTANCE)
            .await
            .unwrap()
            .actor
            .current_workspace_id,
        f.workspace.id
    );
    assert_eq!(
        f.service
            .authenticate(&tokens.access_token, "other-instance")
            .await
            .err()
            .unwrap()
            .error,
        "invalid_grant"
    );
    sqlx::query("update api_keys set scope_id=$1 where id=$2")
        .bind(secondary.id)
        .bind(f.key_id)
        .execute(f.store.pool())
        .await
        .unwrap();
    f.rejected(&tokens.access_token).await;
    sqlx::query("update api_keys set scope_id=$1 where id=$2")
        .bind(f.workspace.id)
        .bind(f.key_id)
        .execute(f.store.pool())
        .await
        .unwrap();
    let tokens = f.tokens().await;
    sqlx::query("update api_keys set role_code='root' where id=$1")
        .bind(f.key_id)
        .execute(f.store.pool())
        .await
        .unwrap();
    f.rejected(&tokens.access_token).await;
    sqlx::query("update api_keys set role_code='member' where id=$1")
        .bind(f.key_id)
        .execute(f.store.pool())
        .await
        .unwrap();
    let tokens = f.tokens().await;
    sqlx::query(
        "update mcp_instances set status='disabled' where workspace_id=$1 and instance_id=$2",
    )
    .bind(f.workspace.id)
    .bind(INSTANCE)
    .execute(f.store.pool())
    .await
    .unwrap();
    f.rejected(&tokens.access_token).await;
}

#[tokio::test]
async fn durable_client_registration_survives_expiry_cleanup_and_later_reauthorization() {
    let f = Fixture::new().await;
    let expiry: Option<time::OffsetDateTime> = sqlx::query_scalar(
        "select expires_at from mcp_oauth_state where kind='client' and token_hash=$1",
    )
    .bind(oauth::hash(&f.client.client_id))
    .fetch_one(f.store.pool())
    .await
    .unwrap();
    assert_eq!(
        expiry, None,
        "registration must not have a fixed connection-breaking TTL"
    );
    // Run the exact expiry predicate at a future horizon: clients survive, short-lived state does not.
    f.store
        .oauth_put(
            "request",
            "expired-fixture",
            serde_json::json!({}),
            Some(oauth::now() - 1),
        )
        .await
        .unwrap();
    sqlx::query("delete from mcp_oauth_state where expires_at<=now()+interval '10 years'")
        .execute(f.store.pool())
        .await
        .unwrap();
    let retained = f
        .store
        .oauth_get("client", &oauth::hash(&f.client.client_id))
        .await
        .unwrap();
    assert!(retained.is_some());
    assert!(f
        .store
        .oauth_get("request", "expired-fixture")
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        f.service
            .register(registration(CALLBACK))
            .await
            .unwrap()
            .client_id,
        f.client.client_id
    );
    let count: i64 = sqlx::query_scalar("select count(*) from mcp_oauth_state where kind='client'")
        .fetch_one(f.store.pool())
        .await
        .unwrap();
    assert_eq!(
        count, 1,
        "repeated registration of the callback set is idempotent"
    );
    let tokens = f.tokens().await;
    let next = f
        .service
        .token(f.refresh(&tokens.refresh_token))
        .await
        .unwrap();
    f.service
        .authenticate(&next.access_token, INSTANCE)
        .await
        .unwrap();
}

#[test]
fn metadata_challenge_resource_encoding_handles_reserved_and_negative_uri_cases() {
    for instance in [
        "team?x",
        "team#x",
        "team/name",
        "team\"name",
        "space name",
        "a+b",
        "team&other",
        "工作区",
    ] {
        let resource = oauth::resource_url(ISSUER, instance).unwrap();
        let metadata = oauth::resource_metadata_url(ISSUER, instance).unwrap();
        assert_eq!(
            metadata,
            format!(
                "{ISSUER}/.well-known/oauth-protected-resource{}",
                &resource[ISSUER.len()..]
            )
        );
        assert!(!metadata.contains(['?', '#', '"', ' ']));
        assert_eq!(metadata.rsplit('/').next(), resource.rsplit('/').next());
    }
    assert_eq!(
        oauth::resource_metadata_url(ISSUER, "team/name").unwrap(),
        format!("{ISSUER}/.well-known/oauth-protected-resource/api/mcp/team%2Fname")
    );
    for instance in ["", ".", ".."] {
        assert!(oauth::resource_metadata_url(ISSUER, instance).is_err());
    }
}
