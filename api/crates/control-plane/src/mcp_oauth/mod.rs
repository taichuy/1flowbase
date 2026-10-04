//! OAuth authorization server for user-key-backed, instance-scoped MCP grants.
use crate::{
    auth::{hash_api_key_token, ApiKeyService, UserApiKeyActor},
    ports::{ApiKeyRepository, AuthRepository, McpManagementRepository, WorkspaceRepository},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use control_plane_contracts::ports::mcp_oauth::{McpOAuthGrant, McpOAuthRepository};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

pub const SCOPE: &str = "mcp:invoke";
const REQUEST_SECONDS: i64 = 600;
const GRANT_SECONDS: i64 = 30 * 86400;
const ACCESS_SECONDS: i64 = 3600;
#[derive(Debug, thiserror::Error)]
#[error("{error}: {description}")]
pub struct OAuthError {
    pub error: &'static str,
    pub description: &'static str,
}
impl OAuthError {
    pub fn invalid() -> Self {
        Self {
            error: "invalid_request",
            description: "The authorization request is invalid or expired.",
        }
    }
    pub fn grant() -> Self {
        Self {
            error: "invalid_grant",
            description: "The credential is invalid, expired, or no longer authorized.",
        }
    }
}
impl From<anyhow::Error> for OAuthError {
    fn from(_: anyhow::Error) -> Self {
        Self {
            error: "server_error",
            description: "The authorization service is unavailable.",
        }
    }
}
impl From<serde_json::Error> for OAuthError {
    fn from(_: serde_json::Error) -> Self {
        Self::invalid()
    }
}
type Result<T> = std::result::Result<T, OAuthError>;
pub fn hash(value: &str) -> String {
    hash_api_key_token(value)
}
pub fn random_token() -> String {
    let mut bytes = [0; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
pub fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

pub fn validate_issuer(value: &str, production: bool) -> std::result::Result<String, String> {
    let url = Url::parse(value).map_err(|_| "invalid MCP OAuth issuer".to_owned())?;
    let loopback = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if !(url.scheme() == "https" || (!production && loopback && url.scheme() == "http"))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "MCP OAuth issuer must be a trusted HTTPS origin (HTTP loopback is development-only)"
                .into(),
        );
    }
    Ok(url.origin().ascii_serialization())
}
pub fn resource_url(issuer: &str, instance: &str) -> Result<String> {
    if instance.trim().is_empty() || instance.len() > 255 || instance == "." || instance == ".." {
        return Err(OAuthError::invalid());
    }
    let mut url = Url::parse(issuer).map_err(|_| OAuthError::invalid())?;
    url.path_segments_mut()
        .map_err(|_| OAuthError::invalid())?
        .extend(["api", "mcp", instance]);
    Ok(url.to_string())
}
fn resource_instance(issuer: &str, resource: &str) -> Result<String> {
    let encoded = resource
        .strip_prefix(&format!("{issuer}/api/mcp/"))
        .ok_or_else(OAuthError::invalid)?;
    // Decode a single RFC3986 path segment; a literal '+' must not become a space.
    let form = format!("instance={}", encoded.replace('+', "%2B"));
    let instance = url::form_urlencoded::parse(form.as_bytes())
        .next()
        .ok_or_else(OAuthError::invalid)?
        .1
        .into_owned();
    if resource_url(issuer, &instance)? != resource {
        return Err(OAuthError::invalid());
    }
    Ok(instance)
}
pub fn allowed_redirect(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https"
        || url.host_str() != Some("chatgpt.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    if value == "https://chatgpt.com/connector_platform_oauth_redirect" {
        return true;
    }
    url.path()
        .strip_prefix("/connector/oauth/")
        .is_some_and(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Client {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub token_endpoint_auth_method: String,
}
#[derive(Deserialize)]
pub struct Registration {
    pub redirect_uris: Vec<String>,
    pub client_name: Option<String>,
    pub token_endpoint_auth_method: Option<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthorizationRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub state: String,
    pub resource: String,
    pub scope: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    request: AuthorizationRequest,
    browser_hash: String,
    instance_id: String,
    client_name: String,
}
#[derive(Serialize, Deserialize)]
struct Approval {
    request_hash: String,
    browser_hash: String,
    grant_id: Uuid,
    token_hash: String,
}
#[derive(Serialize, Deserialize)]
struct Code {
    grant_id: Uuid,
    redirect_uri: String,
    challenge: String,
}
#[derive(Serialize)]
pub struct AuthorizationView {
    pub client_name: String,
    pub instance_id: String,
    pub scope: &'static str,
}
#[derive(Serialize)]
pub struct VerificationView {
    pub approval_token: String,
    pub workspace_name: String,
    pub instance_name: String,
    pub scope: &'static str,
}
#[derive(Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub client_id: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub resource: Option<String>,
}
#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: &'static str,
}

pub struct McpOAuthService<R> {
    repository: R,
    issuer: String,
}
impl<R> McpOAuthService<R>
where
    R: Clone
        + McpOAuthRepository
        + AuthRepository
        + ApiKeyRepository
        + McpManagementRepository
        + WorkspaceRepository,
{
    pub fn new(repository: R, issuer: String) -> Self {
        Self { repository, issuer }
    }
    pub async fn register(&self, input: Registration) -> Result<Client> {
        if input.redirect_uris.is_empty()
            || input.redirect_uris.len() > 8
            || input.redirect_uris.iter().any(|u| !allowed_redirect(u))
            || input
                .token_endpoint_auth_method
                .as_deref()
                .is_some_and(|m| m != "none")
            || input.grant_types.as_ref().is_some_and(|g| {
                g.iter()
                    .any(|s| s != "authorization_code" && s != "refresh_token")
            })
            || input
                .response_types
                .as_ref()
                .is_some_and(|g| g.iter().any(|s| s != "code"))
        {
            return Err(OAuthError {
                error: "invalid_client_metadata",
                description:
                    "Only registered ChatGPT HTTPS callbacks and public PKCE clients are supported.",
            });
        }
        // The untrusted registration name is not an identity assertion.
        let mut redirect_uris = input.redirect_uris;
        redirect_uris.sort();
        redirect_uris.dedup();
        // Public client identity is deterministic for the exact registered callback set.
        let client_id = format!("chatgpt_{}", hash(&serde_json::to_string(&redirect_uris)?));
        let client = Client {
            client_id,
            client_name: "ChatGPT".into(),
            redirect_uris,
            token_endpoint_auth_method: "none".into(),
        };
        self.repository
            .oauth_put(
                "client",
                &hash(&client.client_id),
                serde_json::to_value(&client)?,
                now() + 365 * 86400,
            )
            .await?;
        Ok(client)
    }
    async fn client(&self, id: &str) -> Result<Client> {
        serde_json::from_value(
            self.repository
                .oauth_get("client", &hash(id))
                .await?
                .ok_or(OAuthError {
                    error: "invalid_client",
                    description: "The OAuth client is not registered.",
                })?,
        )
        .map_err(Into::into)
    }
    pub async fn begin(&self, request: AuthorizationRequest, browser: &str) -> Result<String> {
        let client = self.client(&request.client_id).await?;
        if request.response_type != "code"
            || request.code_challenge_method != "S256"
            || request.code_challenge.len() != 43
            || !request
                .code_challenge
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || request.state.is_empty()
            || request.state.len() > 2048
            || request.scope.as_deref().unwrap_or(SCOPE) != SCOPE
            || !client.redirect_uris.contains(&request.redirect_uri)
        {
            return Err(OAuthError::invalid());
        }
        let instance_id = resource_instance(&self.issuer, &request.resource)?;
        let id = random_token();
        let pending = Pending {
            request,
            browser_hash: hash(browser),
            instance_id,
            client_name: client.client_name,
        };
        self.repository
            .oauth_put(
                "request",
                &hash(&id),
                serde_json::to_value(pending)?,
                now() + REQUEST_SECONDS,
            )
            .await?;
        Ok(id)
    }
    async fn pending(&self, id: &str, browser: &str) -> Result<(Value, Pending)> {
        let value = self
            .repository
            .oauth_get("request", &hash(id))
            .await?
            .ok_or_else(OAuthError::invalid)?;
        let pending: Pending = serde_json::from_value(value.clone())?;
        if pending.browser_hash != hash(browser) {
            return Err(OAuthError::invalid());
        }
        Ok((value, pending))
    }
    pub async fn authorization(&self, id: &str, browser: &str) -> Result<AuthorizationView> {
        let (_, p) = self.pending(id, browser).await?;
        Ok(AuthorizationView {
            client_name: p.client_name,
            instance_id: p.instance_id,
            scope: SCOPE,
        })
    }
    pub async fn verify(&self, id: &str, browser: &str, key: &str) -> Result<VerificationView> {
        let (_, p) = self.pending(id, browser).await?;
        let actor = ApiKeyService::new(self.repository.clone())
            .authenticate_user_api_key(key)
            .await
            .map_err(|_| OAuthError::grant())?;
        let instance = self
            .repository
            .get_mcp_instance(actor.actor.current_workspace_id, &p.instance_id)
            .await?
            .filter(|i| i.status == domain::mcp_management::McpInstanceStatus::Enabled)
            .ok_or_else(OAuthError::grant)?;
        let workspace = self
            .repository
            .get_workspace(actor.actor.current_workspace_id)
            .await?
            .ok_or_else(OAuthError::grant)?;
        let grant = McpOAuthGrant {
            id: Uuid::now_v7(),
            user_id: actor.user.id,
            api_key_id: actor.api_key.id,
            key_hash: hash(key),
            role_code: actor.actor.effective_display_role,
            workspace_id: actor.actor.current_workspace_id,
            instance_id: p.instance_id,
            client_id: p.request.client_id,
            resource: p.request.resource,
            expires_at: now() + GRANT_SECONDS,
            authorization_stamp: self
                .repository
                .oauth_authorization_stamp(actor.user.id)
                .await?,
        };
        self.repository.oauth_create_grant(&grant).await?;
        let approval_token = random_token();
        self.repository
            .oauth_put(
                "approval",
                &hash(id),
                serde_json::to_value(Approval {
                    request_hash: hash(id),
                    browser_hash: hash(browser),
                    grant_id: grant.id,
                    token_hash: hash(&approval_token),
                })?,
                now() + REQUEST_SECONDS,
            )
            .await?;
        Ok(VerificationView {
            approval_token,
            workspace_name: workspace.name,
            instance_name: instance.name,
            scope: SCOPE,
        })
    }
    pub async fn decision(
        &self,
        id: &str,
        browser: &str,
        approval: Option<&str>,
        approved: bool,
    ) -> Result<String> {
        let (pending_value, p) = self.pending(id, browser).await?;
        let mut redirect =
            Url::parse(&p.request.redirect_uri).map_err(|_| OAuthError::invalid())?;
        if approved {
            let token = approval.ok_or_else(OAuthError::invalid)?;
            let value = self
                .repository
                .oauth_get("approval", &hash(id))
                .await?
                .ok_or_else(OAuthError::invalid)?;
            let a: Approval = serde_json::from_value(value.clone())?;
            if a.token_hash != hash(token)
                || a.browser_hash != hash(browser)
                || a.request_hash != hash(id)
            {
                return Err(OAuthError::invalid());
            }
            self.validate_grant(a.grant_id).await?;
            if !self
                .repository
                .oauth_consume("approval", &hash(id), &value)
                .await?
                || !self
                    .repository
                    .oauth_consume("request", &hash(id), &pending_value)
                    .await?
            {
                return Err(OAuthError::invalid());
            }
            let code = random_token();
            self.repository
                .oauth_put(
                    "code",
                    &hash(&code),
                    serde_json::to_value(Code {
                        grant_id: a.grant_id,
                        redirect_uri: p.request.redirect_uri.clone(),
                        challenge: p.request.code_challenge,
                    })?,
                    now() + 120,
                )
                .await?;
            redirect.query_pairs_mut().append_pair("code", &code);
        } else {
            if !self
                .repository
                .oauth_consume("request", &hash(id), &pending_value)
                .await?
            {
                return Err(OAuthError::invalid());
            }
            redirect
                .query_pairs_mut()
                .append_pair("error", "access_denied");
        }
        redirect
            .query_pairs_mut()
            .append_pair("state", &p.request.state)
            .append_pair("iss", &self.issuer);
        Ok(redirect.to_string())
    }
    async fn validate_grant(&self, id: Uuid) -> Result<(McpOAuthGrant, UserApiKeyActor)> {
        let g = self
            .repository
            .oauth_grant(id)
            .await?
            .ok_or_else(OAuthError::grant)?;
        let result = ApiKeyService::new(self.repository.clone())
            .authenticate_user_api_key_hash(&g.key_hash)
            .await;
        let Ok(actor) = result else {
            self.repository.oauth_revoke_grant(id).await?;
            return Err(OAuthError::grant());
        };
        if actor.api_key.id != g.api_key_id
            || actor.user.id != g.user_id
            || actor.actor.current_workspace_id != g.workspace_id
            || actor.actor.effective_display_role != g.role_code
            || self.repository.oauth_authorization_stamp(g.user_id).await? != g.authorization_stamp
            || !self
                .repository
                .get_mcp_instance(g.workspace_id, &g.instance_id)
                .await?
                .is_some_and(|i| i.status == domain::mcp_management::McpInstanceStatus::Enabled)
        {
            self.repository.oauth_revoke_grant(id).await?;
            return Err(OAuthError::grant());
        }
        Ok((g, actor))
    }
    pub async fn token(&self, input: TokenRequest) -> Result<TokenResponse> {
        self.client(&input.client_id).await?;
        let (grant, old) = match input.grant_type.as_str() {
            "authorization_code" => {
                let code = input.code.as_deref().ok_or_else(OAuthError::grant)?;
                let value = self
                    .repository
                    .oauth_get("code", &hash(code))
                    .await?
                    .ok_or_else(OAuthError::grant)?;
                let c: Code = serde_json::from_value(value.clone())?;
                let verifier = input
                    .code_verifier
                    .as_deref()
                    .ok_or_else(OAuthError::grant)?;
                if !(43..=128).contains(&verifier.len())
                    || !verifier
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
                    || URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != c.challenge
                    || input.redirect_uri.as_deref() != Some(c.redirect_uri.as_str())
                {
                    return Err(OAuthError::grant());
                }
                let (g, _) = self.validate_grant(c.grant_id).await?;
                if g.client_id != input.client_id
                    || input.resource.as_deref().is_some_and(|r| r != g.resource)
                {
                    return Err(OAuthError::grant());
                }
                if !self
                    .repository
                    .oauth_consume("code", &hash(code), &value)
                    .await?
                {
                    return Err(OAuthError::grant());
                }
                (g, None)
            }
            "refresh_token" => {
                let old = hash(
                    input
                        .refresh_token
                        .as_deref()
                        .ok_or_else(OAuthError::grant)?,
                );
                let id = self
                    .repository
                    .oauth_refresh_grant(&old)
                    .await?
                    .ok_or_else(OAuthError::grant)?;
                let (g, _) = self.validate_grant(id).await?;
                if g.client_id != input.client_id
                    || input.resource.as_deref().is_some_and(|r| r != g.resource)
                {
                    return Err(OAuthError::grant());
                }
                (g, Some(old))
            }
            _ => {
                return Err(OAuthError {
                    error: "unsupported_grant_type",
                    description: "Only authorization_code and refresh_token are supported.",
                })
            }
        };
        let refresh_token = format!("mcp_rt_{}", random_token());
        if !self
            .repository
            .oauth_rotate_refresh(
                grant.id,
                old.as_deref(),
                &hash(&refresh_token),
                grant.expires_at,
            )
            .await?
        {
            return Err(OAuthError::grant());
        }
        let access_token = format!("mcp_at_{}", random_token());
        let expires_in = ACCESS_SECONDS.min(grant.expires_at - now());
        self.repository
            .oauth_put(
                "access",
                &hash(&access_token),
                json!({"grant_id":grant.id}),
                now() + expires_in,
            )
            .await?;
        Ok(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in,
            refresh_token,
            scope: SCOPE,
        })
    }
    pub async fn authenticate(&self, token: &str, instance: &str) -> Result<UserApiKeyActor> {
        if !token.starts_with("mcp_at_") {
            return Err(OAuthError::grant());
        }
        let value = self
            .repository
            .oauth_get("access", &hash(token))
            .await?
            .ok_or_else(OAuthError::grant)?;
        let id = serde_json::from_value(value["grant_id"].clone())?;
        let (g, actor) = self.validate_grant(id).await?;
        if g.instance_id != instance || g.resource != resource_url(&self.issuer, instance)? {
            return Err(OAuthError::grant());
        }
        Ok(actor)
    }
}
#[cfg(test)]
#[path = "../_tests/mcp_oauth.rs"]
mod tests;
