use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    auth::{
        AuthKernel, AuthenticatorAuthentication, AuthenticatorProvider, AuthenticatorRegistry,
        LoginCommand, SessionIssuer,
    },
    errors::ControlPlaneError,
    ports::AuthRepository,
};
use domain::{
    ActorContext, AuditLogRecord, AuthenticatorRecord, BoundRole, ExternalIdentityClaim,
    PermissionDefinition, RoleScopeKind, ScopeContext, UserRecord, UserStatus,
};
use plugin_framework::{
    AuthProviderContributionManifest, HostExtensionBootstrapPhase, HostExtensionRegistry,
    RegisteredHostExtension,
};
use uuid::Uuid;

use crate::_tests::support::{password_hash, MemorySessionStore};

#[derive(Clone)]
struct InstanceAwareAuthRepository {
    authenticators: Arc<HashMap<Uuid, AuthenticatorRecord>>,
    user: UserRecord,
    password_login_calls: Arc<Mutex<Vec<(Uuid, String)>>>,
}

impl InstanceAwareAuthRepository {
    fn new(authenticators: Vec<AuthenticatorRecord>, user: UserRecord) -> Self {
        Self {
            authenticators: Arc::new(
                authenticators
                    .into_iter()
                    .map(|authenticator| (authenticator.id, authenticator))
                    .collect(),
            ),
            user,
            password_login_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn password_login_calls(&self) -> Vec<(Uuid, String)> {
        self.password_login_calls
            .lock()
            .expect("password login calls lock should be free")
            .clone()
    }
}

#[async_trait]
impl AuthRepository for InstanceAwareAuthRepository {
    async fn find_authenticator(&self, id: Uuid) -> Result<Option<AuthenticatorRecord>> {
        Ok(self.authenticators.get(&id).cloned())
    }

    async fn find_user_for_password_login(
        &self,
        authenticator_id: Uuid,
        identifier: &str,
    ) -> Result<Option<UserRecord>> {
        self.password_login_calls
            .lock()
            .expect("password login calls lock should be free")
            .push((authenticator_id, identifier.to_string()));
        Ok(
            (authenticator_id == STAFF_PASSWORD_AUTHENTICATOR_ID && identifier == "root")
                .then(|| self.user.clone()),
        )
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<UserRecord>> {
        Ok((self.user.id == user_id).then(|| self.user.clone()))
    }

    async fn default_scope_for_user(&self, _user_id: Uuid) -> Result<ScopeContext> {
        Ok(ScopeContext {
            tenant_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
        })
    }

    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        self.load_actor_context(actor_user_id, Uuid::nil(), Uuid::nil(), None)
            .await
    }

    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        display_role: Option<&str>,
    ) -> Result<ActorContext> {
        Ok(ActorContext {
            user_id,
            tenant_id,
            current_workspace_id: workspace_id,
            effective_display_role: display_role.unwrap_or("root").to_string(),
            is_root: true,
            permissions: Default::default(),
        })
    }

    async fn update_password_hash(
        &self,
        _user_id: Uuid,
        _password_hash: &str,
        _actor_id: Uuid,
    ) -> Result<i64> {
        anyhow::bail!("update_password_hash should not be called")
    }

    async fn update_profile(
        &self,
        _input: &control_plane::ports::UpdateProfileInput,
    ) -> Result<UserRecord> {
        anyhow::bail!("update_profile should not be called")
    }

    async fn update_user_meta(
        &self,
        _input: &control_plane::ports::UpdateUserMetaInput,
    ) -> Result<UserRecord> {
        anyhow::bail!("update_user_meta should not be called")
    }

    async fn bump_session_version(&self, _user_id: Uuid, _actor_id: Uuid) -> Result<i64> {
        anyhow::bail!("bump_session_version should not be called")
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionDefinition>> {
        Ok(Vec::new())
    }

    async fn append_audit_log(&self, _event: &AuditLogRecord) -> Result<()> {
        Ok(())
    }
}

struct CrossInstanceClaimProvider {
    user: UserRecord,
}

#[async_trait]
impl AuthenticatorProvider for CrossInstanceClaimProvider {
    fn auth_type(&self) -> &'static str {
        "external-test"
    }

    async fn authenticate(
        &self,
        _authenticator: &AuthenticatorRecord,
        _identifier: &str,
        _password: &str,
        _repository: &dyn AuthRepository,
    ) -> Result<AuthenticatorAuthentication> {
        Ok(AuthenticatorAuthentication {
            user: self.user.clone(),
            external_identity_claim: Some(ExternalIdentityClaim {
                authenticator_id: OTHER_EXTERNAL_AUTHENTICATOR_ID,
                subject_type: "email".to_string(),
                subject_value: "root@example.com".to_string(),
                issuer: None,
                realm: None,
                profile: serde_json::json!({}),
                verified: true,
                metadata: serde_json::json!({}),
            }),
        })
    }
}

fn test_user() -> UserRecord {
    UserRecord {
        id: Uuid::now_v7(),
        account: "root".to_string(),
        email: "root@example.com".to_string(),
        phone: None,
        password_hash: password_hash("change-me"),
        name: "Root".to_string(),
        nickname: "Root".to_string(),
        avatar_url: None,
        introduction: String::new(),
        preferred_locale: None,
        meta: serde_json::json!({}),
        default_display_role: Some("root".to_string()),
        email_login_enabled: true,
        phone_login_enabled: false,
        status: UserStatus::Active,
        session_version: 1,
        roles: vec![BoundRole {
            code: "root".to_string(),
            name: "Root".to_string(),
            scope_kind: RoleScopeKind::System,
            workspace_id: None,
        }],
    }
}

const STAFF_PASSWORD_AUTHENTICATOR_ID: Uuid =
    Uuid::from_u128(0x00000000_0000_0000_0000_000000000011);
const PARTNER_EXTERNAL_AUTHENTICATOR_ID: Uuid =
    Uuid::from_u128(0x00000000_0000_0000_0000_000000000012);
const OTHER_EXTERNAL_AUTHENTICATOR_ID: Uuid =
    Uuid::from_u128(0x00000000_0000_0000_0000_000000000013);

fn authenticator(id: Uuid, title: &str, auth_type: &str) -> AuthenticatorRecord {
    AuthenticatorRecord {
        id,
        auth_type: auth_type.to_string(),
        title: title.to_string(),
        enabled: true,
        is_builtin: false,
        sort_order: 0,
        public_ui_block: crate::auth::public_ui::PASSWORD_LOCAL_PUBLIC_UI_BLOCK.to_string(),
        options: serde_json::json!({}),
    }
}

#[test]
fn backend_only_auth_provider_contributes_block_schema_and_public_projection() {
    let mut host_extensions = HostExtensionRegistry::default();
    host_extensions
        .register(RegisteredHostExtension {
            extension_id: "fixture-auth".to_string(),
            bootstrap_phase: HostExtensionBootstrapPhase::Boot,
            provides_contracts: vec![],
            overrides_contracts: vec![],
            registers_slots: vec![],
            registers_storage: vec![],
            infrastructure_providers: vec![],
            auth_providers: vec![AuthProviderContributionManifest {
                auth_type: "fixture-auth.qr".to_string(),
                display_name: "Fixture QR".to_string(),
                config_schema: vec![serde_json::from_value(serde_json::json!({
                    "key": "issuer",
                    "label": "Issuer",
                    "type": "string"
                }))
                .unwrap()],
                default_public_ui_block: "export default { main } satisfies BlockModule;"
                    .to_string(),
                public_variable_keys: vec!["issuer".to_string()],
                public_route_ids: vec!["fixture-auth.qr.start".to_string()],
            }],
            owned_resources: vec![],
            extends_resources: vec![],
            routes: vec!["fixture-auth.qr.start".to_string()],
            workers: vec![],
            migrations: vec![],
        })
        .unwrap();

    let registry = AuthenticatorRegistry::from_host_extensions(&host_extensions).unwrap();
    let definition = registry.definition("fixture-auth.qr").unwrap();
    assert!(definition.default_public_ui_block.contains("BlockModule"));
    assert_eq!(definition.config_schema[0]["key"], "issuer");
    assert_eq!(
        definition.public_variables_schema,
        serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "issuer": {
                    "type": "string",
                    "title": "Issuer"
                }
            }
        })
    );
    assert!(registry
        .context_variables("fixture-auth.qr")
        .iter()
        .any(|variable| {
            variable.label == "Issuer" && variable.member_path == "inputs.public_variables.issuer"
        }));
    assert!(registry
        .context_variables("fixture-auth.qr")
        .iter()
        .any(|variable| {
            variable.member_path == "inputs.authenticator_selection_available"
                && variable.schema == serde_json::json!({ "type": "boolean" })
        }));
    let record = AuthenticatorRecord {
        options: serde_json::json!({
            "extension_config": {
                "issuer": "https://issuer.example.test",
                "client_secret": "must-not-leak"
            }
        }),
        ..authenticator(Uuid::now_v7(), "Fixture QR", "fixture-auth.qr")
    };
    assert_eq!(
        registry.public_variables(&record).unwrap(),
        serde_json::from_value(serde_json::json!({
            "title": "Fixture QR",
            "enabled": true,
            "issuer": "https://issuer.example.test"
        }))
        .unwrap()
    );
}

#[tokio::test]
async fn password_local_provider_reads_identity_from_selected_authenticator_instance() {
    let user = test_user();
    let repository = InstanceAwareAuthRepository::new(
        vec![authenticator(
            STAFF_PASSWORD_AUTHENTICATOR_ID,
            "Staff Password",
            "password-local",
        )],
        user,
    );
    let kernel = AuthKernel::new(
        repository.clone(),
        SessionIssuer::new(MemorySessionStore::default(), 7),
    );

    kernel
        .login(LoginCommand {
            authenticator_id: STAFF_PASSWORD_AUTHENTICATOR_ID,
            identifier: "root".to_string(),
            password: "change-me".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        repository.password_login_calls(),
        vec![(STAFF_PASSWORD_AUTHENTICATOR_ID, "root".to_string())]
    );
}

#[tokio::test]
async fn auth_kernel_rejects_provider_claim_from_another_authenticator_instance() {
    let user = test_user();
    let repository = InstanceAwareAuthRepository::new(
        vec![authenticator(
            PARTNER_EXTERNAL_AUTHENTICATOR_ID,
            "Partner External",
            "external-test",
        )],
        user.clone(),
    );
    let registry =
        AuthenticatorRegistry::from_providers(vec![Arc::new(CrossInstanceClaimProvider { user })]);
    let kernel = AuthKernel::with_registry(
        repository,
        SessionIssuer::new(MemorySessionStore::default(), 7),
        registry,
    );

    let result = kernel
        .login(LoginCommand {
            authenticator_id: PARTNER_EXTERNAL_AUTHENTICATOR_ID,
            identifier: "root@example.com".to_string(),
            password: "ignored".to_string(),
        })
        .await;

    match result {
        Ok(_) => panic!("cross-instance claim should be rejected"),
        Err(error) => assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::NotAuthenticated)
        )),
    }
}
