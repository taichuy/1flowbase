//! Typed canonical interface; password hashes and OAuth proof carriers stay Host-only.
use super::*;
use crate::routes::console_interface::{
    ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use std::sync::Weak;
pub(crate) const OWNER: &str = "api-server.me-authentication-methods";
pub(crate) const OPERATIONS: &[&str] = &[
    "console.identity.authentication-methods.read",
    "console.identity.external-identity.unlink",
    "console.identity.password-setup.authorize",
    "console.identity.password.initialize",
];
pub(crate) const BINDING_IDS: &[&str] = &[
    "http.console.identity.authentication-methods.read.v1",
    "http.console.identity.external-identity.unlink.v1",
    "http.console.identity.password-setup.authorize.v1",
    "http.console.identity.password.initialize.v1",
];
const PATHS: &[&str] = &[
    "/api/console/me/authentication-methods",
    "/api/console/me/authentication-methods/external-identities/:identity_id/unlink",
    "/api/console/me/authentication-methods/password/authorize",
    "/api/console/me/authentication-methods/password",
];
const SUMMARIES: &[&str] = &[
    "Read personal authentication methods",
    "Unlink an owned external identity",
    "Verify an identity for initial password setup",
    "Initialize a local password",
];
const DESCRIPTIONS:&[&str]=&["Return current password availability and owned external identity bindings without configuration secrets.","Retain the external subject owner while disabling its login binding; preserve another usable method atomically.","Start a browser and session bound provider verification for setting the account's first password.","Consume a short-lived session bound verification, create a local account credential atomically and revoke existing sessions."];
pub(crate) fn metadata(method: &str, path: &str) -> Option<(&'static str, &'static str)> {
    PATHS
        .iter()
        .enumerate()
        .find(|(i, p)| {
            **p == path && method.eq_ignore_ascii_case(if *i == 0 { "GET" } else { "POST" })
        })
        .map(|(i, _)| (SUMMARIES[i], DESCRIPTIONS[i]))
}
pub(crate) enum Input {
    Read,
    Unlink(Uuid),
    AuthorizePassword(Uuid),
    InitializePassword(String),
}
pub(crate) enum Output {
    Methods(AuthenticationMethodsResponse),
    Authorization(CookieJar, String),
    Complete,
}
macro_rules! private_contract {
    ($ty:ty,$id:literal) => {
        impl InterfaceContract for $ty {
            const CONTRACT_ID: &'static str = $id;
            const CONTRACT_VERSION: &'static str = "1";
            fn managed_projection_schema() -> Option<serde_json::Value> {
                Some(crate::extension_bus::managed_projection::object_schema(&[]))
            }
            fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
                Some(crate::extension_bus::managed_projection::object_value(&[]))
            }
        }
    };
}
private_contract!(Input, "personal-authentication-methods-input");
private_contract!(Output, "personal-authentication-methods-output");
struct Port(Weak<ApiState>);
impl ConsoleInterfacePort<Input, Output> for Port {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        Box::pin(async move {
            let result = async {
                let state = self.0.upgrade().ok_or(Error::NotAuthenticated)?;
                let actor = principal.actor();
                let session_id = principal
                    .authenticated_session()
                    .ok_or(Error::NotAuthenticated)?
                    .expose_to_trusted_handler();
                match input {
                    Input::Read => Ok(Output::Methods(
                        read_methods(&state, actor, session_id).await?,
                    )),
                    Input::AuthorizePassword(login_entry_id) => {
                        let available = available(&state).await?;
                        let view = AuthenticationMethodsService::new(state.store.clone())
                            .read(actor.user_id, &available.authorities)
                            .await?;
                        if view.password_configured {
                            return Err(Error::Conflict("password_already_configured").into());
                        }
                        let entry = state
                            .store
                            .find_login_entry(login_entry_id)
                            .await?
                            .ok_or(Error::NotFound("login_entry"))?;
                        let provider =
                            crate::authentication_runtime::provider(&state, &entry.auth_type)
                                .await?;
                        if !view.bindings.iter().any(|binding| {
                            binding.state == "bound"
                                && binding.provider_available
                                && binding.authority_namespace == provider.authority_namespace
                        }) {
                            return Err(Error::PermissionDenied(
                                "password_setup_identity_required",
                            )
                            .into());
                        }
                        release(available.leases).await?;
                        let (cookies, url) = crate::routes::external_auth::begin_for_purpose(
                            &state,
                            entry,
                            Some(BindingIntent {
                                actor_id: actor.user_id,
                                session_hash: transaction::digest(session_id),
                            }),
                            Purpose::PasswordSetup,
                        )
                        .await?;
                        Ok(Output::Authorization(cookies, url))
                    }
                    Input::Unlink(identity_id) => {
                        let available = available(&state).await?;
                        AuthenticationMethodsService::new(state.store.clone())
                            .unlink(actor, identity_id, &available.authorities)
                            .await?;
                        release(available.leases).await?;
                        Ok(Output::Complete)
                    }
                    Input::InitializePassword(new_password) => {
                        let proof = proof::claim(&state, actor.user_id, session_id).await?;
                        let entry = state
                            .store
                            .find_login_entry(proof.login_entry_id)
                            .await?
                            .ok_or(Error::NotFound("login_entry"))?;
                        let connection = state
                            .store
                            .find_authentication_connection(entry.connection_id)
                            .await?
                            .ok_or(Error::NotFound("authentication_connection"))?;
                        let provider =
                            crate::authentication_runtime::provider(&state, &entry.auth_type)
                                .await?;
                        if !entry.enabled
                            || provider.fingerprint != proof.provider_fingerprint
                            || provider.authority_namespace != proof.authority_namespace
                            || super::super::external_auth::fingerprint(&entry)?
                                != proof.entry_fingerprint
                            || super::super::external_auth::fingerprint(&connection)?
                                != proof.connection_fingerprint
                        {
                            return Err(Error::PermissionDenied(
                                "password_setup_verification_required",
                            )
                            .into());
                        }
                        if new_password.chars().count() < 8 || new_password.len() > 1024 {
                            return Err(Error::InvalidInput("new_password").into());
                        }
                        let password_hash = super::super::me::hash_password(&new_password)?;
                        let available = available(&state).await?;
                        AuthenticationMethodsService::new(state.store.clone())
                            .initialize_password(
                                actor,
                                proof.identity_id,
                                &password_hash,
                                &available.authorities,
                            )
                            .await?;
                        state.session_store.delete(session_id).await?;
                        release(available.leases).await?;
                        Ok(Output::Complete)
                    }
                }
            }
            .await;
            result.map_err(ConsoleInterfaceTargetError)
        })
    }
}
pub(crate) fn compile_registry(
    state: &Arc<ApiState>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    let declarations = OPERATIONS
        .iter()
        .enumerate()
        .map(|(i, operation)| ConsoleInterfaceDeclaration {
            interface_id: operation,
            binding_id: BINDING_IDS[i],
            method: if i == 0 { "GET" } else { "POST" },
            path: PATHS[i],
            mutating: i != 0,
        })
        .collect::<Vec<_>>();
    crate::routes::console_interface::compile_registry(
        OWNER,
        "graph:personal-authentication-methods-v1",
        &declarations,
        Arc::new(Port(Arc::downgrade(state))),
    )
}
