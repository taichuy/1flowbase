use domain::ActorContext;
use thiserror::Error;
use uuid::Uuid;

mod sealed {
    pub trait Sealed {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrincipalProfile {
    Public,
    User,
    Application,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserCredentialKind {
    CookieSession,
    UserApiKey { api_key_id: Uuid },
    ServerDelegation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrincipalSummary {
    profile: PrincipalProfile,
    user_id: Option<Uuid>,
    application_id: Option<Uuid>,
    api_key_id: Option<Uuid>,
    workspace_id: Option<Uuid>,
}

impl PrincipalSummary {
    fn public() -> Self {
        Self {
            profile: PrincipalProfile::Public,
            user_id: None,
            application_id: None,
            api_key_id: None,
            workspace_id: None,
        }
    }

    fn user(actor: &ActorContext, api_key_id: Option<Uuid>) -> Self {
        Self {
            profile: PrincipalProfile::User,
            user_id: Some(actor.user_id),
            application_id: None,
            api_key_id,
            workspace_id: Some(actor.current_workspace_id),
        }
    }

    fn application(principal: &ApplicationPrincipal) -> Self {
        Self {
            profile: PrincipalProfile::Application,
            user_id: Some(principal.authorized_actor.user_id),
            application_id: Some(principal.application_id),
            api_key_id: Some(principal.api_key_id),
            workspace_id: Some(principal.workspace_id),
        }
    }

    pub fn profile(&self) -> PrincipalProfile {
        self.profile
    }

    pub fn user_id(&self) -> Option<Uuid> {
        self.user_id
    }

    pub fn application_id(&self) -> Option<Uuid> {
        self.application_id
    }

    pub fn api_key_id(&self) -> Option<Uuid> {
        self.api_key_id
    }

    pub fn workspace_id(&self) -> Option<Uuid> {
        self.workspace_id
    }
}

pub trait InvocationPrincipal: sealed::Sealed + Clone + Send + Sync + 'static {
    const PROFILE: PrincipalProfile;

    fn summary(&self) -> PrincipalSummary;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Public callers intentionally have no `ActorContext` accessor.
///
/// ```compile_fail
/// use interface_runtime::PublicPrincipal;
/// let principal = PublicPrincipal::new();
/// let _actor = principal.actor();
/// ```
pub struct PublicPrincipal;

impl PublicPrincipal {
    pub fn new() -> Self {
        Self
    }
}

impl sealed::Sealed for PublicPrincipal {}

impl InvocationPrincipal for PublicPrincipal {
    const PROFILE: PrincipalProfile = PrincipalProfile::Public;

    fn summary(&self) -> PrincipalSummary {
        PrincipalSummary::public()
    }
}

#[derive(Clone, Debug)]
pub struct UserPrincipal {
    actor: ActorContext,
    credential_kind: UserCredentialKind,
}

impl UserPrincipal {
    pub fn new(actor: ActorContext, credential_kind: UserCredentialKind) -> Self {
        Self {
            actor,
            credential_kind,
        }
    }

    pub fn server_delegation(actor: ActorContext) -> Self {
        Self::new(actor, UserCredentialKind::ServerDelegation)
    }

    pub fn actor(&self) -> &ActorContext {
        &self.actor
    }

    pub fn credential_kind(&self) -> UserCredentialKind {
        self.credential_kind
    }
}

impl sealed::Sealed for UserPrincipal {}

impl InvocationPrincipal for UserPrincipal {
    const PROFILE: PrincipalProfile = PrincipalProfile::User;

    fn summary(&self) -> PrincipalSummary {
        PrincipalSummary::user(
            &self.actor,
            match self.credential_kind {
                UserCredentialKind::UserApiKey { api_key_id } => Some(api_key_id),
                UserCredentialKind::CookieSession | UserCredentialKind::ServerDelegation => None,
            },
        )
    }
}

#[derive(Clone, Debug)]
/// Application identity cannot be constructed without all validated identities.
///
/// ```compile_fail
/// use interface_runtime::ApplicationPrincipal;
/// let _principal = ApplicationPrincipal {};
/// ```
pub struct ApplicationPrincipal {
    application_id: Uuid,
    api_key_id: Uuid,
    workspace_id: Uuid,
    authorized_actor: ActorContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ApplicationPrincipalError {
    #[error("application principal {identity} identity must not be nil")]
    MissingIdentity { identity: &'static str },
    #[error("application principal workspace does not match its authorized actor")]
    WorkspaceMismatch,
}

impl ApplicationPrincipal {
    pub fn new(
        application_id: Uuid,
        api_key_id: Uuid,
        workspace_id: Uuid,
        authorized_actor: ActorContext,
    ) -> Result<Self, ApplicationPrincipalError> {
        for (identity, value) in [
            ("application", application_id),
            ("api_key", api_key_id),
            ("workspace", workspace_id),
        ] {
            if value.is_nil() {
                return Err(ApplicationPrincipalError::MissingIdentity { identity });
            }
        }
        if workspace_id != authorized_actor.current_workspace_id {
            return Err(ApplicationPrincipalError::WorkspaceMismatch);
        }
        Ok(Self {
            application_id,
            api_key_id,
            workspace_id,
            authorized_actor,
        })
    }

    pub fn application_id(&self) -> Uuid {
        self.application_id
    }

    pub fn api_key_id(&self) -> Uuid {
        self.api_key_id
    }

    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    pub fn authorized_actor(&self) -> &ActorContext {
        &self.authorized_actor
    }
}

impl sealed::Sealed for ApplicationPrincipal {}

impl InvocationPrincipal for ApplicationPrincipal {
    const PROFILE: PrincipalProfile = PrincipalProfile::Application;

    fn summary(&self) -> PrincipalSummary {
        PrincipalSummary::application(self)
    }
}
