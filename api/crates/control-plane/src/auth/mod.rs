use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use async_trait::async_trait;
use domain::{ActorContext, SessionRecord, UserStatus};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{ApiKeyRepository, AuthRepository, CreateApiKeyInput, SessionStore},
};

pub struct LoginCommand {
    pub authenticator: String,
    pub identifier: String,
    pub password: String,
}

pub struct LoginResult {
    pub actor: ActorContext,
    pub session: SessionRecord,
}

pub struct AuthenticatorAuthentication {
    pub user: domain::UserRecord,
    pub external_identity_claim: Option<domain::ExternalIdentityClaim>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserApiKeyExpirationPolicy {
    ThirtyDays,
    OneYear,
    ThreeYears,
    Never,
}

impl UserApiKeyExpirationPolicy {
    pub fn expires_at(self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        match self {
            Self::ThirtyDays => Some(now + time::Duration::days(30)),
            Self::OneYear => Some(now + time::Duration::days(365)),
            Self::ThreeYears => Some(now + time::Duration::days(365 * 3)),
            Self::Never => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateUserApiKeyCommand {
    pub actor_user_id: Uuid,
    pub tenant_id: Uuid,
    pub current_workspace_id: Uuid,
    pub name: String,
    pub role_code: String,
    pub expiration_policy: UserApiKeyExpirationPolicy,
}

#[derive(Debug, Clone)]
pub struct ListUserApiKeysCommand {
    pub actor_user_id: Uuid,
    pub tenant_id: Uuid,
    pub current_workspace_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct RevokeUserApiKeyCommand {
    pub actor_user_id: Uuid,
    pub tenant_id: Uuid,
    pub current_workspace_id: Uuid,
    pub api_key_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateUserApiKeyResult {
    pub api_key: domain::ApiKeyRecord,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct ApiKeyActor {
    pub api_key: domain::ApiKeyRecord,
    pub actor: ActorContext,
}

#[derive(Debug, Clone)]
pub struct UserApiKeyActor {
    pub api_key: domain::ApiKeyRecord,
    pub user: domain::UserRecord,
    pub actor: ActorContext,
}

#[async_trait]
pub trait AuthenticatorProvider: Send + Sync {
    fn auth_type(&self) -> &'static str;
    async fn authenticate(
        &self,
        authenticator: &domain::AuthenticatorRecord,
        identifier: &str,
        password: &str,
        repository: &dyn AuthRepository,
    ) -> Result<AuthenticatorAuthentication>;
}

pub struct PasswordLocalAuthenticator;

#[async_trait]
impl AuthenticatorProvider for PasswordLocalAuthenticator {
    fn auth_type(&self) -> &'static str {
        "password-local"
    }

    async fn authenticate(
        &self,
        authenticator: &domain::AuthenticatorRecord,
        identifier: &str,
        password: &str,
        repository: &dyn AuthRepository,
    ) -> Result<AuthenticatorAuthentication> {
        let user = repository
            .find_user_for_password_login(&authenticator.name, identifier)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?;
        let parsed = PasswordHash::new(&user.password_hash)
            .map_err(|_| ControlPlaneError::NotAuthenticated)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| ControlPlaneError::NotAuthenticated)?;
        Ok(AuthenticatorAuthentication {
            user,
            external_identity_claim: None,
        })
    }
}

pub struct AuthenticatorRegistry {
    providers: HashMap<String, Arc<dyn AuthenticatorProvider>>,
}

impl AuthenticatorRegistry {
    pub fn new() -> Self {
        let password_provider: Arc<dyn AuthenticatorProvider> =
            Arc::new(PasswordLocalAuthenticator);
        Self::from_providers(vec![password_provider])
    }

    pub fn from_providers(providers: Vec<Arc<dyn AuthenticatorProvider>>) -> Self {
        let providers = providers
            .into_iter()
            .map(|provider| (provider.auth_type().to_string(), provider))
            .collect();
        Self { providers }
    }

    pub fn provider(&self, auth_type: &str) -> Option<Arc<dyn AuthenticatorProvider>> {
        self.providers.get(auth_type).cloned()
    }

    pub fn supported_auth_types(&self) -> Vec<String> {
        let mut auth_types = self.providers.keys().cloned().collect::<Vec<_>>();
        auth_types.sort();
        auth_types
    }
}

impl Default for AuthenticatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ApiKeyService<R> {
    repository: R,
}

const API_KEY_SECRET_ALPHABET: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const API_KEY_SHORT_ID_LEN: usize = 12;
const API_KEY_SECRET_LEN: usize = 40;
const API_KEY_SECRET_ALPHABET_LEN: u8 = 62;

fn generate_user_api_key_token(key_id: Uuid) -> (String, String) {
    let key_id_hex = key_id.simple().to_string();
    let token_prefix = format!("pat_{}", &key_id_hex[..API_KEY_SHORT_ID_LEN]);
    let mut secret = String::with_capacity(API_KEY_SECRET_LEN);
    let unbiased_limit = u8::MAX - (u8::MAX % API_KEY_SECRET_ALPHABET_LEN);

    while secret.len() < API_KEY_SECRET_LEN {
        let random = OsRng.next_u32() as u8;
        if random >= unbiased_limit {
            continue;
        }
        let index = usize::from(random % API_KEY_SECRET_ALPHABET_LEN);
        secret.push(API_KEY_SECRET_ALPHABET[index] as char);
    }

    let token = format!("{token_prefix}_{secret}");
    (token_prefix, token)
}

impl<R> ApiKeyService<R>
where
    R: AuthRepository + ApiKeyRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn create_user_api_key(
        &self,
        command: CreateUserApiKeyCommand,
    ) -> Result<CreateUserApiKeyResult> {
        let actor = self
            .repository
            .load_actor_context(
                command.actor_user_id,
                command.tenant_id,
                command.current_workspace_id,
                None,
            )
            .await?;
        let key_id = Uuid::now_v7();
        let (token_prefix, token) = generate_user_api_key_token(key_id);
        let api_key = self
            .repository
            .create_api_key(&CreateApiKeyInput {
                id: key_id,
                name: command.name,
                token_hash: hash_api_key_token(&token),
                token_prefix,
                key_kind: domain::ApiKeyKind::UserApiKey,
                application_id: None,
                role_code: Some(command.role_code),
                creator_user_id: command.actor_user_id,
                tenant_id: actor.tenant_id,
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: actor.current_workspace_id,
                enabled: true,
                expires_at: command
                    .expiration_policy
                    .expires_at(OffsetDateTime::now_utc()),
            })
            .await?;

        Ok(CreateUserApiKeyResult { api_key, token })
    }

    pub async fn list_user_api_keys(
        &self,
        command: ListUserApiKeysCommand,
    ) -> Result<Vec<domain::ApiKeyRecord>> {
        let actor = self
            .repository
            .load_actor_context(
                command.actor_user_id,
                command.tenant_id,
                command.current_workspace_id,
                None,
            )
            .await?;
        self.repository
            .list_user_api_keys(actor.user_id, actor.tenant_id, actor.current_workspace_id)
            .await
    }

    pub async fn revoke_user_api_key(&self, command: RevokeUserApiKeyCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context(
                command.actor_user_id,
                command.tenant_id,
                command.current_workspace_id,
                None,
            )
            .await?;
        self.repository
            .revoke_user_api_key(
                command.api_key_id,
                actor.user_id,
                actor.tenant_id,
                actor.current_workspace_id,
            )
            .await
    }

    pub async fn authenticate_user_api_key(&self, token: &str) -> Result<UserApiKeyActor> {
        if !token.starts_with("pat_") {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }
        let token_hash = hash_api_key_token(token);
        let api_key = self
            .repository
            .find_api_key_by_token_hash(&token_hash)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?;
        if api_key.key_kind != domain::ApiKeyKind::UserApiKey
            || api_key.application_id.is_some()
            || !api_key.enabled
            || api_key
                .expires_at
                .is_some_and(|expires_at| expires_at <= OffsetDateTime::now_utc())
        {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }
        let user = self
            .repository
            .find_user_by_id(api_key.creator_user_id)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?;
        if matches!(user.status, UserStatus::Disabled) {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }
        let actor = self
            .repository
            .load_actor_context_for_bound_role(
                user.id,
                api_key.tenant_id,
                api_key.scope_id,
                api_key
                    .role_code
                    .as_deref()
                    .or(user.default_display_role.as_deref())
                    .unwrap_or("manager"),
            )
            .await?;
        self.repository.mark_api_key_used(api_key.id).await?;

        Ok(UserApiKeyActor {
            api_key,
            user,
            actor,
        })
    }

    pub async fn authenticate_bearer_token(&self, token: &str) -> Result<ApiKeyActor> {
        if token.starts_with("pat_") {
            let user_api_key = self.authenticate_user_api_key(token).await?;
            return Ok(ApiKeyActor {
                api_key: user_api_key.api_key,
                actor: user_api_key.actor,
            });
        }
        Err(ControlPlaneError::NotAuthenticated.into())
    }
}

pub fn hash_api_key_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub struct SessionIssuer<S> {
    store: S,
    ttl_days: i64,
}

impl<S> SessionIssuer<S>
where
    S: SessionStore,
{
    pub fn new(store: S, ttl_days: i64) -> Self {
        Self { store, ttl_days }
    }

    pub async fn issue(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        current_workspace_id: Uuid,
        session_version: i64,
    ) -> Result<SessionRecord> {
        let session = SessionRecord {
            session_id: Uuid::now_v7().to_string(),
            user_id,
            tenant_id,
            current_workspace_id,
            session_version,
            csrf_token: Uuid::now_v7().to_string(),
            expires_at_unix: (OffsetDateTime::now_utc() + time::Duration::days(self.ttl_days))
                .unix_timestamp(),
        };
        self.store.put(session.clone()).await?;
        Ok(session)
    }
}

pub struct AuthKernel<R, S> {
    repository: R,
    registry: AuthenticatorRegistry,
    issuer: SessionIssuer<S>,
}

impl<R, S> AuthKernel<R, S>
where
    R: AuthRepository,
    S: SessionStore,
{
    pub fn new(repository: R, issuer: SessionIssuer<S>) -> Self {
        Self::with_registry(repository, issuer, AuthenticatorRegistry::new())
    }

    pub fn with_registry(
        repository: R,
        issuer: SessionIssuer<S>,
        registry: AuthenticatorRegistry,
    ) -> Self {
        Self {
            repository,
            registry,
            issuer,
        }
    }

    pub async fn login(&self, command: LoginCommand) -> Result<LoginResult> {
        let authenticator = self
            .repository
            .find_authenticator(&command.authenticator)
            .await?
            .ok_or(ControlPlaneError::NotFound("authenticator"))?;
        if !authenticator.enabled {
            return Err(ControlPlaneError::PermissionDenied("authenticator_disabled").into());
        }

        let provider = self
            .registry
            .provider(&authenticator.auth_type)
            .ok_or(ControlPlaneError::NotFound("auth_provider"))?;
        let authentication = provider
            .authenticate(
                &authenticator,
                &command.identifier,
                &command.password,
                &self.repository,
            )
            .await?;
        if authentication
            .external_identity_claim
            .as_ref()
            .is_some_and(|claim| claim.authenticator_name != authenticator.name)
        {
            return Err(ControlPlaneError::NotAuthenticated.into());
        }

        let user = authentication.user;
        if matches!(user.status, UserStatus::Disabled) {
            return Err(ControlPlaneError::PermissionDenied("user_disabled").into());
        }

        let scope = self.repository.default_scope_for_user(user.id).await?;
        let actor = self
            .repository
            .load_actor_context(
                user.id,
                scope.tenant_id,
                scope.workspace_id,
                user.default_display_role.as_deref(),
            )
            .await?;
        let session = self
            .issuer
            .issue(
                user.id,
                scope.tenant_id,
                scope.workspace_id,
                user.session_version,
            )
            .await?;

        Ok(LoginResult { actor, session })
    }
}
