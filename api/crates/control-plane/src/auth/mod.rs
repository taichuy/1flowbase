use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use async_trait::async_trait;
use domain::{ActorContext, SessionRecord, UserStatus};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

pub mod public_ui;
pub mod settings;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        ApiKeyRepository, AuthRepository, CreateApiKeyInput, CreateSelfRegisteredMemberInput,
        SelfRegistrationRepository, SessionStore,
    },
};

pub struct LoginCommand {
    pub authenticator_id: Uuid,
    pub identifier: String,
    pub password: String,
}

pub struct SignUpCommand {
    pub authenticator_id: Uuid,
    pub account: String,
    pub email: String,
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
    fn default_public_ui_block(&self) -> &'static str {
        ""
    }
    fn public_variables(
        &self,
        _authenticator: &domain::AuthenticatorRecord,
    ) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()
    }
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

    fn default_public_ui_block(&self) -> &'static str {
        public_ui::PASSWORD_LOCAL_PUBLIC_UI_BLOCK
    }

    fn public_variables(
        &self,
        authenticator: &domain::AuthenticatorRecord,
    ) -> serde_json::Map<String, serde_json::Value> {
        public_ui::password_local_public_variables(&authenticator.options)
    }

    async fn authenticate(
        &self,
        authenticator: &domain::AuthenticatorRecord,
        identifier: &str,
        password: &str,
        repository: &dyn AuthRepository,
    ) -> Result<AuthenticatorAuthentication> {
        let user = repository
            .find_user_for_password_login(authenticator.id, identifier)
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

#[derive(Debug, Clone)]
pub struct AuthenticatorProviderDefinition {
    pub auth_type: String,
    pub config_schema: serde_json::Value,
    pub default_public_ui_block: String,
    pub public_variable_keys: Vec<String>,
    pub public_variables_schema: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthenticatorContextVariableGroup {
    Configuration,
    Runtime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticatorContextVariableDefinition {
    pub group: AuthenticatorContextVariableGroup,
    pub label: String,
    pub member_path: String,
    pub schema: serde_json::Value,
}

#[derive(Clone)]
pub struct AuthenticatorRegistry {
    providers: HashMap<String, Arc<dyn AuthenticatorProvider>>,
    definitions: HashMap<String, AuthenticatorProviderDefinition>,
}

impl AuthenticatorRegistry {
    pub fn new() -> Self {
        let password_provider: Arc<dyn AuthenticatorProvider> =
            Arc::new(PasswordLocalAuthenticator);
        let mut registry = Self::from_providers(vec![password_provider]);
        let config_schema = public_ui::password_local_config_form_schema();
        let public_variable_keys = vec!["self_registration_enabled".to_string()];
        let public_variables_schema =
            public_variables_schema(&config_schema, &public_variable_keys);
        registry.definitions.insert(
            "password-local".to_string(),
            AuthenticatorProviderDefinition {
                auth_type: "password-local".to_string(),
                config_schema,
                default_public_ui_block: public_ui::PASSWORD_LOCAL_PUBLIC_UI_BLOCK.to_string(),
                public_variable_keys,
                public_variables_schema,
            },
        );
        registry
    }

    pub fn from_providers(providers: Vec<Arc<dyn AuthenticatorProvider>>) -> Self {
        let providers = providers
            .into_iter()
            .map(|provider| (provider.auth_type().to_string(), provider))
            .collect();
        Self {
            providers,
            definitions: HashMap::new(),
        }
    }

    pub fn from_host_extensions(
        host_extensions: &plugin_framework::HostExtensionRegistry,
    ) -> Result<Self> {
        let mut registry = Self::new();
        for (_, provider) in host_extensions.auth_providers() {
            if registry.definitions.contains_key(&provider.auth_type) {
                return Err(ControlPlaneError::Conflict("auth_provider").into());
            }
            let config_schema = serde_json::to_value(&provider.config_schema)?;
            let public_variables_schema =
                public_variables_schema(&config_schema, &provider.public_variable_keys);
            registry.definitions.insert(
                provider.auth_type.clone(),
                AuthenticatorProviderDefinition {
                    auth_type: provider.auth_type.clone(),
                    config_schema,
                    default_public_ui_block: provider.default_public_ui_block.clone(),
                    public_variable_keys: provider.public_variable_keys.clone(),
                    public_variables_schema,
                },
            );
        }
        Ok(registry)
    }

    pub fn provider(&self, auth_type: &str) -> Option<Arc<dyn AuthenticatorProvider>> {
        self.providers.get(auth_type).cloned()
    }

    pub fn supported_auth_types(&self) -> Vec<String> {
        let mut auth_types = self.definitions.keys().cloned().collect::<Vec<_>>();
        auth_types.sort();
        auth_types
    }

    pub fn definition(&self, auth_type: &str) -> Option<&AuthenticatorProviderDefinition> {
        self.definitions.get(auth_type)
    }

    pub fn public_variables(
        &self,
        authenticator: &domain::AuthenticatorRecord,
    ) -> Option<serde_json::Map<String, serde_json::Value>> {
        let definition = self.definition(&authenticator.auth_type);
        let provider_variables = if let Some(provider) = self.provider(&authenticator.auth_type) {
            provider.public_variables(authenticator)
        } else {
            let definition = definition?;
            let config = authenticator
                .options
                .get("extension_config")
                .and_then(serde_json::Value::as_object);
            definition
                .public_variable_keys
                .iter()
                .filter_map(|key| {
                    config
                        .and_then(|values| values.get(key))
                        .cloned()
                        .map(|value| (key.clone(), value))
                })
                .collect()
        };
        let mut variables = public_ui::authenticator_host_public_variables(authenticator);
        let Some(definition) = definition else {
            variables.extend(provider_variables);
            return Some(variables);
        };
        let properties = definition
            .public_variables_schema
            .get("properties")
            .and_then(serde_json::Value::as_object);
        variables.extend(provider_variables.into_iter().filter(|(key, value)| {
            properties
                .and_then(|schemas| schemas.get(key))
                .is_some_and(|schema| json_value_matches_schema_type(value, schema))
        }));
        Some(variables)
    }

    pub fn context_variables(
        &self,
        auth_type: &str,
    ) -> Vec<AuthenticatorContextVariableDefinition> {
        let provider_public_variables_schema = self
            .definition(auth_type)
            .map(|definition| definition.public_variables_schema.clone())
            .unwrap_or_else(|| {
                serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {}
                })
            });
        let host_variable_keys = ["title", "description", "enabled"];
        let host_public_variables_schema = public_variables_schema(
            &public_ui::auth_common_config_form_schema(),
            &host_variable_keys.map(str::to_string),
        );
        let mut variables = host_variable_keys
            .iter()
            .filter_map(|key| {
                host_public_variables_schema
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|properties| properties.get(*key))
                    .map(|schema| AuthenticatorContextVariableDefinition {
                        group: AuthenticatorContextVariableGroup::Configuration,
                        label: schema
                            .get("title")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(key)
                            .to_string(),
                        member_path: format!("inputs.public_variables.{key}"),
                        schema: schema.clone(),
                    })
            })
            .collect::<Vec<_>>();
        if let Some(definition) = self.definition(auth_type) {
            if let Some(properties) = provider_public_variables_schema
                .get("properties")
                .and_then(serde_json::Value::as_object)
            {
                variables.extend(definition.public_variable_keys.iter().filter_map(|key| {
                    properties
                        .get(key)
                        .map(|schema| AuthenticatorContextVariableDefinition {
                            group: AuthenticatorContextVariableGroup::Configuration,
                            label: schema
                                .get("title")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(key)
                                .to_string(),
                            member_path: format!("inputs.public_variables.{key}"),
                            schema: schema.clone(),
                        })
                }));
            }
        }
        variables.extend([
            AuthenticatorContextVariableDefinition {
                group: AuthenticatorContextVariableGroup::Runtime,
                label: "Authenticator ID".to_string(),
                member_path: "inputs.authenticator_id".to_string(),
                schema: serde_json::json!({ "type": "string", "format": "uuid" }),
            },
            AuthenticatorContextVariableDefinition {
                group: AuthenticatorContextVariableGroup::Runtime,
                label: "Authentication event".to_string(),
                member_path: "inputs.auth_event".to_string(),
                schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action_id": { "type": "string" },
                        "values": { "type": "object" },
                        "payload": {}
                    },
                    "required": ["action_id"]
                }),
            },
            AuthenticatorContextVariableDefinition {
                group: AuthenticatorContextVariableGroup::Runtime,
                label: "API".to_string(),
                member_path: "api".to_string(),
                schema: serde_json::json!({ "type": "object" }),
            },
        ]);
        variables
    }
}

fn public_variables_schema(
    config_schema: &serde_json::Value,
    public_variable_keys: &[String],
) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let fields = config_schema
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    for key in public_variable_keys {
        let Some(field) = fields
            .iter()
            .find(|field| field["key"].as_str() == Some(key.as_str()))
        else {
            continue;
        };
        let schema_type = match field["type"].as_str() {
            Some("boolean") => "boolean",
            Some("number") => "number",
            Some("integer") => "integer",
            Some("array") => "array",
            Some("object") => "object",
            _ => "string",
        };
        let mut schema = serde_json::Map::from_iter([
            (
                "type".to_string(),
                serde_json::Value::String(schema_type.to_string()),
            ),
            (
                "title".to_string(),
                field.get("label").cloned().unwrap_or_else(|| {
                    serde_json::Value::String(format!("ctx.inputs.public_variables.{key}"))
                }),
            ),
        ]);
        if let Some(description) = field.get("description").filter(|value| value.is_string()) {
            schema.insert("description".to_string(), description.clone());
        }
        properties.insert(key.clone(), serde_json::Value::Object(schema));
    }
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties
    })
}

fn json_value_matches_schema_type(value: &serde_json::Value, schema: &serde_json::Value) -> bool {
    match schema.get("type").and_then(serde_json::Value::as_str) {
        Some("boolean") => value.is_boolean(),
        Some("number") => value.is_number(),
        Some("integer") => value.as_i64().is_some() || value.as_u64().is_some(),
        Some("array") => value.is_array(),
        Some("object") => value.is_object(),
        Some("string") => value.is_string(),
        _ => false,
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
                    .unwrap_or("member"),
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
            .find_authenticator(command.authenticator_id)
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
            .is_some_and(|claim| claim.authenticator_id != authenticator.id)
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

    pub async fn sign_up(&self, command: SignUpCommand) -> Result<LoginResult>
    where
        R: SelfRegistrationRepository,
    {
        let authenticator = self
            .repository
            .find_authenticator(command.authenticator_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("authenticator"))?;
        if !authenticator.enabled {
            return Err(ControlPlaneError::PermissionDenied("authenticator_disabled").into());
        }
        if authenticator.auth_type != "password-local" {
            return Err(
                ControlPlaneError::PermissionDenied("self_registration_unsupported").into(),
            );
        }
        if !public_ui::password_local_self_registration_enabled(&authenticator.options) {
            return Err(ControlPlaneError::PermissionDenied("self_registration_disabled").into());
        }

        let account = command.account.trim().to_lowercase();
        let email = command.email.trim().to_lowercase();
        validate_self_registration_input(&account, &email, &command.password)?;
        let password_hash = hash_password(&command.password)?;
        let user = self
            .repository
            .create_self_registered_member(&CreateSelfRegisteredMemberInput {
                authenticator_id: authenticator.id,
                account,
                email,
                password_hash,
            })
            .await?;
        let scope = self.repository.default_scope_for_user(user.id).await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(scope.workspace_id),
                None,
                "user",
                Some(user.id),
                "member.self_registered",
                serde_json::json!({ "account": user.account }),
            ))
            .await?;
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

fn validate_self_registration_input(account: &str, email: &str, password: &str) -> Result<()> {
    if account.len() < 3 {
        return Err(ControlPlaneError::InvalidInput("account").into());
    }
    if !email.contains('@') {
        return Err(ControlPlaneError::InvalidInput("email").into());
    }
    if password.len() < 8 {
        return Err(ControlPlaneError::InvalidInput("password").into());
    }
    Ok(())
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| anyhow::anyhow!("failed to hash self-registration password: {error}"))?
        .to_string())
}
